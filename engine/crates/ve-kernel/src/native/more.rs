//! The remaining mesh operators, split between owner files.

use crate::{EditedMesh, KernelError, MeshOp, MeshSelection, PolygonMesh};

/// Runs the operator natively, or returns `None` when it is not implemented yet.
#[must_use]
pub fn edit(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Option<Result<EditedMesh, KernelError>> {
    super::more_a::edit(mesh, selection, op)
        .or_else(|| super::more_b::edit(mesh, selection, op))
        .or_else(|| super::more_c::edit(mesh, selection, op))
        .or_else(|| super::subdivide::edit(mesh, selection, op))
        .or_else(|| super::unsubdivide::edit(mesh, selection, op))
}
