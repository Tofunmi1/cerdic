//! Incremental evaluator. One step executes one node, allowing the worker to yield between calls.
use crate::{
    Evaluation, EvaluationWarning, Fail, Quality, SceneEnvironment, budget,
    cache::{Cache, Value},
    fail, operations, plan,
};
use std::collections::BTreeMap;
use std::rc::Rc;
use ve_graph::{Document, NodeId, Origin};
use ve_kernel::ExecutionContext;

struct Job {
    /// The document with its groups unfolded; what is planned and executed.
    doc: Document,
    /// Which instance and member each unfolded node came from, for diagnostics.
    origins: BTreeMap<NodeId, Origin>,
    /// The written name of each authored node (instances and group members included), to name a failure.
    labels: BTreeMap<NodeId, String>,
    plan: plan::Plan,
    scripts: Vec<crate::ScriptDef>,
    quality: Quality,
    next: usize,
    context: ExecutionContext,
    values: BTreeMap<NodeId, (String, Rc<Value>)>,
    staged: Vec<(String, Rc<Value>)>,
    /// Results computed in this job by cache key, so equal nodes (copies of a group's members in
    /// several instances) are computed once even before the cache sees them.
    shared: BTreeMap<String, Rc<Value>>,
    executed: usize,
    hits: usize,
    bytes: usize,
    /// For each node, the outputs (by index in the plan) that read it, directly or through other nodes.
    feeds: BTreeMap<NodeId, Vec<usize>>,
}

/// For each node, the outputs that depend on it: found by walking every output's inputs back to the nodes that feed it.
fn output_feeds(doc: &Document, outputs: &[ve_graph::Output]) -> BTreeMap<NodeId, Vec<usize>> {
    let inputs: BTreeMap<&NodeId, Vec<&NodeId>> = doc
        .nodes
        .iter()
        .map(|node| {
            (
                &node.id,
                node.inputs
                    .values()
                    .flatten()
                    .map(|link| &link.node)
                    .collect(),
            )
        })
        .collect();
    let mut feeds: BTreeMap<NodeId, Vec<usize>> = BTreeMap::new();
    for (index, output) in outputs.iter().enumerate() {
        let mut seen = std::collections::BTreeSet::new();
        let mut stack = vec![&output.from.node];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            feeds.entry(id.clone()).or_default().push(index);
            stack.extend(inputs.get(id).into_iter().flatten().copied());
        }
    }
    feeds
}

/// The node the next [`Evaluator::step`] will run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextNode {
    /// The operator family, such as `solid.box`.
    pub operator: String,
    /// The name the program gave it; inside a group, the instance and the member (`hall, member base`).
    pub label: Option<String>,
}

/// Evaluates immutable documents with a bounded mesh cache; it never owns authoring history.
pub struct Evaluator {
    cache: Cache,
    ao_cache: crate::ao::Cache,
    job: Option<Job>,
    last_executed: usize,
    /// For each output (by index), the newest result of any node that feeds it: the shape as it is built up so far.
    latest: BTreeMap<usize, Rc<Value>>,
    /// The outputs whose `latest` changed since [`Evaluator::take_drafts`] last ran.
    dirty: std::collections::BTreeSet<usize>,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::with_cache_limit(128 * 1024 * 1024)
    }
}

