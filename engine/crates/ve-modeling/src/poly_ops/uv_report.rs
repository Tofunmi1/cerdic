//! What `poly.unwrap_uv` tells the author about the layout it made: refusals and warnings that say how to fix it.

use ve_geometry::{PolyMesh, uv_quality};
use ve_graph::{Diagnostic, Node};

use crate::{EvaluationWarning, Fail};

/// Texel density this many times the mean (or its inverse) counts as stretched.
const STRETCH_LIMIT: f64 = 4.0;
/// Share of covered texels used by two faces above which islands count as overlapping.
const OVERLAP_LIMIT: f64 = 0.05;

fn warning(node: &Node, code: &str, message: String) -> EvaluationWarning {
    EvaluationWarning {
        code: code.to_owned(),
        node: node.id.to_string(),
        related: Vec::new(),
        message,
    }
}

/// Measures the unwrapped mesh. A layout with no usable area is refused; stretch, flips and overlap are warnings.
pub(super) fn uv_report(node: &Node, mesh: &PolyMesh) -> Result<Vec<EvaluationWarning>, Fail> {
    let Some(quality) = uv_quality(mesh) else {
        return Err(Box::new(
            Diagnostic::error(
                "poly.uv_unwrap_failed",
                format!(
                    "{}: the unwrap left every face with no area in UV space; the surface is probably closed or has a handle. \
                     Select seam edges (poly.select element=edge) to cut it open, or keep auto_seams on",
                    node.op
                ),
            )
            .at_node(&node.id),
        ));
    };
    let mut warnings = Vec::new();
    if quality.max_density_ratio > STRETCH_LIMIT || quality.min_density_ratio < 1.0 / STRETCH_LIMIT
    {
        warnings.push(warning(
            node,
            "poly.uv.stretched",
            format!(
                "UV texel density varies from {:.2}x to {:.1}x the mean, so textures will look stretched or squashed there; \
                 add seams across the stretched area, lower seam_angle, or use method=angle",
                quality.min_density_ratio, quality.max_density_ratio
            ),
        ));
    }
    if quality.flipped_faces > 0 {
        warnings.push(warning(
            node,
            "poly.uv.flipped",
            format!("{} face(s) are mirrored in UV space, so their texture runs backwards; add a seam where the island folds over", quality.flipped_faces),
        ));
    }
    if quality.overlap > OVERLAP_LIMIT {
        warnings.push(warning(
            node,
            "poly.uv.overlap",
            format!("{:.0}% of the used UV area is covered by more than one face, so those faces share texture; turn pack on or add seams", quality.overlap * 100.0),
        ));
    }
    Ok(warnings)
}
