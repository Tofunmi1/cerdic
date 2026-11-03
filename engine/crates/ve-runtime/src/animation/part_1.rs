use std::collections::HashMap;

use bevy_camera::visibility::InheritedVisibility;
use bevy_color::{Color, LinearRgba};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::Without,
    resource::Resource,
    system::{Commands, Local, Query, Res, ResMut},
};
use bevy_math::{EulerRot, Mat4, Quat, Vec3};
use bevy_time::{Real, Time};
use bevy_transform::components::Transform;
use ve_modeling::anim::{Track, TrackProperty};
use ve_modeling::hierarchy::MAX_DEPTH;
use ve_render::{VeMaterial, VePointLight, VeSpotLight};

use crate::projection::Authored;

/// A part that moves or glows: its tracks, who it is and what it follows. Its transform is replaced by the
/// pose of its tracks (after its parent's pose) each frame.
#[derive(Component, Clone, Debug)]
pub(crate) struct Animated {
    pub(crate) tracks: Vec<Track>,
    /// The part's output name, which children name as their parent.
    pub(crate) name: String,
    /// The authored node that makes the part, which its lights share.
    pub(crate) node: String,
    pub(crate) parent: Option<String>,
    /// Set on the parts of a game entity's own copy of a model (ADR 0066): which entity, so the game's placement of it leads the chain.
    pub(crate) instance: Option<u32>,
}

/// Where the game has put each instance of a model this frame (by game entity id): the matrix in front of its chain of parents.
#[derive(Resource, Default)]
pub(crate) struct InstancePlacement(pub(crate) HashMap<u32, Mat4>);

/// The emissive colour a glowing part was authored with; its glow track scales it.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct BaseEmissive(pub(crate) Color);

/// A light that rides with the part that emits it: where it started and how bright it was authored.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct PartLight {
    pub(crate) position: Vec3,
    pub(crate) direction: Vec3,
    pub(crate) intensity: f32,
}

/// This frame's world pose and glow factor of each animated part, by authored node.
#[derive(Resource, Default)]
pub(crate) struct PartPoses(
    HashMap<String, (Mat4, f32, bool, Option<[f32; 3]>)>,
    /// World pose by part output name, for skins (their bones are named).
    HashMap<String, [f32; 16]>,
);

impl PartPoses {
    /// The world pose of the part with this output name this frame; `None` for a part that does not move.
    pub(crate) fn matrix_of(&self, name: &str) -> Option<[f32; 16]> {
        self.1.get(name).copied()
    }
}

/// Seconds into a clip, by the clip's name: the shared clock for a part with no behaviour (main clip only), or each clip's
/// own time. NaN means the clip is not playing, and its tracks are skipped.
pub(crate) type ClipTime<'a> = &'a dyn Fn(&str) -> f32;

/// The transform that `tracks` give a part, each sampled at its own clip's time. Each track acts in the order
/// listed, in world space: position adds a translation, rotation and scale act about the track's pivot.
pub(crate) fn pose(tracks: &[Track], time_of: ClipTime) -> Transform {
    pose_by(tracks, &|track| time_of(&track.clip))
}

/// Like [`pose`], with each track's sample time chosen by the caller (a placed copy plays its tracks ahead by their phase).
pub(crate) fn pose_by(tracks: &[Track], seconds_of: &dyn Fn(&Track) -> f32) -> Transform {
    let mut matrix = Mat4::IDENTITY;
    for track in tracks {
        let seconds = seconds_of(track);
        if seconds.is_nan() {
            continue;
        }
        let value = Vec3::from(track.sample(seconds));
        let pivot = Vec3::from(track.pivot);
        let about_pivot =
            |change: Mat4| Mat4::from_translation(pivot) * change * Mat4::from_translation(-pivot);
        let step = match track.property {
            TrackProperty::Position => Mat4::from_translation(value),
            TrackProperty::Rotation => about_pivot(Mat4::from_quat(Quat::from_euler(
                EulerRot::YXZ,
                value.y.to_radians(),
                value.x.to_radians(),
                value.z.to_radians(),
            ))),
            TrackProperty::Scale => about_pivot(Mat4::from_scale(value)),
            TrackProperty::Glow | TrackProperty::Visible | TrackProperty::Color => Mat4::IDENTITY,
        };
        matrix = step * matrix;
    }
    Transform::from_matrix(matrix)
}

/// The brightness factor the glow tracks give a part (1 when it has none).
pub(crate) fn glow(tracks: &[Track], time_of: ClipTime) -> f32 {
    tracks
        .iter()
        .filter(|track| track.property == TrackProperty::Glow)
        .map(|track| (track, time_of(&track.clip)))
        .filter(|(_, seconds)| !seconds.is_nan())
        .map(|(track, seconds)| track.sample(seconds)[0])
        .product()
}

/// The surface colour the colour tracks give a part (the last track wins; none: `None`).
pub(crate) fn tint(tracks: &[Track], time_of: ClipTime) -> Option<[f32; 3]> {
    tracks
        .iter()
        .rfind(|track| track.property == TrackProperty::Color && !time_of(&track.clip).is_nan())
        .map(|track| track.sample(time_of(&track.clip)))
}

/// Shows the colour of parts whose tracks include `color`.
pub(crate) fn apply_color(poses: Res<PartPoses>, mut parts: Query<(&Animated, &mut VeMaterial)>) {
    for (animated, mut material) in &mut parts {
        let Some((.., Some([r, g, b]))) = poses.0.get(&animated.node) else {
            continue;
        };
        let colour = Color::srgb(*r, *g, *b);
        if material.base_color != colour {
            material.base_color = colour;
        }
    }
}

/// Whether the visibility tracks show a part (shown when it has none).
pub(crate) fn shown(tracks: &[Track], time_of: ClipTime) -> bool {
    tracks
        .iter()
        .filter(|track| track.property == TrackProperty::Visible)
        .map(|track| (track, time_of(&track.clip)))
        .filter(|(_, seconds)| !seconds.is_nan())
        .all(|(track, seconds)| track.sample(seconds)[0] >= 0.5)
}

/// The world pose of every part: its own pose after those of the parents it hangs from, up the chain.
fn resolve_worlds(
    locals: &[(Entity, &Animated, Mat4, f32, bool, Option<[f32; 3]>)],
    by_name: &HashMap<&str, usize>,
    placements: &HashMap<u32, Mat4>,
    loose: &std::collections::HashSet<usize>,
) -> Vec<Mat4> {
    (0..locals.len())
        .map(|start| {
            let (mut world, mut at, mut depth) = (locals[start].2, start, 0);
            // A part in `loose` (a limp ragdoll's bone) already has its world pose in its local matrix: the chain stops there.
            while !loose.contains(&at)
                && let Some(&parent) = locals[at]
                .1
                .parent
                .as_deref()
                .and_then(|name| by_name.get(name))
            {
                depth += 1;
                if depth > MAX_DEPTH || parent == start {
                    break;
                }
                at = parent;
                world = locals[at].2 * world;
            }
            // A part of a game entity's own copy of a model is placed by the game: its matrix leads the whole chain.
            match locals[start].1.instance.and_then(|id| placements.get(&id)) {
                Some(placed) => *placed * world,
                None => world,
            }
        })
        .collect()
}

/// A part that a chain of bones reaches for (`rig.ik`): the chain, and the middle of the part when it was authored.
#[derive(Component, Clone)]
pub(crate) struct IkTarget {
    pub(crate) chain: std::sync::Arc<ve_modeling::rig::IkChain>,
    pub(crate) centre: Vec3,
}