impl Evaluator {
    /// Sets the retained cache byte limit, useful for budget-profile tests.
    #[must_use]
    pub fn with_cache_limit(bytes: usize) -> Self {
        Self {
            cache: Cache::new(bytes),
            ao_cache: crate::ao::Cache::default(),
            job: None,
            last_executed: 0,
            latest: BTreeMap::new(),
            dirty: std::collections::BTreeSet::new(),
        }
    }
    /// The parts of every output that has grown since the last call, as they stand now: the shape built up to the node that last fed it, with
    /// the mesh and material exactly as the nodes made them, before the passes that need the whole scene (ambient occlusion, parenting,
    /// rigging, motion checks). A page can show them while the rest of the document is still evaluating, so a model grows step by step; the
    /// finished [`Evaluation`] replaces them. Each part's `node` is its output's node, so a newer draft replaces an older one of the same output.
    pub fn take_drafts(&mut self) -> Vec<crate::Part> {
        let dirty = std::mem::take(&mut self.dirty);
        let (Some(job), latest) = (self.job.as_ref(), &self.latest) else {
            return Vec::new();
        };
        let mut parts = Vec::new();
        for index in dirty {
            let (Some(output), Some(value)) = (job.plan.outputs.get(index), latest.get(&index))
            else {
                continue;
            };
            // A draft is only a preview: a shape that cannot be built yet is left for the finished evaluation to report.
            if let Ok(made) = crate::instances::parts(output, value) {
                parts.extend(made);
            }
        }
        parts
    }
    /// Remembers the result of the node that just ran as the newest stage of every output it feeds.
    fn collect_drafts(&mut self, job: &Job) {
        let Some(&index) = job
            .next
            .checked_sub(1)
            .and_then(|last| job.plan.order.get(last))
        else {
            return;
        };
        let Some(node) = job.doc.nodes.get(index) else {
            return;
        };
        let (Some((_, value)), Some(feeds)) = (job.values.get(&node.id), job.feeds.get(&node.id))
        else {
            return;
        };
        if value.light.is_some()
            || value.environment.is_some()
            || value
                .game
                .as_ref()
                .is_some_and(|fragment| fragment.definition.is_some())
            || value.mesh.triangle_count() == 0
        {
            return;
        }
        for &output in feeds {
            self.latest.insert(output, Rc::clone(value));
            self.dirty.insert(output);
        }
    }
    /// Retained mesh cache bytes, including charged metadata and map overhead.
    #[must_use]
    pub fn cache_bytes(&self) -> usize {
        self.cache.bytes()
    }
    /// Operators executed in the current or last job.
    #[must_use]
    pub fn evaluated_nodes(&self) -> usize {
        self.job.as_ref().map_or(self.last_executed, |j| j.executed)
    }
    /// Which node the next step runs, or `None` with no job or once every node has run (the next step then finishes the scene).
    /// A caller can time each step and say which line of the program was slow.
    #[must_use]
    pub fn next_node(&self) -> Option<NextNode> {
        let job = self.job.as_ref()?;
        let node = job.doc.nodes.get(*job.plan.order.get(job.next)?)?;
        let label = match job.origins.get(&node.id) {
            Some(origin) => match (
                job.labels.get(&origin.instance),
                job.labels.get(&origin.member),
            ) {
                (Some(i), Some(m)) => Some(format!("{i}, member {m}")),
                (Some(i), None) => Some(i.clone()),
                (None, Some(m)) => Some(m.clone()),
                (None, None) => None,
            },
            None => job.labels.get(&node.id).cloned(),
        };
        Some(NextNode {
            operator: node.op.family().as_str().to_owned(),
            label,
        })
    }
    /// Preflights a document before creating any kernel geometry.
    ///
    /// # Errors
    /// Graph, parameter or budget diagnostics. An existing job is cancelled first.
    pub fn begin(&mut self, doc: &Document, quality: Quality) -> Result<(), Fail> {
        self.cancel();
        self.last_executed = 0;
        let expanded = ve_graph::expand(doc)?;
        let labels: BTreeMap<NodeId, String> = doc
            .nodes
            .iter()
            .filter_map(|n| Some((n.id.clone(), n.label.clone()?)))
            .collect();
        let plan = plan::build(&expanded.doc, quality)
            .map_err(|e| remap(e, &expanded.origins, &labels))?;
        let scripts = crate::script_nodes::collect(doc)?;
        let context = ExecutionContext::new().map_err(|e| fail("kernel.failed", e.to_string()))?;
        let feeds = output_feeds(&expanded.doc, &plan.outputs);
        self.job = Some(Job {
            doc: expanded.doc,
            origins: expanded.origins,
            labels,
            plan,
            scripts,
            quality,
            next: 0,
            context,
            values: BTreeMap::new(),
            staged: Vec::new(),
            shared: BTreeMap::new(),
            executed: 0,
            hits: 0,
            bytes: 0,
            feeds,
        });
        Ok(())
    }
    /// Permanently cancels and discards the current job. Its staged values never enter the cache.
    pub fn cancel(&mut self) {
        self.latest.clear();
        self.dirty.clear();
        if let Some(job) = self.job.take() {
            job.context.cancel();
            self.last_executed = job.executed;
        }
    }
    /// Executes one node, or returns a completed scene. Callers yield between steps on the web.
    ///
    /// # Errors
    /// Kernel or actual-output budget failure; the job is discarded.
    pub fn step(&mut self) -> Result<Option<Evaluation>, Fail> {
        let Some(mut job) = self.job.take() else {
            return Err(fail("eval.no_job", "no active evaluation"));
        };
        if job.next < job.plan.order.len() {
            let result = self.node(&mut job);
            self.last_executed = job.executed;
            result.map_err(|e| remap(e, &job.origins, &job.labels))?;
            self.collect_drafts(&job);
            self.job = Some(job);
            return Ok(None);
        }
        let result = finish(&job, &mut self.ao_cache)?;
        for (key, value) in job.staged {
            self.cache.insert(key, value);
        }
        self.last_executed = result.evaluated_nodes;
        Ok(Some(Evaluation {
            cache_bytes: self.cache.bytes(),
            ..result
        }))
    }
    /// Synchronous native entry point over the same stepped evaluator used by the worker.
    ///
    /// # Errors
    /// Preflight/evaluation diagnostics.
    pub fn evaluate(&mut self, doc: &Document, quality: Quality) -> Result<Evaluation, Fail> {
        self.begin(doc, quality)?;
        loop {
            if let Some(result) = self.step()? {
                return Ok(result);
            }
        }
    }

