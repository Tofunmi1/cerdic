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
    plan: plan::Plan,
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
}

/// Evaluates immutable documents with a bounded mesh cache; it never owns authoring history.
pub struct Evaluator {
    cache: Cache,
    ao_cache: crate::ao::Cache,
    job: Option<Job>,
    last_executed: usize,
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
    /// Preflights a document before creating any kernel geometry.
    ///
    /// # Errors
    /// Graph, parameter or budget diagnostics. An existing job is cancelled first.
    pub fn begin(&mut self, doc: &Document, quality: Quality) -> Result<(), Fail> {
        self.cancel();
        self.last_executed = 0;
        let expanded = ve_graph::expand(doc)?;
        let plan = plan::build(&expanded.doc, quality).map_err(|e| remap(e, &expanded.origins))?;
        let context = ExecutionContext::new().map_err(|e| fail("kernel.failed", e.to_string()))?;
        self.job = Some(Job {
            doc: expanded.doc,
            origins: expanded.origins,
            plan,
            quality,
            next: 0,
            context,
            values: BTreeMap::new(),
            staged: Vec::new(),
            shared: BTreeMap::new(),
            executed: 0,
            hits: 0,
            bytes: 0,
        });
        Ok(())
    }
    /// Permanently cancels and discards the current job. Its staged values never enter the cache.
    pub fn cancel(&mut self) {
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
            result.map_err(|e| remap(e, &job.origins))?;
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
        if job.bytes > 256 * 1024 * 1024 {
            return Err(budget::exceeded(
                "budget.memory",
                256 * 1024 * 1024,
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
fn remap(mut failure: Fail, origins: &BTreeMap<NodeId, Origin>) -> Fail {
    let member_of = |id: &NodeId| {
        origins
            .get(id)
            .map_or_else(|| id.clone(), |o| o.member.clone())
    };
    if let Some(node) = failure.node.clone()
        && let Some(origin) = origins.get(&node)
    {
        failure.node = Some(origin.instance.clone());
        failure.related.insert(0, origin.member.clone());
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
    let mut triangles = 0_usize;
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
        let drawn = generated
            .iter()
            .map(|part| (part.indices.len() / 3) * part.transforms.as_ref().map_or(1, Vec::len))
            .sum::<usize>();
        triangles = triangles.saturating_add(drawn);
        if triangles > job.quality.triangles() {
            return Err(budget::exceeded(
                "budget.triangles",
                job.quality.triangles(),
                triangles,
            ));
        }
        parts.extend(generated);
    }
    crate::ao::bake(&mut parts, job.quality, ao_cache)?;
    let mut hierarchy_warnings = crate::hierarchy::resolve(&mut parts);
    hierarchy_warnings.extend(crate::rig::weigh(&mut parts));
    hierarchy_warnings.extend(crate::rig::resolve_ik(&mut parts));
    hierarchy_warnings.extend(crate::physics::check(&mut parts));
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
        evaluated_nodes: job.executed,
        cache_hits: job.hits,
        cache_bytes: 0,
        warnings: collect_warnings(
            job.values
                .values()
                .flat_map(|(_, value)| value.warnings.iter().cloned())
                .chain(hierarchy_warnings),
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
