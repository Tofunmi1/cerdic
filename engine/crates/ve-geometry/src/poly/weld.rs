//! Deterministic tolerance weld using a sorted spatial grid.

mod attributes;
mod grid;
mod mesh;

use ve_kernel::ExecutionContext;

use crate::{GeometryError, PolyMesh, validate};
use grid::{build_cells, compare_candidate_pairs, count_candidate_pairs, representatives};
use mesh::{construct_candidate, rewrite_faces, validate_candidate};

/// Welded candidate and count of exact vertex-distance pairs tested.
#[derive(Clone, Debug, PartialEq)]
pub struct WeldResult {
    /// Candidate mesh, validated before publication.
    pub mesh: PolyMesh,
    /// Candidate pairs checked against the requested tolerance.
    pub pairs_tested: usize,
}

/// Welds vertices within a positive tolerance using deterministic minimum-index representatives.
///
/// Point attributes use the representative value. Corner attributes remain attached to their
/// original corners, and edge attributes follow mapped edges; conflicting collapsed edge values
/// refuse. Pair budget counts candidate distance checks and is preflighted before those checks.
///
/// # Errors
/// Returns a typed refusal for bad tolerance, budget, collapsed faces or new topology conflicts;
/// returns structural or cancellation errors without changing `mesh`.
pub fn weld(
    mesh: &PolyMesh,
    tolerance: f64,
    pair_budget: usize,
    context: &ExecutionContext,
) -> Result<WeldResult, GeometryError> {
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(GeometryError::PolyRefusal {
            code: "poly.tolerance_invalid",
            element: None,
        });
    }
    let before = validate(mesh, context)?;
    let cells = build_cells(mesh, tolerance, context)?;
    let pairs_tested = count_candidate_pairs(&cells, context)?;
    if pairs_tested > pair_budget {
        return Err(GeometryError::PolyRefusal {
            code: "poly.weld_pair_budget",
            element: None,
        });
    }
    let mut parents: Vec<usize> = (0..mesh.positions().len()).collect();
    compare_candidate_pairs(mesh, tolerance, &cells, &mut parents, context)?;
    let (representatives, remap) = representatives(&mut parents, context)?;
    let (face_sizes, corners, corner_sources) = rewrite_faces(mesh, &remap, context)?;
    let candidate = construct_candidate(
        mesh,
        &representatives,
        &remap,
        &face_sizes,
        &corners,
        &corner_sources,
        context,
    )?;
    validate_candidate(&candidate, &representatives, &before, context)?;
    Ok(WeldResult {
        mesh: candidate,
        pairs_tested,
    })
}
