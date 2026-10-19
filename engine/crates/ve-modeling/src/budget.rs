//! Consistent numeric budget diagnostics for preflight and actual output checks.
use crate::Fail;

#[expect(
    clippy::cast_precision_loss,
    reason = "diagnostic estimates may round above the bounded budgets; comparisons use usize"
)]
pub(crate) fn exceeded(code: &str, limit: usize, observed: usize) -> Fail {
    let diagnostic = ve_graph::Diagnostic::error(code, "geometry or storage budget exceeded")
        .with_limit(limit as f64, observed as f64);
    let diagnostic = match suggestion(code, limit, observed) {
        Some(text) => diagnostic.suggest(text),
        None => diagnostic,
    };
    Box::new(diagnostic)
}

/// What a caller can change so a numeric budget is not exceeded again, by code. The real limit and
/// observed amount are written into the text where the code has them.
fn suggestion(code: &str, limit: usize, observed: usize) -> Option<String> {
    Some(match code {
        "budget.triangles" => format!(
            "the result would need about {observed} triangles, over the {limit} budget: lower the mesh detail (segments, subdivision levels or sections), or ask for a coarser quality"
        ),
        "budget.memory" => format!(
            "the result would need about {observed} bytes, over the {limit} budget: decimate or lower the detail, or close another document"
        ),
        "budget.instances" => format!(
            "the placements would number {observed}, over the {limit} budget: lower count_x, count_y, count_z or count"
        ),
        "budget.sockets" => format!(
            "the solid would carry {observed} sockets, over the {limit} budget: remove the sockets this part does not need"
        ),
        "budget.lights" => format!(
            "the scene has {observed} lights, over the {limit} budget: remove emitters or share one"
        ),
        "budget.field_cells" => format!(
            "the field grid needs {observed} cells, over the {limit} budget: raise edge_length, or shrink the min and max bounds"
        ),
        "budget.field_ops" => format!(
            "the field needs {observed} work units, over the {limit} budget: raise edge_length, or use fewer field primitives"
        ),
        "budget.field_tape" => format!(
            "the field needs {observed} instructions, over the {limit} budget: use fewer or simpler field primitives"
        ),
        "budget.ao_triangles" => format!(
            "ambient occlusion needs {observed} triangles, over the {limit} budget: lower the mesh detail"
        ),
        _ => return None,
    })
}
