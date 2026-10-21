//! Our light units in Bevy's photometric ones.
//!
//! Our lights and surfaces are scene-referred: a sun of intensity 3 on a white surface gives an outgoing radiance of 3. Bevy's lights
//! are in lux, lumens and cd/m^2, and a camera's `Exposure` scales them back down (default EV100 9.7, a factor of `1 / (1.2 * 2^EV100)`).
//! With that exposure one of our units is `SCENE_UNIT_CD` cd/m^2 of radiance, which is `pi` times as many lux of irradiance.

use core::f32::consts::{FRAC_PI_2, PI};

/// The exposure the cameras use, as an EV100 (Bevy's default for outdoor scenes).
pub const EXPOSURE_EV100: f32 = 9.7;

/// The widest half angle Bevy accepts for a spot light's outer cone (it must stay under pi / 2).
pub const MAX_SPOT_HALF_ANGLE: f32 = FRAC_PI_2 - 0.01;

/// cd/m^2 of radiance that one scene unit stands for at [`EXPOSURE_EV100`].
#[must_use]
pub fn scene_unit_cd() -> f32 {
    1.2 * EXPOSURE_EV100.exp2()
}

/// Illuminance in lux for a directional light of our intensity.
#[must_use]
pub fn sun_lux(intensity: f32) -> f32 {
    intensity * PI * scene_unit_cd()
}

/// Luminous power in lumens for a point or spot light whose intensity is the irradiance it gives at one metre.
#[must_use]
pub fn point_lumens(intensity: f32) -> f32 {
    4.0 * PI * sun_lux(intensity)
}

/// Brightness in cd/m^2 for an ambient light of our intensity.
#[must_use]
pub fn ambient_brightness(intensity: f32) -> f32 {
    intensity * scene_unit_cd()
}

/// Bevy's `(inner, outer)` spot half angles for our full cone angles: halved, ordered, and kept below the limit.
#[must_use]
pub fn spot_half_angles(inner_full: f32, outer_full: f32) -> (f32, f32) {
    let outer = (outer_full * 0.5).clamp(0.0, MAX_SPOT_HALF_ANGLE);
    let inner = (inner_full * 0.5).clamp(0.0, outer);
    (inner, outer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_unit_is_about_a_thousand_candelas_at_the_default_exposure() {
        let cd = scene_unit_cd();
        assert!((cd - 998.1).abs() < 0.1, "{cd}");
    }

    #[test]
    fn a_sun_of_three_gives_about_nine_thousand_lux() {
        let lux = sun_lux(3.0);
        assert!((lux - 9_407.0).abs() < 5.0, "{lux}");
    }

    #[test]
    fn exposure_undoes_the_scale_so_a_white_surface_under_the_sun_gives_back_the_intensity() {
        // radiance = albedo / pi * lux * exposure, with exposure = 1 / scene_unit_cd.
        let radiance = sun_lux(3.0) / PI / scene_unit_cd();
        assert!((radiance - 3.0).abs() < 1e-4);
    }

    #[test]
    fn ambient_follows_the_same_scale_without_pi() {
        assert!((ambient_brightness(0.5) / scene_unit_cd() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_point_light_gives_its_intensity_in_lux_at_one_metre() {
        // candela = lumens / 4 pi, and lux at one metre equals candela.
        let lux_at_one_metre = point_lumens(2.0) / (4.0 * PI);
        assert!((lux_at_one_metre - sun_lux(2.0)).abs() < 1.0);
    }

    #[test]
    fn spot_angles_are_halved_ordered_and_limited() {
        assert_eq!(spot_half_angles(0.4, 0.8), (0.2, 0.4));
        let (inner, outer) = spot_half_angles(3.0, 2.0);
        assert!(inner <= outer, "inner {inner} outer {outer}");
        let (_, wide) = spot_half_angles(0.0, 10.0);
        assert!(wide < FRAC_PI_2);
    }
}
