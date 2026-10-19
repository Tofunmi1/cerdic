//! Turning the result of one of Blender's mesh operators back into a [`PolyMesh`] with its attributes.

use ve_kernel::{BeveledMesh, ExecutionContext};

use super::attrs::{Sources, transfer};
use crate::topology::check_cancelled;
use crate::{AttrData, Attribute, Domain, GeometryError, PolyMesh, TriangulationMode};

pub(super) fn refusal(code: &'static str) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: None,
    }
}

/// Builds the polygon mesh for `result`, the output of an operator that ran on `source`. Rounded and
/// offset patches are slightly curved quads, so the result triangulates as curved quads. Point, face
/// and corner attributes follow the input element each output element descends from; edge attributes
/// have no source on new edges and refuse (`poly.attribute_conflict`).
pub(super) fn into_poly(
    source: &PolyMesh,
    result: &BeveledMesh,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    check_cancelled(context)?;
    let mut mesh = PolyMesh::new(
        result.positions.clone(),
        result.face_sizes.clone(),
        result.corners.clone(),
    )?;
    mesh.set_triangulation_mode(TriangulationMode::CurvedQuads);
    if !source.attributes().is_empty() {
        let sources = sources(source, result)?;
        transfer(source, &mut mesh, &sources, context)?;
    }
    tag_roles(&mut mesh, result)?;
    Ok(mesh)
}

/// Tags what an inset or extrusion made in the face attribute `role`, as the older inset and extrude
/// operators do: 1 for the inner face or moved cap, 2 for the ring or side walls. Faces the operator
/// did not make keep any `role` they had (0 when they had none). Nothing is written when the operator
/// made no tagged face, so other operators leave `role` alone.
fn tag_roles(mesh: &mut PolyMesh, result: &BeveledMesh) -> Result<(), GeometryError> {
    if result.face_role.iter().all(|&role| role == 0) {
        return Ok(());
    }
    let mut roles = match mesh
        .attributes()
        .get("role")
        .map(|attribute| &attribute.data)
    {
        Some(AttrData::U32(values)) if values.len() == result.face_role.len() => values.clone(),
        _ => vec![0; result.face_role.len()],
    };
    for (role, &made) in roles.iter_mut().zip(&result.face_role) {
        if made != 0 {
            *role = made;
        }
    }
    mesh.set_attribute(
        "role",
        Attribute {
            domain: Domain::Face,
            data: AttrData::U32(roles),
        },
    )
}

/// Where each output element's attributes come from, from the operator's provenance lists.
fn sources(mesh: &PolyMesh, result: &BeveledMesh) -> Result<Sources, GeometryError> {
    let unmapped = || refusal("poly.attribute_unmapped");
    let known = |origin: u32| {
        (origin != BeveledMesh::NO_ORIGIN)
            .then_some(origin as usize)
            .ok_or_else(unmapped)
    };
    let points = result
        .vertex_origin
        .iter()
        .map(|&origin| known(origin))
        .collect::<Result<Vec<_>, _>>()?;
    let faces = result
        .face_parent
        .iter()
        .map(|&parent| known(parent))
        .collect::<Result<Vec<_>, _>>()?;
    let starts = face_starts(mesh);
    let mut corners = Vec::with_capacity(result.corners.len());
    let mut offset = 0_usize;
    for (face, &size) in result.face_sizes.iter().enumerate() {
        let parent = *faces
            .get(face)
            .filter(|&&parent| parent < starts.len())
            .ok_or_else(unmapped)?;
        let first = starts[parent];
        let parent_size = mesh.face_sizes()[parent] as usize;
        let parent_corners = &mesh.corners()[first..first + parent_size];
        for &vertex in &result.corners[offset..offset + size as usize] {
            let origin = u32::try_from(points[vertex as usize]).map_err(|_| unmapped())?;
            // The parent corner at the vertex this one descends from, else the parent's first corner.
            let k = parent_corners
                .iter()
                .position(|&c| c == origin)
                .unwrap_or(0);
            corners.push(first + k);
        }
        offset += size as usize;
    }
    Ok(Sources {
        points,
        faces,
        corners,
    })
}

fn face_starts(mesh: &PolyMesh) -> Vec<usize> {
    let mut start = 0_usize;
    mesh.face_sizes()
        .iter()
        .map(|&size| {
            let here = start;
            start += size as usize;
            here
        })
        .collect()
}
