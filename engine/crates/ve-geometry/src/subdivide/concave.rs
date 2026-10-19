use ve_kernel::ExecutionContext;

use crate::{AttrData, Attribute, Domain, GeometryError, PolyMesh, TriangulationMode};

pub(super) fn triangulate(
    cage: &PolyMesh,
    context: &ExecutionContext,
) -> Result<(PolyMesh, usize), GeometryError> {
    let mut faces = Vec::new();
    let mut sizes = Vec::new();
    let mut source_faces = Vec::new();
    let mut source_corners = Vec::new();
    let mut start = 0;
    let mut changed = 0;
    for (face_index, &size) in cage.face_sizes().iter().enumerate() {
        if context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        let end = start + size as usize;
        let corners = &cage.corners()[start..end];
        if size > 3 && !crate::triangulate::is_convex_face(cage, corners, context)? {
            changed += 1;
            for triangle in
                crate::triangulate::face(cage, corners, cage.triangulation_mode(), context)?
            {
                sizes.push(3);
                faces.extend(triangle);
                source_faces.push(face_index);
                for vertex in triangle {
                    let local = corners
                        .iter()
                        .position(|candidate| *candidate == vertex)
                        .ok_or(GeometryError::InvalidMesh(
                            "triangulation returned an unknown corner",
                        ))?;
                    source_corners.push(start + local);
                }
            }
        } else {
            sizes.push(size);
            faces.extend_from_slice(corners);
            source_faces.push(face_index);
            source_corners.extend(start..end);
        }
        start = end;
    }
    if changed == 0 {
        return Ok((cage.clone(), 0));
    }
    let mut result = PolyMesh::new(cage.positions().to_vec(), sizes, faces)?;
    result.set_triangulation_mode(TriangulationMode::CurvedQuads);
    for (name, attribute) in cage.attributes() {
        let data = match attribute.domain {
            Domain::Point => attribute.data.clone(),
            Domain::Face => select(&attribute.data, &source_faces),
            Domain::Corner => select(&attribute.data, &source_corners),
            Domain::Edge | Domain::CurvePoint | Domain::Instance => {
                return Err(GeometryError::PolyRefusal {
                    code: "poly.subdivide.attribute_unsupported",
                    element: None,
                });
            }
        };
        result.set_attribute(
            name,
            Attribute {
                domain: attribute.domain,
                data,
            },
        )?;
    }
    Ok((result, changed))
}

fn select(data: &AttrData, indices: &[usize]) -> AttrData {
    match data {
        AttrData::F32(values) => {
            AttrData::F32(indices.iter().map(|&index| values[index]).collect())
        }
        AttrData::F64(values) => {
            AttrData::F64(indices.iter().map(|&index| values[index]).collect())
        }
        AttrData::I32(values) => {
            AttrData::I32(indices.iter().map(|&index| values[index]).collect())
        }
        AttrData::U32(values) => {
            AttrData::U32(indices.iter().map(|&index| values[index]).collect())
        }
    }
}
