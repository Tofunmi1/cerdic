//! Wind: one global field that bends and flutters foliage in the vertex shaders.
//!
//! The field is a direction on the ground plane, a strength and the animation clock; it travels in the view uniform
//! (`ViewUniform::wind`, read by the forward, sun-shadow and depth-for-AO vertex stages). A surface opts in through its
//! [`FoliageStyle`](crate::FoliageStyle) `sway` and `flutter`; everything else is untouched, so a scene with no foliage
//! pays nothing and looks as before.

use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_render::RenderApp;
use bevy_render::extract_resource::ExtractResource;

/// The wind blowing through the scene.
#[derive(Resource, ExtractResource, Clone, Copy, Debug, PartialEq)]
#[extract_app(RenderApp)]
pub struct WindField {
    /// Unit direction the wind blows toward, on the ground plane (x, z).
    pub direction: [f32; 2],
    /// How hard it blows: 0 is still air, 1 a strong breeze, up to 2.
    pub strength: f32,
    /// Seconds on the shared animation clock; gusts and flutter are functions of it.
    pub time: f32,
}

impl Default for WindField {
    /// A gentle breeze toward +x.
    fn default() -> Self {
        Self::from_degrees(0.0, 0.5)
    }
}

impl WindField {
    /// A wind blowing toward the compass-style angle `degrees` around the vertical axis (0 is +x, 90 is +z), at `strength`
    /// (clamped to 0 to 2).
    #[must_use]
    pub fn from_degrees(degrees: f32, strength: f32) -> Self {
        let radians = if degrees.is_finite() {
            degrees.to_radians()
        } else {
            0.0
        };
        Self {
            direction: [radians.cos(), radians.sin()],
            strength: if strength.is_finite() {
                strength.clamp(0.0, 2.0)
            } else {
                0.0
            },
            time: 0.0,
        }
    }

    /// The view-uniform lane: direction x and z, strength, time. Non-finite numbers become still air.
    #[must_use]
    pub(crate) fn uniform(&self) -> Vec4 {
        let [x, z] = self.direction;
        if [x, z, self.strength, self.time]
            .iter()
            .all(|v| v.is_finite())
        {
            Vec4::new(x, z, self.strength.clamp(0.0, 2.0), self.time)
        } else {
            Vec4::new(1.0, 0.0, 0.0, 0.0)
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::float_cmp,
    reason = "the clamped and defaulted values are exact constants"
)]
mod tests {
    use super::WindField;

    #[test]
    fn the_wind_function_is_defined_once_and_built_into_all_three_shaders() {
        let definition = "fn wind_displacement";
        assert!(include_str!("shaders/wind.wgsl").contains(definition));
        for (name, own) in [
            ("forward", include_str!("shaders/forward.wgsl")),
            ("shadow", include_str!("shaders/shadow.wgsl")),
            ("ao_depth", include_str!("shaders/ao_depth.wgsl")),
        ] {
            assert!(
                !own.contains(definition),
                "{name}.wgsl defines its own copy"
            );
            assert!(own.contains("wind_displacement("), "{name}.wgsl uses it");
        }
        for built in [
            crate::pipeline::FORWARD_SHADER,
            crate::shadow::SHADOW_SHADER,
            crate::ao_depth::SHADER,
        ] {
            assert_eq!(built.matches(definition).count(), 1);
        }
    }

    #[test]
    fn angles_become_unit_directions_and_strength_is_bounded() {
        let east = WindField::from_degrees(0.0, 1.0);
        assert!((east.direction[0] - 1.0).abs() < 1e-6 && east.direction[1].abs() < 1e-6);
        let south = WindField::from_degrees(90.0, 1.0);
        assert!(south.direction[0].abs() < 1e-6 && (south.direction[1] - 1.0).abs() < 1e-6);
        for degrees in [0.0, 33.0, 135.0, -70.0, 400.0] {
            let [x, z] = WindField::from_degrees(degrees, 1.0).direction;
            assert!((x * x + z * z - 1.0).abs() < 1e-5, "{degrees}");
        }
        assert_eq!(WindField::from_degrees(0.0, 9.0).strength, 2.0);
        assert_eq!(WindField::from_degrees(0.0, -1.0).strength, 0.0);
        assert_eq!(WindField::from_degrees(f32::NAN, f32::NAN).strength, 0.0);
    }

    #[test]
    fn the_uniform_carries_direction_strength_and_time_and_survives_nan() {
        let mut wind = WindField::from_degrees(90.0, 0.75);
        wind.time = 12.5;
        let lane = wind.uniform();
        assert!(lane.x.abs() < 1e-6 && (lane.y - 1.0).abs() < 1e-6);
        assert_eq!((lane.z, lane.w), (0.75, 12.5));
        wind.time = f32::NAN;
        assert_eq!(wind.uniform().z, 0.0, "a bad clock means still air");
    }

    #[test]
    fn the_default_is_a_gentle_breeze() {
        let wind = WindField::default();
        assert!(wind.strength > 0.0 && wind.strength < 1.0);
        assert_eq!(wind.time, 0.0);
    }
}
