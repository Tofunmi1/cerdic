//! Small authored settings that ride on the scene environment: the wind and the light probes.
use serde::{Deserialize, Serialize};

/// Distance haze and sun shafts authored on the scene environment.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Haze {
    /// Distance haze density: 0 is clear air, 1 is thick fog.
    pub density: f32,
    /// sRGB colour of the haze.
    pub color: [f32; 3],
    /// Ground haze falloff height in metres; 0 is uniform.
    pub height: f32,
    /// Visibility of the sun's shafts through haze, 0 to 1.
    pub shafts: f32,
    /// Distance in metres the camera focuses on; 0 turns depth of field off.
    #[serde(default)]
    pub focus_distance: f32,
    /// Lens aperture in f-stops: the smaller, the blurrier what is out of focus.
    #[serde(default = "default_aperture")]
    pub aperture: f32,
    /// Motion blur strength, 0 (off) to 1 (a full half-turn shutter).
    #[serde(default)]
    pub motion_blur: f32,
    /// Short-range shadows traced in screen space, which fix the gap under a foot or a pebble that the sun's shadow map misses.
    #[serde(default)]
    pub contact_shadows: bool,
}

fn default_aperture() -> f32 {
    2.8
}

impl Default for Haze {
    fn default() -> Self {
        Self {
            density: 0.0,
            color: [0.72, 0.78, 0.86],
            height: 0.0,
            shafts: 0.0,
            focus_distance: 0.0,
            aperture: 2.8,
            motion_blur: 0.0,
            contact_shadows: false,
        }
    }
}

/// The scene's wind: where it blows and how hard.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wind {
    /// Direction it blows toward on the ground plane, in degrees around the vertical axis: 0 is +x, 90 is +z.
    pub direction: f32,
    /// 0 is still air, 1 a strong breeze, 2 a gale.
    pub strength: f32,
}

impl Default for Wind {
    /// A gentle breeze toward +x.
    fn default() -> Self {
        Self {
            direction: 0.0,
            strength: 0.5,
        }
    }
}

/// Settings of the traced light probes (`gi` on a `scene.environment` or `scene.sky`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightProbes {
    /// Distance between probes in metres; 0 sizes it from the scene.
    pub spacing: f32,
    /// How many times light bounces between surfaces (1 to 4).
    pub bounces: u32,
    /// Rays traced per probe, per bounce (32 to 512): more is smoother and slower.
    pub rays: u32,
}
