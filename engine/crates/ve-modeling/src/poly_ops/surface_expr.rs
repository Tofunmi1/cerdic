//! `poly.surface_expr`: a surface written as a program of two numbers, `u` and `v`, each from 0 to 1.

use ve_geometry::PolyMesh;
use ve_graph::Node;
use ve_kernel::ExecutionContext;
use ve_world::expr::{Bindings, Sample, Type, Value as Sampled};

use super::{
    convert::value_from_poly,
    errors::{poly_fail, refuse},
    params::count,
    program::compile,
};
use crate::{Fail, cache::Value};

pub(super) fn run(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let (columns, rows) = (count(node, "columns")?, count(node, "rows")?);
    let bindings = Bindings {
        fields: vec![("u".to_owned(), Type::Float), ("v".to_owned(), Type::Float)],
    };
    let expr = compile(node, &bindings, Type::Vec3)?;
    // The unit grid carries the quads and the corner UVs; it is centred, so x and y run -0.5..0.5 and u and v are each plus one half.
    let unit = ve_geometry::grid(1.0, 1.0, columns, rows).map_err(|e| poly_fail(node, &e, None))?;
    let mut moved = Vec::with_capacity(unit.positions().len());
    for position in unit.positions() {
        let (u, v) = (
            (position[0] + 0.5).clamp(0.0, 1.0),
            (position[1] + 0.5).clamp(0.0, 1.0),
        );
        let bound = [Sampled::Float(u), Sampled::Float(v)];
        let Sampled::Vec3(at) = expr.eval(&Sample {
            p: [u, v, 0.0],
            uv: [u, v],
            bound: &bound,
            ..Sample::default()
        }) else {
            return Err(refuse(
                node,
                "expr.error",
                "the program must give a vec3".to_owned(),
            ));
        };
        moved.push(at);
    }
    let flip = super::params::flag(node, "flip")?;
    // Which way a surface faces follows from its formula; `flip` reverses every face (and the UVs that ride on its corners).
    let mut corners = unit.corners().to_vec();
    let mut order: Vec<usize> = (0..corners.len()).collect();
    if flip {
        let mut at = 0;
        for &size in unit.face_sizes() {
            let end = at + size as usize;
            corners[at..end].reverse();
            order[at..end].reverse();
            at = end;
        }
    }
    let mut mesh = PolyMesh::new(moved, unit.face_sizes().to_vec(), corners)
        .map_err(|e| poly_fail(node, &e, None))?;
    for (name, attribute) in unit.attributes() {
        let mut attribute = attribute.clone();
        if flip && attribute.domain == ve_geometry::Domain::Corner {
            if let ve_geometry::AttrData::F32(values) = &mut attribute.data {
                *values = order.iter().map(|&k| values[k]).collect();
            }
        }
        mesh.set_attribute(name, attribute)
            .map_err(|e| poly_fail(node, &e, None))?;
    }
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    value_from_poly(node, mesh, None, context)
}