    /// Runs the document and records every node in the order it ran: operator, parameters, inputs, what it produced and what it changed from its main input
    /// (see [`crate::inspect`]). Nothing from the run is kept in the cache.
    ///
    /// # Errors
    /// The same diagnostics as [`Evaluator::evaluate`]; a failure names the node it came from.
    pub fn trace(
        &mut self,
        doc: &Document,
        quality: Quality,
    ) -> Result<Vec<crate::inspect::NodeTrace>, Fail> {
        self.begin(doc, quality)?;
        loop {
            let Some(mut job) = self.job.take() else {
                return Err(fail("eval.no_job", "no active evaluation"));
            };
            if job.next >= job.plan.order.len() {
                let traces = job
                    .plan
                    .order
                    .iter()
                    .filter_map(|index| job.doc.nodes.get(*index))
                    .filter_map(|node| {
                        let (_, value) = job.values.get(&node.id)?;
                        let label_of = |id: &NodeId| {
                            job.labels
                                .get(id)
                                .cloned()
                                .unwrap_or_else(|| id.to_string())
                        };
                        let inputs: Vec<crate::inspect::InputTrace> = node
                            .inputs
                            .iter()
                            .flat_map(|(port, links)| links.iter().map(move |r| (port, r)))
                            .map(|(port, r)| crate::inspect::InputTrace {
                                port: port.to_string(),
                                from: label_of(&r.node),
                            })
                            .collect();
                        // The first link that is geometry (not a number socket) is the main input.
                        let main = node
                            .inputs
                            .iter()
                            .filter(|(port, _)| !crate::values::is_socket(node, port))
                            .flat_map(|(_, links)| links)
                            .find_map(|r| job.values.get(&r.node).map(|(_, v)| v));
                        let (kind, summary, facts) = crate::inspect::describe(value);
                        Some(crate::inspect::NodeTrace {
                            id: node.id.to_string(),
                            label: job.labels.get(&node.id).cloned(),
                            operator: node.op.to_string(),
                            params: serde_json::to_value(&node.params)
                                .unwrap_or(serde_json::Value::Null),
                            inputs,
                            kind: kind.to_owned(),
                            summary,
                            facts,
                            effect: crate::inspect::effect(main.map(AsRef::as_ref), value),
                            warnings: value
                                .warnings
                                .iter()
                                .map(|w| format!("{}: {}", w.code, w.message))
                                .collect(),
                        })
                    })
                    .collect();
                self.last_executed = job.executed;
                return Ok(traces);
            }
            let result = self.node(&mut job);
            self.last_executed = job.executed;
            result.map_err(|e| remap(e, &job.origins, &job.labels))?;
            self.job = Some(job);
        }
    }

