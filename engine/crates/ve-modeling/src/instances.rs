//! Shared prototypes, checked pattern composition and explicit realization.
use crate::{
    Fail,
    cache::Value,
    fail,
    operations::{kernel_fail, vector},
    profile_ops::{number, whole},
};
use glam::{DMat4, DVec3, EulerRot};
use std::rc::Rc;
use ve_graph::Node;
use ve_kernel::{ExecutionContext, Mesh, Solid};

mod output;
mod preflight;
pub(crate) use output::parts;
pub(crate) use preflight::estimate;
pub(crate) use preflight::realization;
pub(crate) const MAX_INSTANCES: usize = 16_384;

#[derive(Clone)]
pub(crate) struct InstanceSet {
    pub(crate) prototype: Rc<Value>,
    pub(crate) transforms: Vec<DMat4>,
}

pub(crate) fn rotation(angles: [f64; 3]) -> DMat4 {
    DMat4::from_euler(EulerRot::ZYX, angles[2], angles[1], angles[0])
}

pub(crate) fn execute(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "instance prototype missing"))?;
    let base = input.instances.clone().unwrap_or_else(|| InstanceSet {
        prototype: Rc::clone(input),
        transforms: vec![DMat4::IDENTITY],
    });
    if node.op.family().as_str() == "inst.realize" {
        return realize(node, &base, context);
    }
    let transforms = placements(node).map_err(|e| kernel_fail(node, &e))?;
    let count = preflight::product(base.transforms.len(), transforms.len())?;
    let corners = bounds_corners(&base.prototype.mesh);
    let mut composed = Vec::with_capacity(count);
    for outer in transforms {
        for inner in &base.transforms {
            let matrix = outer * *inner;
            if !matrix.is_finite() || matrix.determinant().abs() < 1e-12 {
                return Err(fail(
                    "kernel.invalid_input",
                    "instance matrix is non-finite or collapsed",
                ));
            }
            if corners
                .iter()
                .any(|p| matrix.transform_point3(*p).abs().max_element() > ve_kernel::MAX_EXTENT)
            {
                return Err(fail(
                    "kernel.invalid_input",
                    "placed bounds exceed the kernel extent",
                ));
            }
            composed.push(matrix);
        }
    }
    let mut value = Value::nothing();
    value.material.clone_from(&base.prototype.material);
    value.volume =
        base.prototype.volume * composed.iter().map(|m| m.determinant().abs()).sum::<f64>();
    value.instances = Some(InstanceSet {
        prototype: base.prototype,
        transforms: composed,
    });
    Ok(value)
}

fn bounds_corners(mesh: &Mesh) -> Vec<DVec3> {
    if mesh.is_empty() {
        return Vec::new();
    }
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = DVec3::splat(f64::NEG_INFINITY);
    for p in &mesh.positions {
        let p = DVec3::from_array(p.map(f64::from));
        min = min.min(p);
        max = max.max(p);
    }
    (0..8)
        .map(|i| {
            DVec3::new(
                if i & 1 == 0 { min.x } else { max.x },
                if i & 2 == 0 { min.y } else { max.y },
                if i & 4 == 0 { min.z } else { max.z },
            )
        })
        .collect()
}

