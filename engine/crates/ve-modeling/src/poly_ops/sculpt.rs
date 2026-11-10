//! `poly.sculpt`: brush strokes on a polygon mesh, written as text and applied in order to the vertices.
//!
//! A stroke is a brush, a centre, a radius and a strength (or, for the grab brush, a vector). Every vertex within the radius moves by the
//! brush's rule times a smooth falloff that is 1 at the centre and 0 at the radius, so a stroke never leaves a step at its edge. The
//! arithmetic is `+ - * /` and `sqrt` only, so the result is the same on every platform.

use std::rc::Rc;

use ve_geometry::{AttrData, Attribute, Domain, PolyMesh};
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::{
    convert::value_from_poly,
    errors::{poly_fail, refuse},
    params::{input_error, text},
    program::vertex_normals,
};
use crate::{Fail, cache::Value};

mod parse;
mod surface;
use parse::parse as parse_strokes;

/// The most strokes one node applies.
pub(super) const MAX_STROKES: usize = 64;
/// The most vertices the node works on (each stroke visits every vertex).
const MAX_VERTICES: usize = 500_000;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Brush {
    Draw,
    Inflate,
    Smooth,
    Flatten,
    Pinch,
    Grab,
    Mask,
    Unmask,
}

pub(super) struct Stroke {
    pub(super) brush: Brush,
    pub(super) center: [f32; 3],
    pub(super) radius: f32,
    /// Strength, or for the grab brush the move vector.
    pub(super) amount: [f32; 3],
}

/// 1 at the centre, 0 at the radius, smooth in between.
fn falloff(distance: f32, radius: f32) -> f32 {
    if distance >= radius {
        return 0.0;
    }
    let t = distance / radius;
    let s = 1.0 - t * t;
    s * s
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn length(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Applies one stroke to the positions. `normals` and `neighbours` describe the mesh as it was before this stroke.
fn sphere_reach(stroke: &Stroke, positions: &[[f32; 3]]) -> Vec<(usize, f32)> {
    positions
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let w = falloff(length(sub(*p, stroke.center)), stroke.radius);
            (w > 0.0).then_some((i, w))
        })
        .collect()
}

fn apply(
    stroke: &Stroke,
    positions: &mut [[f32; 3]],
    normals: &[[f32; 3]],
    neighbours: &[Vec<u32>],
    masks: &mut [f32],
    falloff_mode: &str,
    context: &ExecutionContext,
) -> Result<(), Fail> {
    let mut reach = if falloff_mode == "surface" {
        surface::reach(stroke.center, stroke.radius, positions, neighbours, context)?
    } else {
        sphere_reach(stroke, positions)
    };
    if !matches!(stroke.brush, Brush::Mask | Brush::Unmask) {
        for (index, weight) in &mut reach {
            *weight *= 1.0 - masks[*index].clamp(0.0, 1.0);
        }
        reach.retain(|(_, weight)| *weight > 0.0);
    }
    if reach.is_empty() {
        return Ok(());
    }
    let before: Vec<[f32; 3]> = reach.iter().map(|&(i, _)| positions[i]).collect();
    let strength = stroke.amount[0];
    match stroke.brush {
        Brush::Draw => {
            // One direction for the whole stroke, so the surface is pushed as a lump: the weighted average normal under the brush.
            let mut direction = [0.0_f32; 3];
            for &(i, w) in &reach {
                for axis in 0..3 {
                    direction[axis] += normals[i][axis] * w;
                }
            }
            let size = length(direction);
            if size > 1e-9 {
                for (&(i, w), start) in reach.iter().zip(&before) {
                    for axis in 0..3 {
                        positions[i][axis] = start[axis] + direction[axis] / size * strength * w;
                    }
                }
            }
        }
        Brush::Inflate => {
            for (&(i, w), start) in reach.iter().zip(&before) {
                for axis in 0..3 {
                    positions[i][axis] = start[axis] + normals[i][axis] * strength * w;
                }
            }
        }
        Brush::Grab => {
            for (&(i, w), start) in reach.iter().zip(&before) {
                for axis in 0..3 {
                    positions[i][axis] = start[axis] + stroke.amount[axis] * w;
                }
            }
        }
        Brush::Pinch => {
            let pull = strength.clamp(0.0, 1.0);
            for (&(i, w), start) in reach.iter().zip(&before) {
                for axis in 0..3 {
                    positions[i][axis] =
                        start[axis] + (stroke.center[axis] - start[axis]) * pull * w;
                }
            }
        }
        Brush::Smooth => {
            let blend = strength.clamp(0.0, 1.0);
            let snapshot = positions.to_vec();
            for &(i, w) in &reach {
                let around = &neighbours[i];
                if around.is_empty() {
                    continue;
                }
                let mut mean = [0.0_f32; 3];
                for &n in around {
                    for axis in 0..3 {
                        mean[axis] += snapshot[n as usize][axis];
                    }
                }
                let inverse = 1.0 / around.len() as f32;
                for axis in 0..3 {
                    positions[i][axis] =
                        snapshot[i][axis] + (mean[axis] * inverse - snapshot[i][axis]) * blend * w;
                }
            }
        }
        Brush::Flatten => {
            let blend = strength.clamp(0.0, 1.0);
            // The plane through the weighted centre of the points under the brush, facing their weighted average normal.
            let (mut centre, mut normal, mut total) = ([0.0_f32; 3], [0.0_f32; 3], 0.0_f32);
            for &(i, w) in &reach {
                for axis in 0..3 {
                    centre[axis] += positions[i][axis] * w;
                    normal[axis] += normals[i][axis] * w;
                }
                total += w;
            }
            let size = length(normal);
            if size > 1e-9 && total > 0.0 {
                for axis in 0..3 {
                    centre[axis] /= total;
                    normal[axis] /= size;
                }
                for (&(i, w), start) in reach.iter().zip(&before) {
                    let above = dot(sub(*start, centre), normal);
                    for axis in 0..3 {
                        positions[i][axis] = start[axis] - normal[axis] * above * blend * w;
                    }
                }
            }
        }
        Brush::Mask => {
            for &(index, weight) in &reach {
                masks[index] = (masks[index] + strength.clamp(0.0, 1.0) * weight).clamp(0.0, 1.0);
            }
        }
        Brush::Unmask => {
            for &(index, weight) in &reach {
                masks[index] = (masks[index] - strength.clamp(0.0, 1.0) * weight).clamp(0.0, 1.0);
            }
        }
    }
    Ok(())
}