    fn node(&mut self, job: &mut Job) -> Result<(), Fail> {
        let index = *job
            .plan
            .order
            .get(job.next)
            .ok_or_else(|| fail("eval.internal", "missing planned node"))?;
        let node = job
            .doc
            .nodes
            .get(index)
            .ok_or_else(|| fail("eval.internal", "invalid plan index"))?;
        let dependencies: Vec<_> = node
            .inputs
            .values()
            .flatten()
            .map(|r| {
                job.values
                    .get(&r.node)
                    .ok_or_else(|| fail("graph.unknown_node", r.node.to_string()))
            })
            .collect::<Result<_, _>>()?;
        // Number sockets fill the node's parameters; only geometry links are inputs to the operator.
        let resolved = crate::values::resolve(node, &job.values)?;
        let geometry: Vec<_> = node
            .inputs
            .iter()
            .filter(|(port, _)| !crate::values::is_socket(node, port))
            .flat_map(|(_, links)| links)
            .map(|r| {
                job.values
                    .get(&r.node)
                    .ok_or_else(|| fail("graph.unknown_node", r.node.to_string()))
            })
            .collect::<Result<_, _>>()?;
        let hashes: Vec<_> = dependencies.iter().map(|(key, _)| key).collect();
        let key = digest(
            serde_json::json!([
                node.op,
                node.params,
                node.flags,
                hashes,
                job.quality,
                job.doc.seed,
                "ve-modeling.mesh.v1"
            ])
            .to_string()
            .as_bytes(),
        );
        let value = if let Some(value) = job.shared.get(&key).cloned() {
            job.hits += 1;
            value
        } else if let Some(value) = self.cache.get(&key) {
            job.hits += 1;
            value
        } else {
            let inputs: Vec<_> = geometry.iter().map(|(_, value)| Rc::clone(value)).collect();
            let value = Rc::new(operations::execute(
                &resolved,
                &inputs,
                &job.context,
                job.quality,
                &job.labels,
            )?);
            job.executed += 1;
            if value.mesh.triangle_count() > job.quality.triangles() {
                return Err(budget::exceeded(
                    "budget.triangles",
                    job.quality.triangles(),
                    value.mesh.triangle_count(),
                ));
            }
            job.staged.push((key.clone(), Rc::clone(&value)));
            value
        };
        job.bytes = job.bytes.saturating_add(value.bytes());
        if job.bytes > budget::MEMORY_LIMIT {
            return Err(budget::exceeded(
                "budget.memory",
                budget::MEMORY_LIMIT,
                job.bytes,
            ));
        }
        job.shared.insert(key.clone(), Rc::clone(&value));
        job.values.insert(node.id.clone(), (key, value));
        job.next += 1;
        Ok(())
    }
}

