//! Reflections use a winding-corrected prototype and a positive placement matrix.
use super::moved_mesh;
use crate::{Fail, Part, cache::Value};
use glam::{DMat4, DVec3};
use ve_graph::Output;

pub(crate) fn parts(output: &Output, value: &Value) -> Result<Vec<Part>, Fail> {
    let Some(set) = &value.instances else {
        // One part per material region; an ordinary single-material solid is one part.
        return Ok(crate::regions::split(value)?
            .iter()
            .zip(0_u32..)
            .map(|(piece, region)| part(output, piece, (None, Vec::new()), region))
            .collect());
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
            prototype.frames = crate::frames::reflected(&prototype.frames, reflection)?;
        }
        let whole = prototype.volume;
        for (piece, region) in crate::regions::split(&prototype)?.iter().zip(0_u32..) {
            let placements = (Some(transforms.clone()), indices.clone());
            let mut result = part(output, piece, placements, region);
            // A placed set carries its tracks too: every copy plays them in its own frame, shifted by the track's phase.
            result.animation.clone_from(&value.animation);
            result.volume = if whole.abs() > f64::EPSILON {
                volume * piece.volume / whole
            } else {
                volume
            };
            results.push(result);
        }
    }
    Ok(results)
}
fn part(
    output: &Output,
    value: &Value,
    (transforms, instance_indices): (Option<Vec<[f32; 16]>>, Vec<u32>),
    region: u32,
) -> Part {
    Part {
        node: output.from.node.clone(),
        name: output.name.clone(),
        positions: value.mesh.positions.clone(),
        indices: value.mesh.indices.clone(),
        uvs: value.uvs.clone(),
        baked: value.baked.clone(),
        occlusion: Vec::new(),
        volume: value.volume,
        material: value.material.clone(),
        frames: value.frames.clone(),
        animation: value.animation.clone(),
        behavior: if transforms.is_none() {
            value.behavior.clone()
        } else {
            None
        },
        parent: if transforms.is_none() {
            value.parent.clone()
        } else {
            None
        },
        bone: if transforms.is_none() {
            value.bone
        } else {
            None
        },
        skin: if transforms.is_none() {
            value.skin.clone()
        } else {
            None
        },
        ik: if transforms.is_none() {
            value.ik.clone()
        } else {
            None
        },
        body: if transforms.is_none() {
            value.body.clone()
        } else {
            None
        },
        transforms,
        instance_indices,
        region,
    }
}
