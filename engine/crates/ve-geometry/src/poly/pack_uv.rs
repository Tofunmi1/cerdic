//! Packs existing corner UV islands into the unit square.

use ve_kernel::{ExecutionContext, PolygonMesh, UvPackOptions};

use crate::{AttrData, Attribute, Domain, GeometryError, PolyMesh};

/// Packs corner attributes `u` and `v`, preserving the mesh and all other attributes.
pub fn pack_uv(
    mesh: &PolyMesh,
    options: &UvPackOptions,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    if context.is_cancelled() {
        return Err(GeometryError::Cancelled);
    }
    let u = match mesh.attributes().get("u") {
        Some(Attribute {
            domain: Domain::Corner,
            data: AttrData::F32(values),
        }) => values,
        _ => {
            return Err(GeometryError::InvalidMesh(
                "corner UV attributes u and v are required",
            ));
        }
    };
    let v = match mesh.attributes().get("v") {
        Some(Attribute {
            domain: Domain::Corner,
            data: AttrData::F32(values),
        }) => values,
        _ => {
            return Err(GeometryError::InvalidMesh(
                "corner UV attributes u and v are required",
            ));
        }
    };
    if u.len() != mesh.corners().len()
        || v.len() != u.len()
        || u.iter().chain(v).any(|x| !x.is_finite())
    {
        return Err(GeometryError::InvalidMesh(
            "corner UV values must be finite and match the corner count",
        ));
    }
    let input: Vec<[f32; 2]> = u
        .iter()
        .copied()
        .zip(v.iter().copied())
        .map(|(u, v)| [u, v])
        .collect();
    let view = PolygonMesh {
        positions: mesh.positions(),
        face_sizes: mesh.face_sizes(),
        corners: mesh.corners(),
    };
    let result = ve_kernel::pack_uvs(&view, &input, options)?;
    if context.is_cancelled() {
        return Err(GeometryError::Cancelled);
    }
    let mut packed = mesh.clone();
    for (name, axis) in [("u", 0), ("v", 1)] {
        packed.set_attribute(
            name,
            Attribute {
                domain: Domain::Corner,
                data: AttrData::F32(result.iter().map(|uv| uv[axis]).collect()),
            },
        )?;
    }
    Ok(packed)
}
