//! Checked placement batches and shared-mesh entity spawning.
use super::{
    Authored, Commands, Component, Interactable, JobToken, Label, Mesh3d, STATE, Transform,
    Uploaded, VeMaterial, staging,
};
use bevy_math::Mat4;
use ve_render::VeInstanceTransform;

/// Original logical instance index, shared by all chunks of the prototype.
#[derive(Component, Clone, Copy)]
pub(crate) struct InstanceIndex(pub(crate) u32);

#[derive(Default)]
pub(super) struct Placements {
    pub(super) node: String,
    pub(super) matrices: Vec<Mat4>,
    pub(super) indices: Vec<u32>,
}

pub(crate) fn append(
    text: &str,
    group: u32,
    node: &str,
    matrices: &[f32],
    indices: &[u32],
) -> bool {
    if group == 0
        || indices.is_empty()
        || indices.len() > 512
        || matrices.len() != indices.len() * 16
        || ve_graph::NodeId::parse(node).is_err()
        || indices.iter().any(|i| *i >= 16_384)
    {
        return false;
    }
    let mut parsed = Vec::new();
    for values in matrices.chunks_exact(16) {
        let matrix = Mat4::from_cols_slice(values);
        if !matrix.is_finite()
            || matrix.determinant() <= 1e-12
            || matrix.x_axis.w.abs() > 1e-6
            || matrix.y_axis.w.abs() > 1e-6
            || matrix.z_axis.w.abs() > 1e-6
            || (matrix.w_axis.w - 1.0).abs() > 1e-6
            || matrix.w_axis.truncate().abs().max_element() > 1e6
        {
            return false;
        }
        parsed.push(matrix);
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(text)
            || !staging::current(&candidate.token)
            || candidate.finished
            || candidate.placement_count + indices.len() > 16_384
        {
            return false;
        }
        let entry = candidate.placements.entry(group).or_default();
        if !entry.node.is_empty() && entry.node != node {
            return false;
        }
        if entry.indices.last().is_some_and(|last| *last >= indices[0])
            || indices.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return false;
        }
        node.clone_into(&mut entry.node);
        entry.matrices.extend(parsed);
        entry.indices.extend_from_slice(indices);
        candidate.placement_count += indices.len();
        true
    })
}

pub(super) fn spawn(
    commands: &mut Commands,
    item: Uploaded,
    token: &JobToken,
    surface: VeMaterial,
    placements: Option<&Placements>,
) {
    let spawn_one = |commands: &mut Commands, matrix: Mat4, index: Option<u32>| {
        let mut entity = commands.spawn((
            Mesh3d(item.handle.clone()),
            surface,
            Transform::from_translation(matrix.w_axis.truncate()),
            Interactable::selection_only(&item.bounds),
            Label(item.name.clone()),
            Authored {
                node: item.node.clone(),
                revision: token.revision,
            },
        ));
        if let Some(index) = index {
            entity.insert((InstanceIndex(index), VeInstanceTransform(matrix)));
        }
    };
    if let Some(placements) = placements {
        for (matrix, index) in placements.matrices.iter().zip(&placements.indices) {
            spawn_one(commands, *matrix, Some(*index));
        }
    } else {
        spawn_one(commands, Mat4::IDENTITY, None);
    }
}
