//! Catmull-Clark adapter from validated polygon cages to refined polygon meshes.
use std::collections::BTreeMap;

mod concave;

use ve_kernel::{
    CornerSharpness, Crease, ExecutionContext, KernelError, SubdivisionChannels,
    subdivide_catmark_with_channels,
};

use crate::{Element, GeometryError, PolyMesh, TriangulationMode, validate};

/// Subdivision parameters. Boundary values match `OpenSubdiv`'s documented Catmark rules.
#[derive(Clone, Debug, PartialEq)]
pub struct SubdivisionOptions {
    /// Uniform refinement levels, from one through four.
    pub levels: u32,
    /// 0 none, 1 edge-only, 2 edge-and-corner.
    pub boundary_rule: i32,
    /// 0 uniform or 1 Chaikin crease decay.
    pub crease_method: i32,
    /// Creased undirected cage edges with their sharpness values.
    pub creases: Vec<Crease>,
    /// Sharp cage vertices with their corner sharpness values.
    pub sharp_corners: Vec<CornerSharpness>,
}

/// Refined mesh and stable mapping back to cage elements.
#[derive(Clone, Debug, PartialEq)]
pub struct Subdivided {
    /// Newly allocated refined mesh; the cage remains unchanged.
    pub mesh: PolyMesh,
    /// Immediate parent element kind per refined vertex: vertex, edge, face.
    pub vertex_origin_kind: Vec<u32>,
    /// Parent element index; edge indices use sorted undirected cage-edge endpoints.
    pub vertex_origin_index: Vec<u32>,
    /// Cage face ordinal for each refined face.
    pub face_parent: Vec<u32>,
    /// Attributes intentionally dropped by the adapter, with their source names.
    pub dropped_attributes: Vec<String>,
    /// Number of concave cage faces split into triangles before refinement.
    pub triangulated_faces: usize,
}

/// Uniform Catmull-Clark refinement with explicit face provenance propagation. Integer point
/// attributes are reported as dropped; unsupported continuous/corner interpolation refuses.
///
/// # Errors
/// Returns a typed refusal for malformed/non-manifold topology, cancellation, kernel failures,
/// or attributes whose propagation has not been specified.
pub fn subdivide(
    cage: &PolyMesh,
    options: SubdivisionOptions,
    context: &ExecutionContext,
) -> Result<Subdivided, GeometryError> {
    let report = validate(cage, context)?;
    if let Some(&(a, b, _)) = report.nonmanifold_edges.first() {
        return Err(GeometryError::PolyRefusal {
            code: "poly.subdivide.nonmanifold",
            element: Some(Element::Edge(a, b)),
        });
    }
    if let Some(&(a, b)) = report.inconsistent_winding_edges.first() {
        return Err(GeometryError::PolyRefusal {
            code: "poly.subdivide.winding_conflict",
            element: Some(Element::Edge(a, b)),
        });
    }
    if context.is_cancelled() {
        return Err(GeometryError::Cancelled);
    }
    let (refinement_cage, triangulated_faces) = concave::triangulate(cage, context)?;
    let channels = collect_channels(&refinement_cage)?;
    let refined = subdivide_catmark_with_channels(
        refinement_cage.positions(),
        refinement_cage.face_sizes(),
        refinement_cage.corners(),
        &options.creases,
        &options.sharp_corners,
        SubdivisionChannels {
            point_channels: channels.point_names.len(),
            point_values: &channels.point_values,
            fvar_values: &channels.uv_values,
            fvar_corner_values: &channels.uv_corner_values,
        },
        options.levels,
        options.boundary_rule,
        options.crease_method,
        context,
    )
    .map_err(map_kernel_error)?;
    let ve_kernel::SubdivisionMesh {
        positions,
        face_sizes,
        corners,
        vertex_origin_kind,
        vertex_origin_index,
        face_parent,
        point_values,
        point_channels,
        fvar_values,
        fvar_corner_values,
    } = refined;
    let mut mesh = PolyMesh::new(positions, face_sizes, corners)?;
    mesh.set_triangulation_mode(TriangulationMode::CurvedQuads);
    let dropped_attributes = propagate_attributes(
        &refinement_cage,
        &face_parent,
        &point_values,
        point_channels,
        &fvar_values,
        &fvar_corner_values,
        &channels,
        &mut mesh,
    )?;
    Ok(Subdivided {
        mesh,
        vertex_origin_kind,
        vertex_origin_index,
        face_parent,
        dropped_attributes,
        triangulated_faces,
    })
}

