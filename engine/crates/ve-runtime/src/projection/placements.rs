//! Checked placement batches and shared-mesh entity spawning.
use super::{
    Authored, Commands, Component, Interactable, JobToken, Label, Mesh3d, STATE, Transform,
    Uploaded, VeMaterial, staging,
};
use bevy_math::Mat4;
use ve_render::VeInstanceTransform;

/// Most placements one scene may upload across all its sets (each set is still limited to 16,384 copies). A forest planted in three
/// camera-distance bands draws the same trees three times over, so this is well above one set's limit.
const MAX_SCENE_PLACEMENTS: usize = 131_072;

/// Original logical instance index, shared by all chunks of the prototype.
#[derive(Component, Clone, Copy)]
pub(crate) struct InstanceIndex(pub(crate) u32);

#[derive(Default)]
pub(super) struct Placements {
    pub(super) node: String,
    pub(super) matrices: Vec<Mat4>,
    pub(super) indices: Vec<u32>,
    pub(super) visible_range: Option<(f32, f32)>,
    pub(super) visible_fade: f32,
}

pub(crate) fn append(
    text: &str,
    group: u32,
    node: &str,
    matrices: &[f32],
    indices: &[u32],
    visible_range: Option<(f32, f32)>,
    visible_fade: f32,
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
            || candidate.placement_count + indices.len() > MAX_SCENE_PLACEMENTS
        {
            return false;
        }
        let entry = candidate.placements.entry(group).or_default();
        if !entry.node.is_empty() && entry.node != node {
            return false;
        }
        if !entry.indices.is_empty()
            && (entry.visible_range != visible_range || entry.visible_fade != visible_fade)
        {
            return false;
        }
        entry.visible_range = visible_range;
        entry.visible_fade = visible_fade;
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
    /// The animation state machine on this part (`anim.machine`).
    pub(super) machine: Option<&'a std::sync::Arc<ve_animstate::StateMachine>>,
    pub(super) parent: Option<&'a String>,
    pub(super) skin: Option<&'a std::sync::Arc<crate::skinning::SkinData>>,
    /// The chain of bones this part is the target of.
    pub(super) ik: Option<&'a std::sync::Arc<ve_modeling::rig::IkChain>>,
    /// The particle effect this part emits.
    pub(super) fx: Option<&'a std::sync::Arc<ve_modeling::fx::PartFx>>,
    /// The ragdoll this part triggers.
    pub(super) ragdoll: Option<&'a std::sync::Arc<ve_modeling::ragdoll::RagdollSpec>>,
    /// The live cloth this part is.
    pub(super) cloth: Option<&'a std::sync::Arc<crate::cloth_live::Shared>>,
    /// The live liquid this part is.
    pub(super) fluid: Option<&'a std::sync::Arc<crate::fluid_live::Shared>>,
    /// The simulation program this part is drawn by.
    pub(super) sim: Option<&'a std::sync::Arc<crate::sim_live::Shared>>,
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
    if let Some(graph) = motion.machine {
        entity.insert(crate::animation::MachineDrive::new(std::sync::Arc::clone(graph)));
    }
    if let Some(rig) = motion.ragdoll {
        entity.insert(crate::physics::RagdollRig(std::sync::Arc::clone(rig)));
    }
    if let Some(effect) = motion.fx {
        // Particles come from the middle of the part, wherever it has been moved to.
        #[expect(clippy::cast_possible_truncation, reason = "bounds are metres, well inside f32")]
        let centre = (bevy_math::Vec3::from(item.bounds.min.map(|v| v as f32))
            + bevy_math::Vec3::from(item.bounds.max.map(|v| v as f32)))
            * 0.5;
        entity.insert(crate::effects::EffectEmitter::new(std::sync::Arc::clone(effect), centre));
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
            instance: None,
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
    // Many copies that never move are one retained set, not an entity each.
    if let Some(set) = placements
        && super::bulk::eligible(set.matrices.len(), &surface, shared.is_some())
    {
        super::bulk::spawn(
            commands,
            item,
            token,
            surface,
            &set.matrices,
            &set.indices,
            set.visible_range,
            set.visible_fade,
        );
        return;
    }
    let spawn_one = |commands: &mut Commands, matrix: Mat4, index: Option<u32>| {
        let mut entity = commands.spawn((
            Mesh3d(item.handle.clone()),
            surface,
            Transform::from_translation(matrix.w_axis.truncate()),
            Interactable::selection_only(&item.bounds),
            Label(item.name.clone()),
            Authored {
                node: item.node.clone(),
                name: item.name.clone(),
                revision: token.revision,
            },
        ));
        if let Some((from, to)) = placements.and_then(|set| set.visible_range) {
            let fade = placements.map_or(0.0, |set| set.visible_fade);
            entity.insert(visibility_range(from, to, fade));
        }
        if let Some(index) = index {
            entity.insert((InstanceIndex(index), VeInstanceTransform(matrix, index)));
            if let Some(tracks) = &shared {
                entity.insert(crate::animation::InstanceAnimated {
                    tracks: std::sync::Arc::clone(tracks),
                    base: matrix,
                    index,
                });
            }
        } else if let Some(shared) = motion.fluid.filter(|_| !item.rest.is_empty()) {
            // The surface is rebuilt from the particles every frame, wherever it was when uploaded.
            entity.insert(bevy_camera::visibility::NoFrustumCulling);
            entity.insert(crate::fluid_live::FluidLive { shared: std::sync::Arc::clone(shared), corner_offset: item.corner_offset });
        } else if let Some(shared) = motion.sim.filter(|_| !item.rest.is_empty()) {
            // The tetrahedra move every frame, away from the box Bevy computed at upload.
            entity.insert(bevy_camera::visibility::NoFrustumCulling);
            entity.insert(crate::sim_live::SimLive { shared: std::sync::Arc::clone(shared), corner_offset: item.corner_offset });
        } else if let Some(shared) = motion.cloth.filter(|_| !item.rest.is_empty()) {
            // Its points move every frame, away from the box Bevy computed at upload.
            entity.insert(bevy_camera::visibility::NoFrustumCulling);
            entity.insert(crate::cloth_live::ClothLive {
                shared: std::sync::Arc::clone(shared),
                corner_offset: item.corner_offset,
            });
        } else if let Some(skin) = motion.skin.filter(|_| !item.rest.is_empty()) {
            // A GPU skinned chunk stays hidden until `attach_gpu_skins` gives it its skin, so Bevy never draws it unskinned.
            #[cfg(any(feature = "bevy-webgl", feature = "bevy-webgpu"))]
            if skin.blend == ve_modeling::rig::Blend::Linear {
                entity.insert(bevy_camera::visibility::Visibility::Hidden);
            }
            // A deformed mesh leaves the bounding box Bevy computed when it first appeared (culling against that stale box hid the body while
            // its outline hull stayed: a black ghost), so a skinned part is always drawn; static parts keep frustum culling.
            entity.insert(bevy_camera::visibility::NoFrustumCulling);
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

fn visibility_range(from: f32, to: f32, fade: f32) -> bevy_camera::visibility::VisibilityRange {
    use bevy_camera::visibility::VisibilityRange;
    if fade <= 0.0 {
        return VisibilityRange::abrupt(from, to);
    }
    VisibilityRange {
        start_margin: if from == 0.0 {
            0.0..0.0
        } else {
            (from - fade * 0.5)..(from + fade * 0.5)
        },
        end_margin: if to.is_infinite() {
            0.0..0.0
        } else {
            (to - fade * 0.5)..(to + fade * 0.5)
        },
        use_aabb: false,
    }
}