/// A failure inside an unfolded group, reported against the instance: the diagnostic names the
/// instance and lists the member (and any member it relates to) so an editor can select them.
///
/// The message ends with the name the program gave the node (`at rough`, or `at the_slab, member n` inside a group), so a
/// model finds the line by what it wrote, not by an id.
fn remap(
    mut failure: Fail,
    origins: &BTreeMap<NodeId, Origin>,
    labels: &BTreeMap<NodeId, String>,
) -> Fail {
    let member_of = |id: &NodeId| {
        origins
            .get(id)
            .map_or_else(|| id.clone(), |o| o.member.clone())
    };
    let mut at = failure.node.as_ref().and_then(|n| labels.get(n)).cloned();
    if let Some(node) = failure.node.clone()
        && let Some(origin) = origins.get(&node)
    {
        let instance = labels.get(&origin.instance);
        let member = labels.get(&origin.member);
        at = match (instance, member) {
            (Some(i), Some(m)) => Some(format!("{i}, member {m}")),
            (Some(i), None) => Some(i.clone()),
            (None, Some(m)) => Some(m.clone()),
            (None, None) => None,
        };
        failure.node = Some(origin.instance.clone());
        failure.related.insert(0, origin.member.clone());
    }
    if let Some(at) = at {
        failure.message = format!("{} (at {at})", failure.message);
    }
    for related in &mut failure.related {
        *related = member_of(related);
    }
    failure.related.dedup();
    failure
}