struct ChannelData {
    point_names: Vec<String>,
    point_values: Vec<f32>,
    uv_values: Vec<[f32; 2]>,
    uv_corner_values: Vec<u32>,
}

fn collect_channels(cage: &PolyMesh) -> Result<ChannelData, GeometryError> {
    let u = cage.attributes().get("u");
    let v = cage.attributes().get("v");
    let mut result = ChannelData {
        point_names: Vec::new(),
        point_values: Vec::new(),
        uv_values: Vec::new(),
        uv_corner_values: Vec::new(),
    };
    if u.is_some() || v.is_some() {
        let valid_uvs = matches!(
            (u, v),
            (
                Some(crate::Attribute { domain: crate::Domain::Corner, data: crate::AttrData::F32(u_values) }),
                Some(crate::Attribute { domain: crate::Domain::Corner, data: crate::AttrData::F32(v_values) })
            ) if u_values.len() == cage.corners().len() && v_values.len() == cage.corners().len()
        );
        if !valid_uvs {
            return Err(refusal("poly.subdivide.uv_incomplete"));
        }
        collect_uvs(cage, u, v, &mut result)?;
    }
    let has_holes = cage
        .attributes()
        .get("hole")
        .is_some_and(|attribute| {
            attribute.domain == crate::Domain::Face
                && matches!(&attribute.data, crate::AttrData::U32(values) if values.iter().any(|value| *value != 0))
        });
    let has_edge = cage
        .attributes()
        .values()
        .any(|attribute| attribute.domain == crate::Domain::Edge);
    let has_corner_attribute = cage
        .attributes()
        .values()
        .any(|attribute| attribute.domain == crate::Domain::Corner);
    if has_holes {
        return Err(refusal("poly.subdivide.holes_unsupported"));
    }
    if has_edge {
        return Err(refusal("poly.subdivide.edge_attribute_unsupported"));
    }
    if has_corner_attribute
        && cage.attributes().iter().any(|(name, attribute)| {
            attribute.domain == crate::Domain::Corner && name != "u" && name != "v"
        })
    {
        return Err(refusal("poly.subdivide.corner_attribute_unsupported"));
    }
    let mut point_columns = Vec::new();
    for (name, attribute) in cage.attributes() {
        if attribute.domain != crate::Domain::Point {
            continue;
        }
        match &attribute.data {
            crate::AttrData::F32(values) => {
                if result.point_names.len() == 8 {
                    return Err(refusal("poly.subdivide.point_channel_limit"));
                }
                result.point_names.push(name.clone());
                point_columns.push(values.as_slice());
            }
            crate::AttrData::F64(_) => return Err(refusal("poly.subdivide.f64_unsupported")),
            crate::AttrData::I32(_) | crate::AttrData::U32(_) => {}
        }
    }
    result
        .point_values
        .reserve(cage.positions().len() * point_columns.len());
    for vertex in 0..cage.positions().len() {
        for column in &point_columns {
            result.point_values.push(column[vertex]);
        }
    }
    Ok(result)
}

fn collect_uvs(
    cage: &PolyMesh,
    u: Option<&crate::Attribute>,
    v: Option<&crate::Attribute>,
    result: &mut ChannelData,
) -> Result<(), GeometryError> {
    let (
        Some(crate::Attribute {
            data: crate::AttrData::F32(u_values),
            ..
        }),
        Some(crate::Attribute {
            data: crate::AttrData::F32(v_values),
            ..
        }),
    ) = (u, v)
    else {
        return Err(refusal("poly.subdivide.uv_incomplete"));
    };
    let mut indices = BTreeMap::<(u32, u32), u32>::new();
    for (&u_value, &v_value) in u_values.iter().zip(v_values) {
        let key = (u_value.to_bits(), v_value.to_bits());
        let next_index =
            u32::try_from(result.uv_values.len()).map_err(|_| GeometryError::PolyRefusal {
                code: "poly.subdivide.uv_value_limit",
                element: None,
            })?;
        let index = *indices.entry(key).or_insert_with(|| {
            result.uv_values.push([u_value, v_value]);
            next_index
        });
        result.uv_corner_values.push(index);
    }
    if result.uv_corner_values.len() != cage.corners().len() {
        return Err(refusal("poly.subdivide.uv_incomplete"));
    }
    Ok(())
}

