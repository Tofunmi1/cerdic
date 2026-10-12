//! Bounded geometry evaluation shared by native tools and the renderer-free browser worker.
mod budget;
mod cache;
mod evaluate;
mod job;
mod operations;
mod plan;
mod result;

pub use evaluate::Evaluator;
pub use job::{JobToken, hash};
pub use result::{Evaluation, Part, Quality};

pub(crate) type Fail = Box<ve_graph::Diagnostic>;

#[cfg_attr(
    target_family = "wasm",
    expect(
        clippy::unnecessary_box_returns,
        reason = "Diagnostic exceeds the small Result threshold on native targets"
    )
)]
pub(crate) fn fail(code: &str, message: impl Into<String>) -> Fail {
    Box::new(ve_graph::Diagnostic::error(code, message))
}
