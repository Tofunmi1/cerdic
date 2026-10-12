//! Consistent numeric budget diagnostics for preflight and actual output checks.
use crate::Fail;

#[cfg_attr(
    target_family = "wasm",
    expect(
        clippy::unnecessary_box_returns,
        reason = "same diagnostic ABI on both targets"
    )
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "diagnostic estimates may round above the bounded budgets; comparisons use usize"
)]
pub(crate) fn exceeded(code: &str, limit: usize, observed: usize) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(code, "geometry or storage budget exceeded")
            .with_limit(limit as f64, observed as f64),
    )
}