fn refusal(code: &'static str) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: None,
    }
}

fn propagate_attributes(
    cage: &PolyMesh,
    face_parent: &[u32],
    point_values: &[f32],
    point_channels: usize,
    fvar_values: &[[f32; 2]],
    fvar_corner_values: &[u32],
    channels: &ChannelData,
    mesh: &mut PolyMesh,
) -> Result<Vec<String>, GeometryError> {
    let mut dropped_attributes = Vec::new();
    for (name, attribute) in cage.attributes() {
        match attribute.domain {
            crate::Domain::Face => {
                propagate_face_attribute(name, attribute, face_parent, mesh)?;
            }
            crate::Domain::Point
                if matches!(
                    &attribute.data,
                    crate::AttrData::I32(_) | crate::AttrData::U32(_)
                ) =>
            {
                dropped_attributes.push(name.clone());
            }
            crate::Domain::Point if matches!(&attribute.data, crate::AttrData::F32(_)) => {
                let Ok(channel) = channels
                    .point_names
                    .binary_search_by(|candidate| candidate.as_str().cmp(name))
                else {
                    return Err(refusal("poly.subdivide.point_channel_limit"));
                };
                let values = point_values
                    .chunks_exact(point_channels)
                    .map(|row| row[channel])
                    .collect();
                mesh.set_attribute(
                    name,
                    crate::Attribute {
                        domain: crate::Domain::Point,
                        data: crate::AttrData::F32(values),
                    },
                )?;
            }
            crate::Domain::Corner if name == "u" || name == "v" => {
                let coordinate = usize::from(name == "v");
                let values = fvar_corner_values
                    .iter()
                    .map(|&index| fvar_values[index as usize][coordinate])
                    .collect();
                mesh.set_attribute(
                    name,
                    crate::Attribute {
                        domain: crate::Domain::Corner,
                        data: crate::AttrData::F32(values),
                    },
                )?;
            }
            crate::Domain::Point
            | crate::Domain::Corner
            | crate::Domain::Edge
            | crate::Domain::CurvePoint
            | crate::Domain::Instance => {
                return Err(GeometryError::PolyRefusal {
                    code: "poly.subdivide.attribute_unsupported",
                    element: None,
                });
            }
        }
    }
    Ok(dropped_attributes)
}

fn propagate_face_attribute(
    name: &str,
    attribute: &crate::Attribute,
    face_parent: &[u32],
    mesh: &mut PolyMesh,
) -> Result<(), GeometryError> {
    let values = match &attribute.data {
        crate::AttrData::F32(values) => crate::AttrData::F32(
            face_parent
                .iter()
                .map(|&face| values[face as usize])
                .collect(),
        ),
        crate::AttrData::F64(values) => crate::AttrData::F64(
            face_parent
                .iter()
                .map(|&face| values[face as usize])
                .collect(),
        ),
        crate::AttrData::I32(values) => crate::AttrData::I32(
            face_parent
                .iter()
                .map(|&face| values[face as usize])
                .collect(),
        ),
        crate::AttrData::U32(values) => crate::AttrData::U32(
            face_parent
                .iter()
                .map(|&face| values[face as usize])
                .collect(),
        ),
    };
    mesh.set_attribute(
        name,
        crate::Attribute {
            domain: crate::Domain::Face,
            data: values,
        },
    )
}

fn map_kernel_error(error: KernelError) -> GeometryError {
    match error {
        KernelError::Cancelled => GeometryError::Cancelled,
        KernelError::AttributeLimit => refusal("poly.subdivide.point_channel_limit"),
        other => GeometryError::Kernel(other),
    }
}
