use bevy_ecs::prelude::*;
use bevy_time::{Real, Time};
use ve_render::VeMaterial;

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
    mut wind: ResMut<ve_render::WindField>,
    mut materials: Query<&mut VeMaterial>,
) {
    if clock.playing {
        clock.seconds += time.delta_secs() * clock.speed;
    }
    // Gusts and flutter run on the shared clock, so pausing it holds the foliage still.
    wind.time = clock.seconds;
    for mut material in &mut materials {
        // Open water.fill/river sheets carry finish.water with recipe None.
        if material.finish.water.is_some() || material.recipe == ve_render::ProceduralRecipe::Water
        {
            material.water_time = clock.seconds;
        }
    }
}
