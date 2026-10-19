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

/// The reaction state of a part with a behaviour. A response is a gesture, not a cycle, so its tracks hold
/// both ends whatever `loop` says.
#[expect(
    clippy::cast_possible_truncation,
    reason = "bounds are metres, well inside f32"
)]
fn reactive(
    item: &Uploaded,
    tracks: &mut [ve_modeling::anim::Track],
    behavior: &ve_modeling::anim::Behavior,
) -> crate::behavior::Reactive {
    for track in tracks.iter_mut() {
        track.looped = false;
    }
    // How long each clip runs: its longest track ("" is the main clip).
    let mut durations: std::collections::BTreeMap<String, f32> = std::collections::BTreeMap::new();
    for track in tracks.iter() {
        let longest = durations.entry(track.clip.clone()).or_insert(0.0);
        *longest = longest.max(track.duration());
    }
    let durations: Vec<(String, f32)> = durations.into_iter().collect();
    let lo = bevy_math::Vec3::from(item.bounds.min.map(|v| v as f32));
    let hi = bevy_math::Vec3::from(item.bounds.max.map(|v| v as f32));
    crate::behavior::Reactive::new(
        behavior.clone(),
        &durations,
        (lo + hi) * 0.5,
        item.node.clone(),
        item.name.clone(),
    )
}

/// Everything that makes a part move, react or deform, by authored node.
#[derive(Default)]
pub(super) struct Motion<'a> {
    pub(super) tracks: Option<&'a Vec<ve_modeling::anim::Track>>,
    pub(super) behavior: Option<&'a ve_modeling::anim::Behavior>,
    pub(super) parent: Option<&'a String>,
    pub(super) skin: Option<&'a std::sync::Arc<crate::skinning::SkinData>>,
    /// The chain of bones this part is the target of.
    pub(super) ik: Option<&'a std::sync::Arc<ve_modeling::rig::IkChain>>,
    /// Whether this part is a bone in some chain (it must be posed even with no tracks or parent).
    pub(super) ik_bone: bool,
    /// Its physics body, if it is one.
    pub(super) body: Option<&'a std::sync::Arc<crate::physics::BodySpec>>,
}

/// Gives an ordinary solid the components that move it (tracks, a behaviour, a parent), if it has any.
fn attach_motion(
    entity: &mut bevy_ecs::system::EntityCommands,
    item: &Uploaded,
    surface: &VeMaterial,
    motion: &Motion,
) {
    let (animation, behavior, parent) = (motion.tracks, motion.behavior, motion.parent);
    if let Some(chain) = motion.ik {
        // The solver aims the chain at the middle of this part, wherever it has been moved to.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "bounds are metres, well inside f32"
        )]
        let centre = (bevy_math::Vec3::from(item.bounds.min.map(|v| v as f32))
            + bevy_math::Vec3::from(item.bounds.max.map(|v| v as f32)))
            * 0.5;
        entity.insert(crate::animation::IkTarget {
            chain: std::sync::Arc::clone(chain),
            centre,
        });
    }
    if animation.is_none() && behavior.is_none() && parent.is_none() && !motion.ik_bone {
        return;
    }
    let mut tracks = animation.cloned().unwrap_or_default();
    if let Some(behavior) = behavior {
        entity.insert(reactive(item, &mut tracks, behavior));
    }
    if tracks
        .iter()
        .any(|t| t.property == ve_modeling::anim::TrackProperty::Glow)
    {
        entity.insert(crate::animation::BaseEmissive(surface.emissive));
    }
    if !tracks.is_empty() || parent.is_some() || motion.ik_bone {
        entity.insert(crate::animation::Animated {
            tracks,
            name: item.name.clone(),
            node: item.node.clone(),
            parent: parent.cloned(),
        });
    }
}

impl Motion<'_> {
    /// Whether the part moves (it has tracks or follows a parent).
    pub(super) fn moves(&self) -> bool {
        self.tracks.is_some() || self.parent.is_some() || self.ik_bone
    }
}

pub(super) fn spawn(
    commands: &mut Commands,
    item: Uploaded,
    token: &JobToken,
    surface: VeMaterial,
    placements: Option<&Placements>,
    motion: &Motion,
) {
    // Copies of a placed set share one list of motion tracks (glow, colour and visibility apply to single parts only).
    let shared: Option<std::sync::Arc<[ve_modeling::anim::Track]>> = motion
        .tracks
        .map(|tracks| {
            tracks
                .iter()
                .filter(|track| {
                    matches!(
                        track.property,
                        ve_modeling::anim::TrackProperty::Position
                            | ve_modeling::anim::TrackProperty::Rotation
                            | ve_modeling::anim::TrackProperty::Scale
                    )
                })
                .cloned()
                .collect::<Vec<_>>()
                .into()
        })
        .filter(|tracks: &std::sync::Arc<[ve_modeling::anim::Track]>| !tracks.is_empty());
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
            entity.insert((InstanceIndex(index), VeInstanceTransform(matrix, index)));
            if let Some(tracks) = &shared {
                entity.insert(crate::animation::InstanceAnimated {
                    tracks: std::sync::Arc::clone(tracks),
                    base: matrix,
                    index,
                });
            }
        } else if let Some(skin) = motion.skin.filter(|_| !item.rest.is_empty()) {
            entity.insert(crate::skinning::Skinned::new(
                std::sync::Arc::clone(skin),
                item.rest.clone(),
                item.corner_offset,
            ));
        } else {
            attach_motion(&mut entity, &item, &surface, motion);
            if let Some(spec) = motion.body {
                entity.insert(crate::physics::Body(std::sync::Arc::clone(spec)));
            }
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
