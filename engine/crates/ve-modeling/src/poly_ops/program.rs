//! `poly.program`: a field-expression program run once for every vertex of a polygon mesh.

use std::rc::Rc;

use ve_geometry::{AttrData, Attribute, Domain, PolyMesh};
use ve_graph::Node;
use ve_kernel::ExecutionContext;
use ve_world::expr::{Bindings, Expr, Sample, Type, Value as Sampled};

use super::{
    convert::value_from_poly,
    errors::{poly_fail, refuse},
    params::{input_error, text},
};
use crate::{Fail, cache::Value};

pub(super) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| input_error(node, "a polygon mesh"))?;
    let Some(source) = input.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    let mode = text(node, "mode")?;
    let wanted = if matches!(mode, "position" | "offset") {
        Type::Vec3
    } else {
        Type::Float
    };
    let bindings = Bindings {
        fields: vec![
            ("i".to_owned(), Type::Float),
            ("count".to_owned(), Type::Float),
        ],
    };
    let expr = compile(node, &bindings, wanted)?;
    let layers = referenced_layers(node, source, &expr)?;
    let normals = vertex_normals(source);
    let uvs = vertex_uvs(source);
    let count = source.positions().len();
    #[expect(
        clippy::cast_precision_loss,
        reason = "vertex numbers are far below 2^24"
    )]
    let total = count as f32;
    let mut moved = Vec::with_capacity(count);
    let mut numbers = Vec::new();
    for (index, position) in source.positions().iter().enumerate() {
        let attrs: Vec<(&str, Sampled)> = layers
            .iter()
            .map(|(name, values)| (name.as_str(), Sampled::Float(values[index])))
            .collect();
        #[expect(
            clippy::cast_precision_loss,
            reason = "vertex numbers are far below 2^24"
        )]
        let bound = [Sampled::Float(index as f32), Sampled::Float(total)];
        let result = expr.eval(&Sample {
            p: *position,
            n: normals[index],
            uv: uvs[index],
            attrs: &attrs,
            bound: &bound,
        });
        match (mode, result) {
            ("position", Sampled::Vec3(p)) => moved.push(p),
            ("offset", Sampled::Vec3(d)) => {
                moved.push([position[0] + d[0], position[1] + d[1], position[2] + d[2]]);
            }
            ("along_normal", Sampled::Float(d)) => {
                let n = normals[index];
                moved.push([
                    position[0] + n[0] * d,
                    position[1] + n[1] * d,
                    position[2] + n[2] * d,
                ]);
            }
            ("layer", Sampled::Float(v)) => {
                moved.push(*position);
                numbers.push(v);
            }
            _ => {}
        }
    }
    let mut mesh = PolyMesh::new(
        moved,
        source.face_sizes().to_vec(),
        source.corners().to_vec(),
    )
    .map_err(|e| poly_fail(node, &e, Some(source)))?;
    // Moving vertices bends quads, so a program's result is drawn as a surface (split along the shorter diagonal), like terrain.
    mesh.set_triangulation_mode(if mode == "layer" {
        source.triangulation_mode()
    } else {
        ve_geometry::TriangulationMode::CurvedQuads
    });
    for (name, attribute) in source.attributes() {
        mesh.set_attribute(name, attribute.clone())
            .map_err(|e| poly_fail(node, &e, Some(source)))?;
    }
    if mode == "layer" {
        let name = text(node, "layer_name")?;
        mesh.set_attribute(
            name,
            Attribute {
                domain: Domain::Point,
                data: AttrData::F32(numbers),
            },
        )
        .map_err(|e| poly_fail(node, &e, Some(source)))?;
    }
    value_from_poly(node, mesh, input.material.clone(), context)
}

/// Compiles the node's `code` against `bindings`, wanting a result of type `wanted`; a mistake names its line, column and fix.
pub(crate) fn compile(node: &Node, bindings: &Bindings, wanted: Type) -> Result<Expr, Fail> {
    compile_param(node, "code", bindings, wanted)
}

