use std::collections::HashMap;

use bevy_camera::visibility::InheritedVisibility;
use bevy_color::{Color, LinearRgba};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::Without,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
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
}

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

/// Seconds into the clip shared by every animated part, and whether it is running.
#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct AnimationClock {
    pub(crate) seconds: f32,
    pub(crate) playing: bool,
    /// Multiple of real time the clock runs at.
    pub(crate) speed: f32,
}

impl Default for AnimationClock {
    fn default() -> Self {
        Self {
            seconds: 0.0,
            playing: true,
            speed: 1.0,
        }
    }
}

/// Slowest and fastest the shared clock may run, as a multiple of real time.
const SPEED_RANGE: (f32, f32) = (0.05, 8.0);

thread_local! {
    /// The page's latest request (play or pause, and a time to jump to), taken by [`apply_requested`].
    static REQUEST: std::cell::Cell<Option<(bool, f32)>> = const { std::cell::Cell::new(None) };
    /// The page's latest speed request.
    static SPEED: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Asks the clock to run at `speed` times real time (clamped to a sensible range).
pub(crate) fn set_speed(speed: f32) {
    SPEED.with(|request| request.set(Some(speed)));
}

/// Asks the clock to play or pause; a `seconds` of zero or more also jumps it there.
pub(crate) fn control(playing: bool, seconds: f32) {
    REQUEST.with(|request| request.set(Some((playing, seconds))));
}

/// Applies the page's request to the clock.
pub(crate) fn apply_requested(mut clock: ResMut<AnimationClock>) {
    if let Some((playing, seconds)) = REQUEST.with(std::cell::Cell::take) {
        clock.playing = playing;
        if seconds.is_finite() && seconds >= 0.0 {
            clock.seconds = seconds;
        }
    }
    if let Some(speed) = SPEED.with(std::cell::Cell::take)
        && speed.is_finite()
    {
        clock.speed = speed.clamp(SPEED_RANGE.0, SPEED_RANGE.1);
    }
}

/// Moves the clock on by the real time since the last frame (times its speed) while it is playing.
pub(crate) fn advance(
    time: Res<Time<Real>>,
    mut clock: ResMut<AnimationClock>,
    mut materials: Query<&mut VeMaterial>,
) {
    if clock.playing {
        clock.seconds += time.delta_secs() * clock.speed;
    }
    for mut material in &mut materials {
        if material.recipe == ve_render::ProceduralRecipe::Water {
            material.water_time = clock.seconds;
        }
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
) -> Vec<Mat4> {
    (0..locals.len())
        .map(|start| {
            let (mut world, mut at, mut depth) = (locals[start].2, start, 0);
            while let Some(&parent) = locals[at]
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
            world
        })
        .collect()
}

/// A part that a chain of bones reaches for (`rig.ik`): the chain, and the middle of the part when it was authored.
#[derive(Component, Clone)]
pub(crate) struct IkTarget {
    pub(crate) chain: std::sync::Arc<ve_modeling::rig::IkChain>,
    pub(crate) centre: Vec3,
}
