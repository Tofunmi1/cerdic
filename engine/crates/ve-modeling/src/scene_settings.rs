//! Small authored settings that ride on the scene environment: the wind and the light probes.
use serde::{Deserialize, Serialize};

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