/// Like [`compile`], for the program in the parameter called `name`.
pub(crate) fn compile_param(
    node: &Node,
    name: &str,
    bindings: &Bindings,
    wanted: Type,
) -> Result<Expr, Fail> {
    Expr::compile_as(text(node, name)?, bindings, wanted).map_err(|found| {
        let shown: Vec<String> = found
            .iter()
            .take(3)
            .map(|d| format!("line {}, column {}: {}", d.line, d.column, d.message))
            .collect();
        let fix = found
            .first()
            .map(|d| d.suggestion.clone())
            .unwrap_or_default();
        refuse(
            node,
            "expr.error",
            format!("the program is wrong: {}. {fix}", shown.join("; ")),
        )
    })
}

/// The per-vertex number layers the program reads with `attr("name")`, by name.
fn referenced_layers(
    node: &Node,
    mesh: &PolyMesh,
    expr: &Expr,
) -> Result<Vec<(String, Vec<f32>)>, Fail> {
    expr.attributes()
        .iter()
        .map(|(name, ty)| {
            let found = mesh.attributes().get(name);
            match (ty, found) {
                (Type::Float, Some(Attribute { domain: Domain::Point, data: AttrData::F32(values) })) => {
                    Ok((name.clone(), values.clone()))
                }
                (Type::Float, _) => Err(refuse(
                    node,
                    "expr.layer_missing",
                    format!(
                        "the program reads attr(\"{name}\") but the mesh has no per-vertex number layer with that name; layers here: {}",
                        mesh.attributes().keys().cloned().collect::<Vec<_>>().join(", ")
                    ),
                )),
                _ => Err(refuse(
                    node,
                    "expr.layer_type",
                    format!("only attr(\"{name}\") with a single number is supported here; use attr, not attr2/attr3/attr4"),
                )),
            }
        })
        .collect()
}

/// Smoothed vertex normals: each face adds its area-weighted normal to its vertices.
pub(super) fn vertex_normals(mesh: &PolyMesh) -> Vec<[f32; 3]> {
    let positions = mesh.positions();
    let mut sums = vec![[0.0_f32; 3]; positions.len()];
    let mut at = 0;
    for &size in mesh.face_sizes() {
        let corners = &mesh.corners()[at..at + size as usize];
        at += size as usize;
        // Newell's method: the face's area-weighted normal, for any polygon.
        let mut normal = [0.0_f32; 3];
        for (k, &a) in corners.iter().enumerate() {
            let b = corners[(k + 1) % corners.len()];
            let (p, q) = (positions[a as usize], positions[b as usize]);
            normal[0] += (p[1] - q[1]) * (p[2] + q[2]);
            normal[1] += (p[2] - q[2]) * (p[0] + q[0]);
            normal[2] += (p[0] - q[0]) * (p[1] + q[1]);
        }
        for &c in corners {
            for axis in 0..3 {
                sums[c as usize][axis] += normal[axis];
            }
        }
    }
    sums.iter()
        .map(|n| {
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if length > 0.0 {
                [n[0] / length, n[1] / length, n[2] / length]
            } else {
                [0.0, 1.0, 0.0]
            }
        })
        .collect()
}

/// Each vertex's first corner UV (zero when the mesh has none).
fn vertex_uvs(mesh: &PolyMesh) -> Vec<[f32; 2]> {
    let mut uvs = vec![[0.0_f32; 2]; mesh.positions().len()];
    let (Some(u), Some(v)) = (mesh.attributes().get("u"), mesh.attributes().get("v")) else {
        return uvs;
    };
    let (AttrData::F32(u), AttrData::F32(v)) = (&u.data, &v.data) else {
        return uvs;
    };
    let mut seen = vec![false; uvs.len()];
    for (corner, &vertex) in mesh.corners().iter().enumerate() {
        let vertex = vertex as usize;
        if !seen[vertex] {
            seen[vertex] = true;
            uvs[vertex] = [
                u.get(corner).copied().unwrap_or(0.0),
                v.get(corner).copied().unwrap_or(0.0),
            ];
        }
    }
    uvs
}
