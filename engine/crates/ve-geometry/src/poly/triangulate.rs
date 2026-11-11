//! Triangulates polygon faces using the geometry crate's deterministic ear clipping.

use ve_kernel::ExecutionContext;

use crate::{AttrData, Attribute, Domain, GeometryError, PolyMesh};

/// Splits every polygon face into triangles while preserving point, face and corner attributes.
/// Edge attributes refuse because triangulation introduces diagonals with no source edge value.
pub fn triangulate(mesh: &PolyMesh, context: &ExecutionContext) -> Result<PolyMesh, GeometryError> {
    if mesh
        .attributes()
        .values()
        .any(|attribute| attribute.domain == Domain::Edge)
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            element: None,
        });
    }

    let mut corners = Vec::new();
    let mut corner_sources = Vec::new();
    let mut face_sources = Vec::new();
    let mut start = 0_usize;
    for (face_index, &size) in mesh.face_sizes().iter().enumerate() {
        if context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        let size = usize::try_from(size)
            .map_err(|_| GeometryError::InvalidMesh("face size overflows usize"))?;
        let face = mesh
            .corners()
            .get(start..start + size)
            .ok_or(GeometryError::InvalidMesh("face range is outside corners"))?;
        for triangle in crate::triangulate::face(mesh, face, mesh.triangulation_mode(), context)? {
            for vertex in triangle {
                let local = face
                    .iter()
                    .position(|&candidate| candidate == vertex)
                    .ok_or(GeometryError::InvalidMesh(
                        "triangulation returned a foreign corner",
                    ))?;
                corners.push(vertex);
                corner_sources.push(start + local);
            }
            face_sources.push(face_index);
        }
        start += size;
    }

    let mut output = PolyMesh::new(
        mesh.positions().to_vec(),
        vec![3; face_sources.len()],
        corners,
    )?;
    for (name, attribute) in mesh.attributes() {
        let data = match attribute.domain {
            Domain::Point => attribute.data.clone(),
            Domain::Face => select(&attribute.data, &face_sources),
            Domain::Corner => select(&attribute.data, &corner_sources),
            Domain::Edge => unreachable!("edge attributes were refused above"),
            Domain::CurvePoint | Domain::Instance => {
                return Err(GeometryError::DomainNotOnMesh(attribute.domain));
            }
        };
        output.set_attribute(
            name,
            Attribute {
                domain: attribute.domain,
                data,
            },
        )?;
    }
    Ok(output)
}

fn select(data: &AttrData, indices: &[usize]) -> AttrData {
    match data {
        AttrData::F32(values) => AttrData::F32(indices.iter().map(|&i| values[i]).collect()),
        AttrData::F64(values) => AttrData::F64(indices.iter().map(|&i| values[i]).collect()),
        AttrData::I32(values) => AttrData::I32(indices.iter().map(|&i| values[i]).collect()),
        AttrData::U32(values) => AttrData::U32(indices.iter().map(|&i| values[i]).collect()),
    }
}
