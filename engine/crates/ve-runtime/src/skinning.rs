//! Skinned parts (`rig.skin`, ADR 0030): each frame their meshes are deformed on the CPU from the bone parts' poses.
//!
//! A skinned part is uploaded as triangle-corner chunks like any other, with its rest positions and normals kept.
//! Per corner, the vertex it came from picks the bones and weights; the deformed position and normal are the
//! weighted sum of what each bone's pose does to the rest vertex (`ve_modeling::rig::deform`). Bones are ordinary
//! animated parts, so their world poses are the ones `animation::animate` already computed this frame.
use std::sync::Arc;

use bevy_asset::Assets;
use bevy_ecs::{
    component::Component,
    system::{Query, Res, ResMut},
};
use bevy_mesh::{Mesh, Mesh3d};
use ve_modeling::rig::{Blend, deform, deform_prepared, prepare_dual};

use crate::animation::PartPoses;

/// Most corner indices, joint indices or weights one skin may carry (bounded by the scene's triangle budget).
pub(crate) const MAX_ENTRIES: usize = 4_000_000;

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// What a skinned part needs from the evaluation: its bones, and per vertex four joints and four weights.
#[derive(Debug)]
pub(crate) struct SkinData {
    /// Output names of the bone parts, in joint-index order.
    pub(crate) bones: Vec<String>,
    /// How the bones' motions combine on a vertex.
    pub(crate) blend: Blend,
    /// For each triangle corner of the part, the vertex it comes from.
    pub(crate) indices: Vec<u32>,
    /// Four joint indices per vertex.
    pub(crate) joints: Vec<u32>,
    /// Four weights per vertex.
    pub(crate) weights: Vec<f32>,
}

impl SkinData {
    /// The same skin for the copy of a model that game entity `id` gets: the vertices and weights as they are, the bones renamed with
    /// the suffix `game_models` gives that copy's parts (`<name>#<id>`).
    pub(crate) fn for_instance(&self, id: u32) -> Self {
        Self {
            bones: self
                .bones
                .iter()
                .map(|bone| format!("{bone}#{id}"))
                .collect(),
            blend: self.blend,
            indices: self.indices.clone(),
            joints: self.joints.clone(),
            weights: self.weights.clone(),
        }
    }

    /// Whether the arrays agree with each other: corners point at vertices that have weights, joints name bones.
    pub(crate) fn valid(&self) -> bool {
        let vertices = self.joints.len() / 4;
        !self.bones.is_empty()
            && self.joints.len() == self.weights.len()
            && self.joints.len().is_multiple_of(4)
            && self.joints.len() <= MAX_ENTRIES
            && self.indices.len() <= MAX_ENTRIES
            && self
                .indices
                .iter()
                .all(|index| (*index as usize) < vertices)
            && self
                .joints
                .iter()
                .all(|joint| (*joint as usize) < self.bones.len())
            && self
                .weights
                .iter()
                .all(|weight| weight.is_finite() && *weight >= 0.0)
    }
}

/// One uploaded chunk of a skinned part.
#[derive(Component, Clone)]
pub(crate) struct Skinned {
    pub(crate) data: Arc<SkinData>,
    /// Rest position and normal (six floats) of each corner in this chunk.
    pub(crate) rest: Arc<[f32]>,
    /// Which corner of the whole part this chunk starts at.
    pub(crate) corner_offset: usize,
    /// The bone matrices the mesh was last deformed with, so an unchanged pose costs nothing.
    last: Vec<[f32; 16]>,
    /// Per corner, the rest normal averaged over every corner of the chunk that shares its vertex (three floats each), for the toon
    /// outline: a hull pushed along the flat face normals tears open at every edge, along the welded smooth normals it stays closed.
    smooth: Option<Arc<[f32]>>,
}

impl Skinned {
    pub(crate) fn new(data: Arc<SkinData>, rest: Vec<f32>, corner_offset: usize) -> Self {
        Self {
            data,
            rest: rest.into(),
            corner_offset,
            last: Vec::new(),
            smooth: None,
        }
    }

