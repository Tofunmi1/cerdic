//! Polygon sources and the conversions between solids and polygon meshes.

use std::rc::Rc;

use ve_geometry::{AttrData, Attribute, Domain, LossPolicy, PolyMesh};
use ve_graph::Node;
use ve_kernel::{ExecutionContext, Mesh, Solid};

use super::errors::poly_fail;
use super::params::{count, input_error, number};
use crate::operations::kernel_fail;
use crate::{Fail, Material, cache::Value};

/// `poly.grid`: a flat quad grid.
pub(super) fn grid(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let (width, height) = (number(node, "width")?, number(node, "height")?);
    let (columns, rows) = (count(node, "columns")?, count(node, "rows")?);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "sizes are bounded to 1e6 by the parameter type and stored as f32"
    )]
    let mesh = ve_geometry::grid(width as f32, height as f32, columns, rows)
        .map_err(|e| poly_fail(node, &e, None))?;
    value_from_poly(node, mesh, None, context)
}

/// `poly.from_solid`: the solid's triangles as an editable mesh. A solid made of several colours
/// keeps them as a `region` face attribute so a selection can ask for one colour.
pub(super) fn from_solid(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs.first().ok_or_else(|| input_error(node, "a solid"))?;
    if input.mesh.is_empty() {
        return Ok(Value::nothing());
    }
    let mut mesh =
        PolyMesh::from_kernel_mesh(&input.mesh).map_err(|e| poly_fail(node, &e, None))?;
    if input.regions.len() >= 2 {
        let triangles = input.mesh.indices.len() / 3;
        let mut region = vec![0_u32; triangles];
        for (index, start) in input.regions.iter().enumerate() {
            let end = input
                .regions
                .get(index + 1)
                .map_or(triangles, |next| next.first_triangle);
            let label = u32::try_from(index).unwrap_or(u32::MAX);
            for slot in region
                .get_mut(start.first_triangle..end)
                .unwrap_or_default()
            {
                *slot = label;
            }
        }
        mesh.set_attribute(
            "region",
            Attribute {
                domain: Domain::Face,
                data: AttrData::U32(region),
            },
        )
        .map_err(|e| poly_fail(node, &e, None))?;
    }
    let mut value = value_from_poly(node, mesh, input.material.clone(), context)?;
    value.regions.clone_from(&input.regions);
    Ok(value)
}

/// `poly.to_solid`: a closed polygon mesh back to a solid; the kernel refuses anything else.
pub(super) fn to_solid(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| input_error(node, "a polygon mesh"))?;
    let Some(poly) = input.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    if input.volume < 0.0 {
        let mut value = Value::solid(input.mesh.clone(), 0.0);
        value.material.clone_from(&input.material);
        value.uvs.clone_from(&input.uvs);
        return Ok(value);
    }
    let (kernel, _) = poly
        .to_kernel_mesh_with_context(LossPolicy::Report, context)
        .map_err(|e| poly_fail(node, &e, Some(poly)))?;
    let solid = Solid::from_mesh(&kernel).map_err(|e| {
        // An open or pinched mesh is the usual refusal here, and the usual cause is an edit that left a seam or a hole.
        if e.to_string().contains("closed manifold") {
            super::errors::refuse(
                node,
                "poly.not_closed",
                "the mesh is not closed: every edge must join exactly two faces".to_owned(),
            )
        } else {
            kernel_fail(node, &e)
        }
    })?;
    let mut value = Value::solid(kernel, solid.volume());
    value.material.clone_from(&input.material);
    value.uvs = corner_uvs(poly, context)?;
    Ok(value)
}

/// A value holding `poly` and its triangles. The volume is the enclosed volume when the mesh is a
/// closed, consistently wound manifold and zero otherwise (an open mesh encloses nothing).
pub(crate) fn value_from_poly(
    node: &Node,
    poly: PolyMesh,
    material: Option<Material>,
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let (mesh, _) = poly
        .to_kernel_mesh_with_context(LossPolicy::Report, context)
        .map_err(|e| poly_fail(node, &e, Some(&poly)))?;
    let report =
        ve_geometry::validate(&poly, context).map_err(|e| poly_fail(node, &e, Some(&poly)))?;
    let closed = report.boundary_loops == 0
        && report.unclosed_boundary_edges == 0
        && report.nonmanifold_edges.is_empty()
        && report.inconsistent_winding_edges.is_empty();
    let volume = if closed { signed_volume(&mesh) } else { 0.0 };
    let mut value = Value::solid(mesh, volume);
    value.material = material;
    value.uvs = corner_uvs(&poly, context)?;
    value.poly = Some(Rc::new(poly));
    Ok(value)
}

fn corner_uvs(poly: &PolyMesh, context: &ExecutionContext) -> Result<Vec<[f32; 2]>, Fail> {
    let Some(u) = poly.attributes().get("u") else {
        return Ok(Vec::new());
    };
    let Some(v) = poly.attributes().get("v") else {
        return Ok(Vec::new());
    };
    let (
        ve_geometry::Domain::Corner,
        ve_geometry::AttrData::F32(u),
        ve_geometry::Domain::Corner,
        ve_geometry::AttrData::F32(v),
    ) = (u.domain, &u.data, v.domain, &v.data)
    else {
        return Ok(Vec::new());
    };
    let indices = poly
        .triangulated_corner_indices(context)
        .map_err(|error| poly_fail_for_uv(&error))?;
    Ok(indices
        .into_iter()
        .filter_map(|corner| Some([*u.get(corner)?, *v.get(corner)?]))
        .collect())
}

fn poly_fail_for_uv(error: &ve_geometry::GeometryError) -> Fail {
    Box::new(ve_graph::Diagnostic::error(
        "poly.uv_attribute_invalid",
        error.to_string(),
    ))
}

fn signed_volume(mesh: &Mesh) -> f64 {
    let point = |index: u32| mesh.positions[index as usize].map(f64::from);
    mesh.indices
        .chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (point(t[0]), point(t[1]), point(t[2]));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}