pub(crate) fn placements(node: &Node) -> Result<Vec<DMat4>, ve_kernel::KernelError> {
    let translate = |v: [f64; 3]| DMat4::from_translation(DVec3::from_array(v));
    match node.op.family().as_str() {
        "xform.move" => Ok(vec![translate(vector(node, "by")?)]),
        "xform.rotate" => Ok(vec![rotation(vector(node, "angles")?)]),
        "xform.scale" => Ok(vec![DMat4::from_scale(DVec3::from_array(vector(
            node, "by",
        )?))]),
        "inst.mirror" | "xform.mirror" => {
            let axis = match node.params.get("axis") {
                Some(ve_graph::Value::Str(a)) if a == "x" => DVec3::new(-1.0, 1.0, 1.0),
                Some(ve_graph::Value::Str(a)) if a == "y" => DVec3::new(1.0, -1.0, 1.0),
                _ => DVec3::new(1.0, 1.0, -1.0),
            };
            Ok(vec![DMat4::IDENTITY, DMat4::from_scale(axis)])
        }
        "inst.array" | "xform.array" => {
            let spacing = vector(node, "spacing")?;
            let mut out = Vec::new();
            for x in 0..whole(node, "count_x")? {
                for y in 0..whole(node, "count_y")? {
                    for z in 0..whole(node, "count_z")? {
                        out.push(translate([
                            f64::from(x) * spacing[0],
                            f64::from(y) * spacing[1],
                            f64::from(z) * spacing[2],
                        ]));
                    }
                }
            }
            Ok(out)
        }
        "inst.radial" | "xform.radial" => {
            let count = whole(node, "count")?;
            let radius = number(node, "radius")?;
            Ok((0..count)
                .map(|i| {
                    DMat4::from_rotation_y(std::f64::consts::TAU * f64::from(i) / f64::from(count))
                        * translate([radius, 0.0, 0.0])
                })
                .collect())
        }
        _ => {
            let step = vector(node, "step")?;
            let turn = vector(node, "rotate_step")?;
            Ok((0..whole(node, "count")?)
                .map(|i| {
                    let n = f64::from(i);
                    translate(step.map(|v| v * n)) * rotation(turn.map(|v| v * n))
                })
                .collect())
        }
    }
}

pub(crate) fn moved_mesh(mesh: &Mesh, matrix: DMat4) -> Result<Mesh, Fail> {
    let positions = mesh
        .positions
        .iter()
        .map(|point| {
            let p = matrix.transform_point3(DVec3::from_array(point.map(f64::from)));
            if !p.is_finite() || p.abs().max_element() > ve_kernel::MAX_EXTENT {
                return Err(fail(
                    "kernel.invalid_input",
                    "placed mesh exceeds coordinate limits",
                ));
            }
            Ok(p.as_vec3().to_array())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut indices = mesh.indices.clone();
    if matrix.determinant() < 0.0 {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    Ok(Mesh { positions, indices })
}

fn realize(node: &Node, set: &InstanceSet, context: &ExecutionContext) -> Result<Value, Fail> {
    if set.prototype.mesh.is_empty() {
        return Ok(Value::nothing());
    }
    let mut solids = Vec::new();
    for matrix in &set.transforms {
        if context.is_cancelled() {
            return Err(fail("eval.cancelled", "realization cancelled"));
        }
        let mesh = moved_mesh(&set.prototype.mesh, *matrix)?;
        solids.push(Solid::from_mesh(&mesh).map_err(|e| kernel_fail(node, &e))?);
    }
    while solids.len() > 1 {
        let mut merged = Vec::new();
        for chunk in solids.chunks(64) {
            if chunk.len() == 1 {
                merged.push(
                    Solid::from_mesh(&chunk[0].to_mesh().map_err(|e| kernel_fail(node, &e))?)
                        .map_err(|e| kernel_fail(node, &e))?,
                );
            } else {
                merged.push(
                    Solid::union_all_with_context(&chunk.iter().collect::<Vec<_>>(), context)
                        .map_err(|e| kernel_fail(node, &e))?,
                );
            }
        }
        solids = merged;
    }
    let Some(solid) = solids.pop() else {
        return Ok(Value::nothing());
    };
    let mesh = if solid.is_empty() {
        Mesh::default()
    } else {
        solid.to_mesh().map_err(|e| kernel_fail(node, &e))?
    };
    let mut value = Value::solid(mesh, solid.volume());
    value.material.clone_from(&set.prototype.material);
    value.frames = crate::frames::copies(&set.prototype.frames, &set.transforms, false)?;
    Ok(value)
}
