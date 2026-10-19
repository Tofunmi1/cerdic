//! Blender's bmesh operators on polygon meshes: inset, solidify, extrude, dissolve, poke, subdivide,
//! wireframe and symmetrize.

use ve_kernel::{ExecutionContext, KernelError, MeshOp, MeshSelection, PolygonMesh};

use super::blender_result::{into_poly, refusal};
use crate::topology::check_cancelled;
use crate::{Element, GeometryError, PolyMesh, Topology};

fn selection_error(code: &'static str, element: Option<Element>) -> GeometryError {
    GeometryError::PolyRefusal { code, element }
}

/// Runs one of Blender's mesh operators on the selected faces or edges.
///
/// Inset and extrude need faces; solidify, poke and wireframe take faces or, when none are given, the
/// whole mesh; subdivide needs edges; dissolve and symmetrize ignore the selection. The result keeps
/// each face's, point's and corner's attributes through provenance (new elements take their
/// neighbour's); edge attributes refuse, as in the other polygon edits. At most 250,000 faces.
///
/// # Errors
/// `poly.selection_invalid` for a face or edge that is not in the mesh, `poly.selection_empty` when the
/// operator needs a selection and has none, `poly.operator_refused` when Blender declines the mesh,
/// a kernel error for a bad parameter or a mesh over the limits, or cancellation. The input is never
/// changed.
pub fn blender_edit(
    mesh: &PolyMesh,
    faces: &[usize],
    edges: &[(u32, u32)],
    vertices: &[u32],
    op: &MeshOp,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let needs_faces = matches!(op, MeshOp::Inset { .. } | MeshOp::Extrude { .. });
    let needs_edges = matches!(op, MeshOp::Subdivide { .. });
    if (needs_faces && faces.is_empty()) || (needs_edges && edges.is_empty()) {
        return Err(selection_error("poly.selection_empty", None));
    }
    let face_indices = faces
        .iter()
        .map(|&face| {
            u32::try_from(face)
                .ok()
                .filter(|_| face < mesh.face_count())
                .ok_or_else(|| selection_error("poly.selection_invalid", Some(Element::Face(face))))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !edges.is_empty() {
        let topology = Topology::build(mesh, context)?;
        for &(a, b) in edges {
            let key = (a.min(b), a.max(b));
            if !topology.edges().contains_key(&key) {
                return Err(selection_error(
                    "poly.selection_invalid",
                    Some(Element::Edge(key.0, key.1)),
                ));
            }
        }
    }
    check_cancelled(context)?;
    let pairs: Vec<[u32; 2]> = edges.iter().map(|&(a, b)| [a, b]).collect();
    let input = PolygonMesh {
        positions: mesh.positions(),
        face_sizes: mesh.face_sizes(),
        corners: mesh.corners(),
    };
    let selection = MeshSelection {
        faces: &face_indices,
        edges: &pairs,
        vertices,
    };
    let edited = ve_kernel::edit_mesh(&input, &selection, op).map_err(|error| match error {
        KernelError::OperatorRefused => refusal("poly.operator_refused"),
        other => GeometryError::Kernel(other),
    })?;
    into_poly(mesh, &edited, context)
}
