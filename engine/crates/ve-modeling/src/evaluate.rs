//! Incremental evaluator. One step executes one node, allowing the worker to yield between calls.
use crate::{
    Evaluation, Fail, Part, Quality, budget,
    cache::{Cache, Value},
    fail, operations, plan,
};
use std::collections::BTreeMap;
use std::rc::Rc;
use ve_graph::{Document, NodeId};
use ve_kernel::ExecutionContext;

struct Job {
    doc: Document,
    plan: plan::Plan,
    quality: Quality,
    next: usize,
    context: ExecutionContext,
    values: BTreeMap<NodeId, (String, Rc<Value>)>,
    staged: Vec<(String, Rc<Value>)>,
    executed: usize,
    hits: usize,
    bytes: usize,
}

/// Evaluates immutable documents with a bounded mesh cache; it never owns authoring history.
pub struct Evaluator {
    cache: Cache,
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
        let plan = plan::build(doc, quality)?;
        let context = ExecutionContext::new().map_err(|e| fail("kernel.failed", e.to_string()))?;
        self.job = Some(Job {
            doc: doc.clone(),
            plan,
            quality,
            next: 0,
            context,
            values: BTreeMap::new(),
            staged: Vec::new(),
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
            result?;
            self.job = Some(job);
            return Ok(None);
        }
        let result = finish(&job)?;
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
        let value = if let Some(value) = self.cache.get(&key) {
            job.hits += 1;
            value
        } else {
            let inputs: Vec<_> = dependencies
                .iter()
                .map(|(_, value)| Rc::clone(value))
                .collect();
            let value = Rc::new(operations::execute(node, &inputs, &job.context)?);
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
        job.values.insert(node.id.clone(), (key, value));
        job.next += 1;
        Ok(())
    }
}

fn finish(job: &Job) -> Result<Evaluation, Fail> {
    let mut parts = Vec::new();
    let mut triangles = 0_usize;
    for output in &job.plan.outputs {
        let (_, value) = job
            .values
            .get(&output.from.node)
            .ok_or_else(|| fail("graph.unknown_node", output.from.node.to_string()))?;
        triangles = triangles.saturating_add(value.mesh.triangle_count());
        if triangles > job.quality.triangles() {
            return Err(budget::exceeded(
                "budget.triangles",
                job.quality.triangles(),
                triangles,
            ));
        }
        parts.push(Part {
            node: output.from.node.clone(),
            name: output.name.clone(),
            positions: value.mesh.positions.clone(),
            indices: value.mesh.indices.clone(),
            volume: value.volume,
            material: value.material.clone(),
        });
    }
    Ok(Evaluation {
        parts,
        evaluated_nodes: job.executed,
        cache_hits: job.hits,
        cache_bytes: 0,
    })
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