pub(super) fn run(
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
    if poly.positions().len() > MAX_VERTICES {
        return Err(refuse(
            node,
            "poly.sculpt_too_large",
            format!(
                "the mesh has {} vertices; the most this node sculpts is {MAX_VERTICES}",
                poly.positions().len()
            ),
        ));
    }
    let mut strokes = parse_strokes(node, text(node, "strokes")?)?;
    let falloff_mode = text(node, "falloff")?;
    let axis = match text(node, "symmetry")? {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    };
    if let Some(axis) = axis {
        // Each stroke gets a mirror image about the plane through the origin.
        let mirrored: Vec<Stroke> = strokes
            .iter()
            .map(|s| {
                let mut center = s.center;
                let mut amount = s.amount;
                center[axis] = -center[axis];
                if s.brush == Brush::Grab {
                    amount[axis] = -amount[axis];
                }
                Stroke {
                    brush: s.brush,
                    center,
                    radius: s.radius,
                    amount,
                }
            })
            .collect();
        strokes.extend(mirrored);
    }
    let mut neighbours: Vec<Vec<u32>> = vec![Vec::new(); poly.positions().len()];
    let mut at = 0;
    for &size in poly.face_sizes() {
        let face = &poly.corners()[at..at + size as usize];
        at += size as usize;
        for (k, &a) in face.iter().enumerate() {
            let b = face[(k + 1) % face.len()];
            if !neighbours[a as usize].contains(&b) {
                neighbours[a as usize].push(b);
            }
            if !neighbours[b as usize].contains(&a) {
                neighbours[b as usize].push(a);
            }
        }
    }
    let mut positions = poly.positions().to_vec();
    let mut masks = match poly.attributes().get("sculpt_mask") {
        Some(Attribute {
            domain: Domain::Point,
            data: AttrData::F32(values),
        }) if values.len() == positions.len() && values.iter().all(|value| value.is_finite()) => {
            values.clone()
        }
        Some(_) => {
            return Err(refuse(
                node,
                "poly.sculpt_mask_attribute",
                "the existing sculpt_mask attribute must be one float per mesh vertex".to_owned(),
            ));
        }
        None => vec![0.0; positions.len()],
    };
    for stroke in &strokes {
        if context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "sculpt cancelled between strokes",
            ));
        }
        // Normals follow the surface as it is after the earlier strokes.
        let moved = PolyMesh::new(
            positions.clone(),
            poly.face_sizes().to_vec(),
            poly.corners().to_vec(),
        )
        .map_err(|error| poly_fail(node, &error, Some(poly)))?;
        let normals = vertex_normals(&moved);
        apply(
            stroke,
            &mut positions,
            &normals,
            &neighbours,
            &mut masks,
            falloff_mode,
            context,
        )?;
    }
    let mut mesh = PolyMesh::new(
        positions,
        poly.face_sizes().to_vec(),
        poly.corners().to_vec(),
    )
    .map_err(|error| poly_fail(node, &error, Some(poly)))?;
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    for (name, attribute) in poly.attributes() {
        if name == "sculpt_mask" {
            continue;
        }
        mesh.set_attribute(name, attribute.clone())
            .map_err(|error| poly_fail(node, &error, Some(poly)))?;
    }
    mesh.set_attribute(
        "sculpt_mask",
        Attribute {
            domain: Domain::Point,
            data: AttrData::F32(masks),
        },
    )
    .map_err(|error| poly_fail(node, &error, Some(poly)))?;
    let mut value = value_from_poly(node, mesh, input.material.clone(), context)?;
    // Sculpting changes positions but not vertex identity or topology. Keep
    // rig data and weights attached so this edit can stay in a character graph.
    value.human = input.human.as_ref().map(|rig| {
        let mut updated = (**rig).clone();
        updated.update_surface(
            &input.poly_human_source_indices,
            value.poly.as_ref().map_or(&[], |p| p.positions()),
        );
        Rc::new(updated)
    });
    value.skin.clone_from(&input.skin);
    value
        .human_source_indices
        .clone_from(&input.poly_human_source_indices);
    value
        .poly_human_source_indices
        .clone_from(&input.poly_human_source_indices);
    value.colors.clone_from(&input.colors);
    value.regions.clone_from(&input.regions);
    Ok(value)
}
