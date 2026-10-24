//! GPU skinning for the Bevy renderer. `skinning.rs` deforms a skinned part's mesh on the CPU every frame, which caps a skin at 131,072
//! vertices and costs CPU time in proportion; here the mesh keeps its rest pose and carries per-corner joint indices and weights, and
//! Bevy's own skinning moves it on the GPU from joint entities. Each joint entity's transform is the bone's world pose for the frame
//! (`PartPoses`), with an identity inverse bind pose: the part's vertices are already in world space at rest, so the skin matrix is
//! just the pose, which is what the CPU path applies too. Dual-quaternion skins keep the CPU path (Bevy blends linearly only).
//!
//! A mesh's skinned-ness must be fixed from the first frame it is drawn: Bevy caches the pipeline made for a mesh layout, so a mesh that
//! gains joint attributes later (or an entity that gains its `SkinnedMesh` later) is drawn with the wrong bind group. So the mesh is born
//! with its attributes (`add_joint_attributes`, called from the upload), the entity is spawned hidden, and `attach_gpu_skins` gives it its
//! `SkinnedMesh` and shows it in one step.
use std::collections::HashMap;
use std::sync::Arc;

use bevy_asset::{Assets, Handle};
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::Without,
    system::{Commands, Local, Query, Res, ResMut},
};
use bevy_math::Mat4;
use bevy_mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy_mesh::{Mesh, VertexAttributeValues};
use bevy_transform::components::{GlobalTransform, Transform};
use ve_modeling::rig::Blend;

use crate::animation::PartPoses;
use crate::skinning::{GpuSkinned, SkinData, Skinned};

/// A joint of a GPU skin: it follows the pose of the bone part with this output name.
#[derive(Component)]
pub(crate) struct SkinJoint(String);

/// The joints and bind poses shared by every chunk of one skinned part.
pub(crate) struct Rig {
    joints: Vec<Entity>,
    bindposes: Handle<SkinnedMeshInverseBindposes>,
}

/// Writes the joint indices, joint weights and the vertex-averaged outline normal of one chunk (`rest` is six floats per corner,
/// `corner_offset` where the chunk starts in the part's corner list) into its mesh.
pub(crate) fn add_joint_attributes(
    mesh: &mut Mesh,
    data: &SkinData,
    corner_offset: usize,
    rest: &[f32],
) {
    let corners = rest.len() / 6;
    let mut joints = Vec::with_capacity(corners);
    let mut weights = Vec::with_capacity(corners);
    for corner in 0..corners {
        let Some(&vertex) = data.indices.get(corner_offset + corner) else {
            return;
        };
        let at = vertex as usize * 4;
        let (Some(j), Some(w)) = (data.joints.get(at..at + 4), data.weights.get(at..at + 4)) else {
            return;
        };
        #[expect(
            clippy::cast_possible_truncation,
            reason = "joint indices are below 32"
        )]
        joints.push([j[0] as u16, j[1] as u16, j[2] as u16, j[3] as u16]);
        weights.push([w[0], w[1], w[2], w[3]]);
    }
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(joints),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
    mesh.insert_attribute(
        ve_render::ATTRIBUTE_HULL_NORMAL,
        smooth_rest_normals(data, corner_offset, rest),
    );
}

/// The rest normal of every corner averaged over the corners of the chunk that share its vertex.
fn smooth_rest_normals(data: &SkinData, corner_offset: usize, rest: &[f32]) -> Vec<[f32; 3]> {
    let corners = rest.len() / 6;
    let vertex_of = |corner: usize| data.indices.get(corner_offset + corner).copied();
    let mut sums: HashMap<u32, [f32; 3]> = HashMap::new();
    for corner in 0..corners {
        if let Some(vertex) = vertex_of(corner) {
            let sum = sums.entry(vertex).or_insert([0.0; 3]);
            for axis in 0..3 {
                sum[axis] += rest[corner * 6 + 3 + axis];
            }
        }
    }
    (0..corners)
        .map(|corner| {
            let sum = vertex_of(corner)
                .and_then(|vertex| sums.get(&vertex))
                .copied()
                .unwrap_or([0.0, 1.0, 0.0]);
            let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            let scale = if length > 1.0e-6 { 1.0 / length } else { 0.0 };
            [sum[0] * scale, sum[1] * scale, sum[2] * scale]
        })
        .collect()
}

/// Gives every new linear skinned chunk its `SkinnedMesh` (and shows it), and stops the CPU from deforming it. The projection spawned the
/// chunk hidden (see the module docs).
pub(crate) fn attach_gpu_skins(
    mut commands: Commands,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut rigs: Local<HashMap<usize, Rig>>,
    parts: Query<(Entity, &Skinned), Without<GpuSkinned>>,
) {
    for (entity, skinned) in &parts {
        if skinned.data.blend != Blend::Linear {
            continue;
        }
        let key = Arc::as_ptr(&skinned.data) as usize;
        let rig = rigs.entry(key).or_insert_with(|| {
            let joints = skinned
                .data
                .bones
                .iter()
                .map(|name| {
                    commands
                        .spawn((
                            SkinJoint(name.clone()),
                            Transform::IDENTITY,
                            GlobalTransform::IDENTITY,
                        ))
                        .id()
                })
                .collect();
            Rig {
                joints,
                bindposes: bindposes.add(vec![Mat4::IDENTITY; skinned.data.bones.len()]),
            }
        });
        commands.entity(entity).insert((
            SkinnedMesh {
                inverse_bindposes: rig.bindposes.clone(),
                joints: rig.joints.clone(),
            },
            GpuSkinned,
            // The bind-pose box is stale once the bones move (see `placements.rs`): always draw a skinned chunk.
            bevy_camera::visibility::NoFrustumCulling,
            Visibility::Inherited,
        ));
    }
}

/// Moves every joint entity to its bone's world pose this frame (the identity for a bone that does not move).
pub(crate) fn pose_joints(poses: Res<PartPoses>, mut joints: Query<(&SkinJoint, &mut Transform)>) {
    for (joint, mut transform) in &mut joints {
        let matrix = poses
            .matrix_of(&joint.0)
            .map_or(Mat4::IDENTITY, |m| Mat4::from_cols_array(&m));
        let pose = Transform::from_matrix(matrix);
        if *transform != pose {
            *transform = pose;
        }
    }
}
