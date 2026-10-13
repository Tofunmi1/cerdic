//! Reflections use a winding-corrected prototype and a positive placement matrix.
use super::moved_mesh;
use crate::{Fail, Part, cache::Value};
use glam::{DMat4, DVec3};
use ve_graph::Output;

pub(crate) fn parts(output: &Output, value: &Value) -> Result<Vec<Part>, Fail> {
    let Some(set) = &value.instances else {
        return Ok(vec![part(output, value, None, Vec::new())]);
    };
    let reflection = DMat4::from_scale(DVec3::new(-1.0, 1.0, 1.0));
    let mut results = Vec::new();
    for mirrored in [false, true] {
        let mut transforms = Vec::new();
        let mut indices = Vec::new();
        let mut volume = 0.0;
        for (index, matrix) in set.transforms.iter().enumerate() {
            if (matrix.determinant() < 0.0) != mirrored {
                continue;
            }
            let draw = if mirrored {
                *matrix * reflection
            } else {
                *matrix
            };
            transforms.push(draw.as_mat4().to_cols_array());
            indices.push(
                u32::try_from(index)
                    .map_err(|_| crate::fail("budget.instances", "index overflow"))?,
            );
            volume += set.prototype.volume * matrix.determinant().abs();
        }
        if transforms.is_empty() {
            continue;
        }
        let mut prototype = (*set.prototype).clone();
        if mirrored {
            prototype.mesh = moved_mesh(&prototype.mesh, reflection)?;
        }
        let mut result = part(output, &prototype, Some(transforms), indices);
        result.volume = volume;
        results.push(result);
    }
    Ok(results)
}
fn part(
    output: &Output,
    value: &Value,
    transforms: Option<Vec<[f32; 16]>>,
    instance_indices: Vec<u32>,
) -> Part {
    Part {
        node: output.from.node.clone(),
        name: output.name.clone(),
        positions: value.mesh.positions.clone(),
        indices: value.mesh.indices.clone(),
        volume: value.volume,
        material: value.material.clone(),
        transforms,
        instance_indices,
    }
}
