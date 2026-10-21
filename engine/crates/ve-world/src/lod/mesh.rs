//! Tubes and leaf cards from a skeleton.
mod helpers;
mod leaves;

use super::{LodError, Meshing, PlantMesh};
use crate::flora::Skeleton;

/// Meshes a plant as tapered bark tubes and leaf cards.
///
/// # Errors
/// Returns `Cancelled`, `TooLarge`, or `BadParameter` for invalid settings.
pub fn mesh_plant(
    skeleton: &Skeleton,
    params: &Meshing,
    cancel: &dyn Fn() -> bool,
) -> Result<PlantMesh, LodError> {
    helpers::validate(params)?;
    if cancel() {
        return Err(LodError::Cancelled);
    }
    let mut mesh = PlantMesh::default();
    let bark_start = 0;
    for line in skeleton.polylines() {
        if cancel() {
            return Err(LodError::Cancelled);
        }
        helpers::add_tube(&mut mesh, skeleton, &line, params)?;
    }
    if mesh.indices.len() > bark_start {
        mesh.material_ranges.push(helpers::range(
            super::Surface::Bark,
            bark_start,
            mesh.indices.len(),
        ));
    }
    let leaf_start = mesh.indices.len();
    for leaf in &skeleton.leaves {
        if cancel() {
            return Err(LodError::Cancelled);
        }
        leaves::add_leaf(&mut mesh, skeleton, leaf, params.leaf_cards);
        helpers::check_size(mesh.vertices.len())?;
    }
    if mesh.indices.len() > leaf_start {
        mesh.material_ranges.push(helpers::range(
            super::Surface::Leaf,
            leaf_start,
            mesh.indices.len(),
        ));
    }
    Ok(mesh)
}
