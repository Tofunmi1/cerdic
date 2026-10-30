//! The water surface description carried by a material (ADR 0059).

use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

/// The look of a water surface: colour by depth, a flowing ripple pattern and foam. Colours are sRGB; the other values are 0 to 1 unless noted.
///
/// The parameters are serialized through reflection so scripts can tune water materials.
#[derive(
    Clone, Copy, Debug, PartialEq, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize,
)]
#[reflect(Serialize, Deserialize)]
pub struct SurfaceWater {
    /// Colour of clear shallow water.
    pub shallow: [f32; 3],
    /// Colour of deep water.
    pub deep: [f32; 3],
    /// Metres of depth over which shallow turns into deep.
    pub clarity: f32,
    /// Speed of the streaming surface pattern, 1 is natural, up to 4.
    pub flow: f32,
    /// Strength of the small waves, 0 to 2; zero gives a flat wave field.
    pub ripple: f32,
    /// Main wave travel direction in degrees; zero travels along positive X.
    #[serde(default)]
    #[reflect(default)]
    pub direction: f32,
    /// Seed of the repeatable wave table.
    #[serde(default)]
    #[reflect(default)]
    pub seed: u32,
    /// Amount of foam.
    pub foam: f32,
    /// Enables the interactive wave simulation.
    #[serde(default)]
    #[reflect(default)]
    pub sim: bool,
    /// Cells along the longer side (32 to 256).
    #[serde(default = "default_resolution")]
    #[reflect(default = "default_resolution")]
    pub sim_resolution: u32,
    /// Velocity damping per second.
    #[serde(default = "default_damping")]
    #[reflect(default = "default_damping")]
    pub sim_damping: f32,
    /// Fallback depth in metres.
    #[serde(default = "default_depth")]
    #[reflect(default = "default_depth")]
    pub sim_depth: f32,
    /// Seeded drops per second per 100 square metres of water.
    #[serde(default)]
    #[reflect(default)]
    pub rain: f32,
    /// Runtime hint: this water came from a filled terrain basin and can use the bounded 3D reference solver.
    #[serde(default)]
    #[reflect(default)]
    pub volumetric: bool,
    /// Runtime simulation identity; zero binds still water.
    #[serde(skip)]
    #[reflect(default)]
    pub sim_id: u64,
    /// Runtime world bounds: origin X,Z and size X,Z.
    #[serde(skip)]
    #[reflect(default)]
    pub sim_bounds: [f32; 4],
}

impl Default for SurfaceWater {
    fn default() -> Self {
        Self {
            shallow: [0.16, 0.42, 0.45],
            deep: [0.02, 0.12, 0.2],
            clarity: 3.0,
            flow: 1.0,
            ripple: 1.0,
            foam: 0.5,
            direction: 0.0,
            seed: 0,
            sim: false,
            sim_resolution: 128,
            sim_damping: 0.6,
            sim_depth: 1.0,
            rain: 0.0,
            volumetric: false,
            sim_id: 0,
            sim_bounds: [0.0; 4],
        }
    }
}

fn default_resolution() -> u32 {
    128
}
fn default_damping() -> f32 {
    0.6
}
fn default_depth() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_water_json_and_reflection_use_simulation_defaults() {
        let json = r#"{"shallow":[0.1,0.2,0.3],"deep":[0.0,0.1,0.2],"clarity":3.0,"flow":1.0,"ripple":1.0,"foam":0.5}"#;
        let water: SurfaceWater = serde_json::from_str(json).unwrap();
        assert!(!water.sim);
        assert_eq!(water.sim_resolution, 128);
        assert!((water.sim_damping - 0.6).abs() < 1e-6);
        assert!((water.sim_depth - 1.0).abs() < 1e-6);
        assert!(water.rain.abs() < 1e-6);
        use bevy_reflect::{FromReflect, PartialReflect, structs::DynamicStruct};
        let mut old = DynamicStruct::default();
        old.insert("shallow", water.shallow);
        old.insert("deep", water.deep);
        old.insert("clarity", water.clarity);
        old.insert("flow", water.flow);
        old.insert("ripple", water.ripple);
        old.insert("foam", water.foam);
        let restored = SurfaceWater::from_reflect(old.as_partial_reflect()).unwrap();
        assert_eq!(restored, water);
    }
}
