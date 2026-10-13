//! Bounded geometry evaluation shared by native tools and the renderer-free browser worker.
mod appearance;
mod assembly;
mod budget;
mod cache;
mod evaluate;
mod frames;
mod instances;
mod job;
mod operations;
mod plan;
mod profile_ops;
mod result;
mod shapes;

pub use evaluate::Evaluator;
pub use frames::Frame;
pub use job::{JobToken, hash};
pub use result::{Evaluation, Material, Part, Quality};

pub(crate) type Fail = Box<ve_graph::Diagnostic>;

pub(crate) fn fail(code: &str, message: impl Into<String>) -> Fail {
    Box::new(ve_graph::Diagnostic::error(code, message))
}