    /// The vertex-averaged rest normals, worked out the first time they are needed.
    pub(crate) fn smooth_rest(&mut self) -> Arc<[f32]> {
        if let Some(smooth) = &self.smooth {
            return Arc::clone(smooth);
        }
        let corners = self.rest.len() / 6;
        let vertex_of = |corner: usize| self.data.indices.get(self.corner_offset + corner).copied();
        let mut sums: std::collections::HashMap<u32, [f32; 3]> = std::collections::HashMap::new();
        for corner in 0..corners {
            if let Some(vertex) = vertex_of(corner) {
                let sum = sums.entry(vertex).or_insert([0.0; 3]);
                for axis in 0..3 {
                    sum[axis] += self.rest[corner * 6 + 3 + axis];
                }
            }
        }
        let mut smooth = Vec::with_capacity(corners * 3);
        for corner in 0..corners {
            let sum = vertex_of(corner)
                .and_then(|vertex| sums.get(&vertex))
                .copied()
                .unwrap_or([0.0, 1.0, 0.0]);
            let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            let scale = if length > 1.0e-6 { 1.0 / length } else { 0.0 };
            smooth.extend([sum[0] * scale, sum[1] * scale, sum[2] * scale]);
        }
        let smooth: Arc<[f32]> = smooth.into();
        self.smooth = Some(Arc::clone(&smooth));
        smooth
    }
}

/// A skinned chunk the GPU deforms (Bevy's skinning, see `skinning_gpu`): the CPU leaves its mesh alone.
#[derive(Component)]
pub(crate) struct GpuSkinned;

/// Deforms every skinned chunk whose bones moved since the last frame.
pub(crate) fn skin_meshes(
    poses: Res<PartPoses>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Query<(&mut Skinned, &Mesh3d), bevy_ecs::query::Without<GpuSkinned>>,
) {
    for (mut skinned, handle) in &mut parts {
        let matrices: Vec<[f32; 16]> = skinned
            .data
            .bones
            .iter()
            .map(|name| poses.matrix_of(name).unwrap_or(IDENTITY))
            .collect();
        if matrices == skinned.last {
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&handle.0) else {
            continue;
        };
        let dual = (skinned.data.blend == Blend::DualQuaternion).then(|| prepare_dual(&matrices));
        let corners = skinned.rest.len() / 6;
        let smooth = skinned.smooth_rest();
        let (mut positions, mut normals, mut hull) = (
            Vec::with_capacity(corners),
            Vec::with_capacity(corners),
            Vec::with_capacity(corners),
        );
        for corner in 0..corners {
            let Some(&vertex) = skinned.data.indices.get(skinned.corner_offset + corner) else {
                break;
            };
            let at = vertex as usize * 4;
            let (Some(joints), Some(weights)) = (
                skinned.data.joints.get(at..at + 4),
                skinned.data.weights.get(at..at + 4),
            ) else {
                break;
            };
            let rest = &skinned.rest[corner * 6..corner * 6 + 6];
            #[expect(
                clippy::cast_possible_truncation,
                reason = "joint indices are below 32"
            )]
            let joints = [
                joints[0] as u16,
                joints[1] as u16,
                joints[2] as u16,
                joints[3] as u16,
            ];
            let weights = [weights[0], weights[1], weights[2], weights[3]];
            let (at, normal) = ([rest[0], rest[1], rest[2]], [rest[3], rest[4], rest[5]]);
            let (p, n) = match &dual {
                Some(prepared) => deform_prepared(at, normal, joints, weights, prepared),
                None => deform(at, normal, joints, weights, &matrices),
            };
            let smooth_normal = [
                smooth[corner * 3],
                smooth[corner * 3 + 1],
                smooth[corner * 3 + 2],
            ];
            let (_, smooth_n) = match &dual {
                Some(prepared) => deform_prepared(at, smooth_normal, joints, weights, prepared),
                None => deform(at, smooth_normal, joints, weights, &matrices),
            };
            positions.push(p);
            normals.push(n);
            hull.push(smooth_n);
        }
        if positions.len() == corners {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            mesh.insert_attribute(ve_render::ATTRIBUTE_HULL_NORMAL, hull);
        }
        skinned.last = matrices;
    }
}
