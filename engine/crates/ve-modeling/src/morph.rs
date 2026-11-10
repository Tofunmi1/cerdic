//! Shape-key composition for equal-topology character meshes.
use std::rc::Rc;

use ve_geometry::PolyMesh;
use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, Mesh};

use crate::{Fail, MorphTarget, cache::Value, fail, fail_suggest};

pub(crate) const MAX_TARGETS: usize = 16;
const MAX_POINTS: usize = 500_000;

pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let base = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "rig.morph needs a base mesh"))?;
    let target = inputs
        .get(1)
        .ok_or_else(|| fail("graph.missing_input", "rig.morph needs a target mesh"))?;
    if base.instances.is_some() || target.instances.is_some() {
        return Err(fail(
            "rig.morph_instances",
            "shape keys must be authored before instancing",
        ));
    }
    if context.is_cancelled() {
        return Err(fail("eval.cancelled", "cancelled result discarded"));
    }
    let name = match node.params.get("name") {
        Some(Param::Str(name)) if !name.trim().is_empty() => name.trim(),
        _ => return Err(fail("param.range", "rig.morph name cannot be empty")),
    };
    let weight = match node.params.get("weight") {
        Some(Param::Num(value)) if value.is_finite() => *value as f32,
        Some(Param::Int(value)) => *value as f32,
        _ => {
            return Err(fail(
                "param.type",
                "rig.morph weight must be a finite number",
            ));
        }
    };
    if !(-1.0..=1.0).contains(&weight) {
        return Err(fail(
            "param.range",
            "rig.morph weight must be between -1 and 1",
        ));
    }
    if base.morph_targets.len() >= MAX_TARGETS {
        return Err(fail(
            "rig.morph_limit",
            format!("a mesh may have at most {MAX_TARGETS} shape keys"),
        ));
    }
    if base.morph_targets.iter().any(|target| target.name == name) {
        return Err(fail_suggest(
            "rig.morph_duplicate",
            format!("shape key {name:?} already exists on this mesh"),
            "choose a unique name for each shape key",
        ));
    }
    if !target.morph_targets.is_empty() {
        return Err(fail(
            "rig.morph_target_has_keys",
            "the target must be a plain edited mesh without its own shape keys",
        ));
    }

    let (base_points, target_points, poly_base) = match (&base.poly, &target.poly) {
        (Some(base_poly), Some(target_poly)) => {
            if base_poly.face_sizes() != target_poly.face_sizes()
                || base_poly.corners() != target_poly.corners()
                || base_poly.positions().len() != target_poly.positions().len()
            {
                return Err(topology_error());
            }
            (
                base_poly.positions().to_vec(),
                target_poly.positions().to_vec(),
                Some(Rc::clone(base_poly)),
            )
        }
        (None, None) => {
            if base.mesh.positions.len() != target.mesh.positions.len()
                || base.mesh.indices != target.mesh.indices
            {
                return Err(topology_error());
            }
            (
                base.mesh.positions.clone(),
                target.mesh.positions.clone(),
                None,
            )
        }
        _ => return Err(topology_error()),
    };
    if base_points.len() > MAX_POINTS {
        return Err(fail(
            "rig.morph_too_large",
            format!("shape keys support at most {MAX_POINTS} mesh points"),
        ));
    }
    if base
        .morph_targets
        .iter()
        .any(|morph| morph.positions.len() != base_points.len())
    {
        return Err(fail(
            "rig.morph_data_invalid",
            "an existing shape key no longer matches the mesh point count",
        ));
    }

    let mut neutral = base_points;
    for morph in &base.morph_targets {
        for (point, delta) in neutral.iter_mut().zip(&morph.positions) {
            for axis in 0..3 {
                point[axis] -= delta[axis] * morph.weight;
            }
        }
    }
    let delta: Vec<[f32; 3]> = target_points
        .iter()
        .zip(&neutral)
        .map(|(target, base)| std::array::from_fn(|axis| target[axis] - base[axis]))
        .collect();
    let mut current = neutral;
    for morph in &base.morph_targets {
        for (point, delta) in current.iter_mut().zip(&morph.positions) {
            for axis in 0..3 {
                point[axis] += delta[axis] * morph.weight;
            }
        }
    }
    for (point, delta) in current.iter_mut().zip(&delta) {
        for axis in 0..3 {
            point[axis] += delta[axis] * weight;
        }
    }
    if current.iter().flatten().any(|value| !value.is_finite()) {
        return Err(fail(
            "rig.morph_non_finite",
            "shape key produced a non-finite vertex",
        ));
    }

    let mut output = (**base).clone();
    if let Some(poly) = poly_base {
        let mut updated = PolyMesh::new(
            current.clone(),
            poly.face_sizes().to_vec(),
            poly.corners().to_vec(),
        )
        .map_err(|error| fail("poly.invalid_mesh", error.to_string()))?;
        updated.set_triangulation_mode(poly.triangulation_mode());
        for (name, attribute) in poly.attributes() {
            updated
                .set_attribute(name, attribute.clone())
                .map_err(|error| fail("poly.invalid_attribute", error.to_string()))?;
        }
        // Reuse the base triangulation. Sculpting can make a quad non-planar, which is valid for
        // a stable-topology shape key even though a fresh polygon triangulation would refuse it.
        output.mesh.positions = current.clone();
        output.volume = if base.volume == 0.0 {
            0.0
        } else {
            signed_volume(&output.mesh)
        };
        output.poly = Some(Rc::new(updated));
        output
            .human_source_indices
            .clone_from(&base.poly_human_source_indices);
        if let Some(rig) = &base.human {
            let mut updated_rig = (**rig).clone();
            updated_rig.update_surface(&base.poly_human_source_indices, &current);
            output.human = Some(Rc::new(updated_rig));
        }
    } else {
        output.mesh.positions = current;
        output.uvs.clone_from(&base.uvs);
    }
    output.morph_targets.push(MorphTarget {
        name: name.to_owned(),
        weight,
        positions: delta,
    });
    Ok(output)
}

fn signed_volume(mesh: &Mesh) -> f64 {
    let point = |index: u32| mesh.positions[index as usize].map(f64::from);
    mesh.indices
        .chunks_exact(3)
        .map(|triangle| {
            let (a, b, c) = (point(triangle[0]), point(triangle[1]), point(triangle[2]));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}

fn topology_error() -> Fail {
    fail_suggest(
        "rig.morph_topology",
        "shape-key meshes must have identical point counts and face topology",
        "make the target by duplicating the base mesh and editing only its vertex positions",
    )
}
