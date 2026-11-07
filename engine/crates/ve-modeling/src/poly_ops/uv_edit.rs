//! Editing and viewing UV layouts: `poly.uv_transform` moves, scales and turns UVs (all of them, or those of the selected faces, which is how
//! an island is moved), and `poly.uv_layout` makes the layout itself a flat mesh so it can be looked at like any other geometry.

use std::rc::Rc;

use ve_geometry::{AttrData, Attribute, Domain, PolyMesh};
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::{
    convert::value_from_poly,
    errors::{poly_fail, refuse},
    params::{input_error, number, text},
    selection::{Elements, fingerprint},
};
use crate::{Fail, cache::Value};

/// The corner UVs of a mesh, or a refusal that says how to get them.
fn corner_uvs<'a>(node: &Node, mesh: &'a PolyMesh) -> Result<(&'a [f32], &'a [f32]), Fail> {
    let missing = || {
        refuse(
            node,
            "poly.uv_missing",
            "the mesh has no corner UVs; run poly.unwrap_uv first (poly.grid already has them)"
                .to_owned(),
        )
    };
    let (Some(u), Some(v)) = (mesh.attributes().get("u"), mesh.attributes().get("v")) else {
        return Err(missing());
    };
    match (&u.data, &v.data, u.domain, v.domain) {
        (AttrData::F32(u), AttrData::F32(v), Domain::Corner, Domain::Corner) => Ok((u, v)),
        _ => Err(missing()),
    }
}

/// Which corners belong to the faces `faces`, or every corner when `faces` is `None`.
fn corners_of(mesh: &PolyMesh, faces: Option<&[usize]>) -> Vec<usize> {
    let mut starts = Vec::with_capacity(mesh.face_sizes().len());
    let mut at = 0;
    for &size in mesh.face_sizes() {
        starts.push(at);
        at += size as usize;
    }
    match faces {
        None => (0..at).collect(),
        Some(faces) => faces
            .iter()
            .filter(|&&f| f < starts.len())
            .flat_map(|&f| starts[f]..starts[f] + mesh.face_sizes()[f] as usize)
            .collect(),
    }
}

pub(super) fn transform(
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
    let (u, v) = corner_uvs(node, poly)?;
    let faces: Option<Vec<usize>> = match inputs.get(1).and_then(|value| value.selection.as_deref())
    {
        None => None,
        Some(selection) => {
            if selection.fingerprint != fingerprint(poly) {
                return Err(refuse(
                    node,
                    "selection.stale",
                    "the selection was made on a different mesh than the one it is applied to"
                        .to_owned(),
                ));
            }
            match &selection.elements {
                Elements::Faces(items) => Some(items.clone()),
                other => {
                    return Err(refuse(
                        node,
                        "poly.selection_kind",
                        format!(
                            "needs a selection of faces (the island to move) but got {}s (set element on poly.select)",
                            other.kind()
                        ),
                    ));
                }
            }
        }
    };
    let region = corners_of(poly, faces.as_deref());
    let (offset_u, offset_v) = (number(node, "offset_u")?, number(node, "offset_v")?);
    let (scale, angle) = (number(node, "scale")?, number(node, "rotate")?);
    let pivot = if text(node, "pivot")? == "origin" || region.is_empty() {
        (0.0, 0.0)
    } else {
        let (mut low, mut high) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for &c in &region {
            let p = [f64::from(u[c]), f64::from(v[c])];
            for axis in 0..2 {
                low[axis] = low[axis].min(p[axis]);
                high[axis] = high[axis].max(p[axis]);
            }
        }
        ((low[0] + high[0]) * 0.5, (low[1] + high[1]) * 0.5)
    };
    let (sin, cos) = angle.sin_cos();
    let mut new_u = u.to_vec();
    let mut new_v = v.to_vec();
    for &c in &region {
        let (x, y) = (
            (f64::from(u[c]) - pivot.0) * scale,
            (f64::from(v[c]) - pivot.1) * scale,
        );
        let (x, y) = (x * cos - y * sin, x * sin + y * cos);
        #[expect(clippy::cast_possible_truncation, reason = "UVs are stored as f32")]
        {
            new_u[c] = (pivot.0 + x + offset_u) as f32;
            new_v[c] = (pivot.1 + y + offset_v) as f32;
        }
    }
    let mut mesh = poly.as_ref().clone();
    for (name, values) in [("u", new_u), ("v", new_v)] {
        mesh.set_attribute(
            name,
            Attribute {
                domain: Domain::Corner,
                data: AttrData::F32(values),
            },
        )
        .map_err(|error| poly_fail(node, &error, Some(poly)))?;
    }
    value_from_poly(node, mesh, input.material.clone(), context)
}

pub(super) fn layout(
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
    let (u, v) = corner_uvs(node, poly)?;
    let size = number(node, "size")?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "positions are stored as f32"
    )]
    let positions: Vec<[f32; 3]> = u
        .iter()
        .zip(v)
        .map(|(&u, &v)| {
            [
                (f64::from(u) * size) as f32,
                (f64::from(v) * size) as f32,
                0.0,
            ]
        })
        .collect();
    // Every corner is its own vertex, so islands stay apart and a seam shows as a gap.
    let corners: Vec<u32> = (0..u32::try_from(positions.len()).unwrap_or(0)).collect();
    let mut mesh = PolyMesh::new(positions, poly.face_sizes().to_vec(), corners)
        .map_err(|error| poly_fail(node, &error, Some(poly)))?;
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    value_from_poly(node, mesh, input.material.clone(), context)
}
