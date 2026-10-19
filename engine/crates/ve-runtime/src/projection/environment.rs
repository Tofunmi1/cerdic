//! Authored scene-environment upload and conversion to the renderer ambient.
use super::{STATE, staging};
use bevy_color::Color;
use ve_modeling::SceneEnvironment;
use ve_render::VeAmbientLight;

const MAX_ENVIRONMENT_JSON_BYTES: usize = 4096;

/// Stages the authored scene environment for the current evaluation (at most one per document).
pub(crate) fn append(token: &str, json: &str) -> bool {
    if json.len() > MAX_ENVIRONMENT_JSON_BYTES {
        return false;
    }
    let Ok(parsed) = serde_json::from_str::<SceneEnvironment>(json) else {
        return false;
    };
    if !valid(&parsed) {
        return false;
    }
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(candidate) = state.candidate.as_mut() else {
            return false;
        };
        if serde_json::to_string(&candidate.token).ok().as_deref() != Some(token)
            || !staging::current(&candidate.token)
            || candidate.finished
        {
            return false;
        }
        candidate.environment = Some(parsed);
        true
    })
}

fn valid(environment: &SceneEnvironment) -> bool {
    let colours = environment
        .sky_color
        .iter()
        .chain(&environment.ground_color)
        .chain(&environment.bounce_color);
    colours
        .clone()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        && environment.intensity.is_finite()
        && (0.0..=8.0).contains(&environment.intensity)
        && environment.reflection.is_finite()
        && (0.0..=1.0).contains(&environment.reflection)
        && environment.bounce.is_finite()
        && (0.0..=1.0).contains(&environment.bounce)
        && environment.sky.as_ref().is_none_or(|sky| {
            let direction = bevy_math::Vec3::from_array(sky.sun_direction);
            sky.sun_direction.iter().all(|value| value.is_finite())
                && (0.99..=1.01).contains(&direction.length())
                && sky
                    .horizon_color
                    .iter()
                    .chain(&sky.zenith_color)
                    .all(|v| v.is_finite() && (0.0..=2.0).contains(v))
                && sky
                    .ground_color
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                && sky.sun_intensity.is_finite()
                && (0.0..=100.0).contains(&sky.sun_intensity)
                && sky.turbidity.is_finite()
                && (1.0..=10.0).contains(&sky.turbidity)
                && sky.cloud_cover.is_finite()
                && (0.0..=1.0).contains(&sky.cloud_cover)
                && sky.seed <= 1_000_000
        })
}

/// The renderer ambient an authored environment describes.
pub(crate) fn ambient(environment: &SceneEnvironment) -> VeAmbientLight {
    let colour = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]);
    VeAmbientLight {
        sky_color: colour(environment.sky_color),
        ground_color: colour(environment.ground_color),
        intensity: environment.intensity,
        reflection: environment.reflection,
        bounce_color: colour(environment.bounce_color),
        bounce: environment.bounce,
        ..VeAmbientLight::default()
    }
}