fn finish(job: &Job, ao_cache: &mut crate::ao::Cache) -> Result<Evaluation, Fail> {
    let mut parts = Vec::new();
    let mut lights = Vec::new();
    let mut environment: Option<SceneEnvironment> = None;
    let mut authored_environment = false;
    let mut authored_sky = false;
    let mut triangles = budget::Triangles::default();
    for output in &job.plan.outputs {
        let (_, value) = job
            .values
            .get(&output.from.node)
            .ok_or_else(|| fail("graph.unknown_node", output.from.node.to_string()))?;
        if let Some(light) = &value.light {
            let mut light = light.clone();
            light.name.clone_from(&output.name);
            light.node.clone_from(&output.from.node);
            lights.push(light);
            continue;
        }
        if value
            .game
            .as_ref()
            .is_some_and(|fragment| fragment.definition.is_some())
        {
            continue;
        }
        if let Some(scene) = &value.environment {
            if let Some(sky) = &scene.sky {
                if authored_sky {
                    return Err(fail(
                        "scene.sky.duplicate",
                        "only one scene.sky output is allowed",
                    ));
                }
                authored_sky = true;
                let mut sun = crate::SceneLight {
                    node: sky.node.clone(),
                    name: output.name.clone(),
                    kind: crate::SceneLightKind::Sun,
                    position: [0.0; 3],
                    direction: sky.sun_direction,
                    color: crate::environment::sky::sun_colour(sky.sun_direction),
                    intensity: sky.sun_intensity,
                    range: 0.0,
                    inner_angle: 0.0,
                    outer_angle: 0.0,
                };
                sun.name.clone_from(&output.name);
                lights.push(sun);
                if let Some(existing) = &mut environment {
                    crate::environment::add_sky(existing, scene, sky);
                } else {
                    environment = Some(scene.clone());
                }
            } else {
                if authored_environment {
                    return Err(fail(
                        "scene.environment.duplicate",
                        "only one scene.environment output is allowed",
                    ));
                }
                authored_environment = true;
                environment = Some(crate::environment::over_sky(scene, environment.as_ref()));
            }
            continue;
        }
        let generated = crate::instances::parts(output, value)?;
        triangles.add(&output.from.node, &generated, job.quality)?;
        parts.extend(generated);
    }
    // The game is the first scene output that carries one; only that one has to be playable.
    let mut game = None;
    for output in &job.plan.outputs {
        let fragment = job
            .values
            .get(&output.from.node)
            .and_then(|(_, value)| value.game.as_ref());
        if let Some(found) = fragment.map(|f| f.finish()).transpose()?.flatten() {
            game = Some(found);
            break;
        }
    }
    crate::ao::bake(&mut parts, job.quality, ao_cache)?;
    let defined_clips: std::collections::BTreeSet<&str> = parts
        .iter()
        .flat_map(|part| part.animation.iter().map(|track| track.clip.as_str()))
        .collect();
    let mut machine_warnings = Vec::new();
    let mut warned = std::collections::BTreeSet::new();
    for (_, value) in job.values.values() {
        let (Some(machine), Some(node)) = (&value.state_machine, &value.state_machine_node) else {
            continue;
        };
        let clips = machine
            .states
            .iter()
            .flat_map(|state| match &state.motion {
                ve_animstate::Motion::Clip { clip, .. } => vec![clip.as_str()],
                ve_animstate::Motion::Blend1D { points, .. } => {
                    points.iter().map(|point| point.clip.as_str()).collect()
                }
            })
            // An overlay layer plays a clip of its own: it must exist too.
            .chain(machine.extras.layers.iter().map(|layer| layer.clip.as_str()));
        for clip in clips {
            if !defined_clips.contains(clip) && warned.insert((node.clone(), clip.to_owned())) {
                machine_warnings.push(EvaluationWarning {
                    code: "anim.machine.missing_clip".to_owned(),
                    node: node.to_string(),
                    related: Vec::new(),
                    message: format!("the machine plays clip \"{clip}\", which no animation track defines; add anim.track or anim.wave nodes with clip=\"{clip}\""),
                });
            }
        }
    }
    let mut hierarchy_warnings = crate::hierarchy::resolve(&mut parts);
    let motion_warnings = crate::check_motion::evaluate(&job.doc, &job.plan.outputs, &parts)?;
    hierarchy_warnings.extend(motion_warnings);
    hierarchy_warnings.extend(crate::rig::weigh(&mut parts));
    hierarchy_warnings.extend(crate::rig::resolve_ik(&mut parts));
    hierarchy_warnings.extend(crate::fx::check_parts(&mut parts));
    hierarchy_warnings.extend(crate::ragdoll::resolve(&mut parts));
    hierarchy_warnings.extend(crate::cloth_sim::resolve(&mut parts));
    hierarchy_warnings.extend(crate::fluid_sim::resolve(&mut parts));
    hierarchy_warnings.extend(crate::physics::check(&mut parts));
    hierarchy_warnings.extend(triangles.heavy_warning());
    lights.extend(crate::emitters::from_parts(&parts));
    if lights.len() > crate::MAX_SCENE_LIGHTS {
        return Err(budget::exceeded(
            "budget.lights",
            crate::MAX_SCENE_LIGHTS,
            lights.len(),
        ));
    }
    Ok(Evaluation {
        parts,
        lights,
        environment,
        scripts: job.scripts.clone(),
        game,
        evaluated_nodes: job.executed,
        cache_hits: job.hits,
        cache_bytes: 0,
        warnings: collect_warnings(
            job.values
                .values()
                .flat_map(|(_, value)| value.warnings.iter().cloned())
                .chain(hierarchy_warnings)
                .chain(machine_warnings),
        ),
    })
}

fn collect_warnings(
    warnings: impl IntoIterator<Item = EvaluationWarning>,
) -> Vec<EvaluationWarning> {
    let mut warnings: Vec<_> = warnings.into_iter().collect();
    warnings.sort();
    warnings.dedup();
    warnings
}

fn digest(bytes: &[u8]) -> String {
    let hash = |seed: u64| {
        bytes.iter().fold(seed, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3)
        })
    };
    format!(
        "{:016x}{:016x}",
        hash(0xcbf2_9ce4_8422_2325),
        hash(0x8422_2325_cbf2_9ce4)
    )
}

#[cfg(test)]
mod warning_tests {
    use super::collect_warnings;
    use crate::EvaluationWarning;

    #[test]
    fn warnings_from_multiple_nodes_are_kept_and_duplicates_are_removed() {
        let first = EvaluationWarning::attribute_dropped("n_0123abcd", "material");
        let second = EvaluationWarning {
            node: "n_89abcdef".to_owned(),
            ..first.clone()
        };
        let result = collect_warnings([first.clone(), second.clone(), first]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].node, "n_0123abcd");
        assert_eq!(result[1], second);
    }
}
