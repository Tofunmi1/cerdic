//! Authored light upload and replacement.
use super::{Authored, Commands, Component, JobToken, STATE, staging};
use bevy_color::Color;
use bevy_math::Vec3;
use bevy_transform::components::Transform as BevyTransform;
use ve_modeling::{MAX_SCENE_LIGHTS, SceneLight, SceneLightKind};
use ve_render::{VeDirectionalLight, VePointLight, VeSpotLight};

const MAX_LIGHT_JSON_BYTES: usize = 64 * 1024;

/// Marker used to replace evaluated lights atomically with the authored mesh revision.
#[derive(Component, Clone, Copy)]
pub(crate) struct AuthoredLight;

/// Stages the complete bounded light list for the current evaluation.
pub(crate) fn append(token: &str, json: &str) -> bool {
    if json.len() > MAX_LIGHT_JSON_BYTES {
        return false;
    }
    let Ok(parsed) = serde_json::from_str::<Vec<SceneLight>>(json) else {
        return false;
    };
    if parsed.len() > MAX_SCENE_LIGHTS || parsed.iter().any(|light| !valid(light)) {
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
        candidate.lights = parsed;
        true
    })
}

fn valid(light: &SceneLight) -> bool {
    let finite = light
        .position
        .iter()
        .chain(&light.direction)
        .chain(&light.color)
        .all(|value| value.is_finite());
    let direction_length = Vec3::from_array(light.direction).length();
    finite
        && light.position.iter().all(|value| value.abs() <= 1.0e6)
        && light.color.iter().all(|value| (0.0..=1.0).contains(value))
        && light.intensity.is_finite()
        && (0.0..=100_000.0).contains(&light.intensity)
        && light.range.is_finite()
        && (0.0..=1.0e6).contains(&light.range)
        && match light.kind {
            SceneLightKind::Point => true,
            SceneLightKind::Spot => {
                direction_length > 0.999
                    && direction_length < 1.001
                    && light.inner_angle.is_finite()
                    && light.outer_angle.is_finite()
                    && 0.0 < light.inner_angle
                    && light.inner_angle < light.outer_angle
                    && light.outer_angle < std::f32::consts::PI
            }
            SceneLightKind::Sun => direction_length > 0.999 && direction_length < 1.001,
        }
}

/// Creates main-world light entities when the staged geometry is accepted.
pub(super) fn spawn(commands: &mut Commands, lights: &[SceneLight], token: &JobToken) {
    for light in lights {
        let color = Color::srgb(light.color[0], light.color[1], light.color[2]);
        let marker = AuthoredLight;
        match light.kind {
            SceneLightKind::Point => {
                commands.spawn((
                    crate::animation::PartLight {
                        position: Vec3::from_array(light.position),
                        direction: Vec3::from_array(light.direction),
                        intensity: light.intensity,
                    },
                    VePointLight {
                        color,
                        intensity: light.intensity,
                        range: light.range,
                    },
                    BevyTransform::from_translation(Vec3::from_array(light.position)),
                    Authored {
                        node: light.node.to_string(),
                        name: String::new(),
                        revision: token.revision,
                    },
                    marker,
                ));
            }
            SceneLightKind::Spot => {
                commands.spawn((
                    crate::animation::PartLight {
                        position: Vec3::from_array(light.position),
                        direction: Vec3::from_array(light.direction),
                        intensity: light.intensity,
                    },
                    VeSpotLight {
                        color,
                        intensity: light.intensity,
                        range: light.range,
                        direction: Vec3::from_array(light.direction),
                        inner_angle: light.inner_angle,
                        outer_angle: light.outer_angle,
                    },
                    BevyTransform::from_translation(Vec3::from_array(light.position)),
                    Authored {
                        node: light.node.to_string(),
                        name: String::new(),
                        revision: token.revision,
                    },
                    marker,
                ));
            }
            SceneLightKind::Sun => {
                let direction = Vec3::from_array(light.direction);
                let up = if direction.dot(Vec3::Y).abs() > 0.999 {
                    Vec3::Z
                } else {
                    Vec3::Y
                };
                commands.spawn((
                    VeDirectionalLight {
                        color,
                        intensity: light.intensity,
                    },
                    BevyTransform::IDENTITY.looking_to(direction, up),
                    Authored {
                        node: light.node.to_string(),
                        name: String::new(),
                        revision: token.revision,
                    },
                    marker,
                ));
            }
        }
    }
}
