//! Sun shadow geometry: an orthographic shadow camera fitted to a sphere around the viewer, with its
//! position snapped to whole texels so shadow edges do not shimmer when the viewer moves.

use bevy_math::{Mat4, Vec3, proj};
use bevy_transform::components::Transform;

/// Radius in metres of the region around the viewer that receives sun shadows.
pub(crate) const SHADOW_DISTANCE: f32 = 10.0;

/// Shadow settings as the lights uniform carries them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShadowParams {
    /// Whether shadows are sampled at all.
    pub(crate) enabled: bool,
    /// Light-space transform: world position to shadow-map clip space (depth 0 near, 1 far).
    pub(crate) clip_from_world: Mat4,
    /// Filter radius in taps: 0 is one hardware-filtered tap, 1 a 3 by 3 grid.
    pub(crate) pcf_radius: u32,
    /// One texel in shadow-map uv units.
    pub(crate) texel_uv: f32,
    /// Distance in metres to push the lookup position along the surface normal, against acne.
    pub(crate) normal_bias: f32,
}

impl Default for ShadowParams {
    fn default() -> Self {
        Self {
            enabled: false,
            clip_from_world: Mat4::IDENTITY,
            pcf_radius: 0,
            texel_uv: 0.0,
            normal_bias: 0.0,
        }
    }
}

/// How far in texels the lookup is pushed along the normal.
const NORMAL_BIAS_TEXELS: f32 = 1.5;

/// The enabled parameters for a sun shining from `toward_sun` onto a `radius` sphere around `center`.
#[must_use]
pub(crate) fn sun_shadow_params(
    toward_sun: Vec3,
    center: Vec3,
    radius: f32,
    resolution: u32,
    pcf_radius: u32,
) -> ShadowParams {
    #[expect(
        clippy::cast_precision_loss,
        reason = "shadow map sizes are far below 2^24"
    )]
    let texels = resolution.max(1) as f32;
    ShadowParams {
        enabled: true,
        clip_from_world: sun_shadow_matrix(toward_sun, center, radius, resolution),
        pcf_radius,
        texel_uv: 1.0 / texels,
        normal_bias: NORMAL_BIAS_TEXELS * 2.0 * radius / texels,
    }
}

/// The matrix taking world positions to the sun's shadow-map clip space.
///
/// The box is `2 * radius` wide and tall, centred on `center` (snapped to texel steps in the light's
/// own x and y), and reaches `radius` past the centre away from the sun and `2 * radius` toward it, so
/// casters between the sun and the visible region still land in the map.
#[must_use]
pub(crate) fn sun_shadow_matrix(
    toward_sun: Vec3,
    center: Vec3,
    radius: f32,
    resolution: u32,
) -> Mat4 {
    let travel = -toward_sun.normalize_or(Vec3::Y);
    let up = if travel.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let view = light_view(travel, up);
    let centre_in_light = view.transform_point3(center);
    #[expect(
        clippy::cast_precision_loss,
        reason = "shadow map sizes are far below 2^24"
    )]
    let texel = 2.0 * radius / resolution.max(1) as f32;
    let x = (centre_in_light.x / texel).round() * texel;
    let y = (centre_in_light.y / texel).round() * texel;
    // The light looks down its -z axis; larger z is nearer the sun.
    let near = -(centre_in_light.z + 2.0 * radius);
    let far = -(centre_in_light.z - radius);
    proj::orthographic(x - radius, x + radius, y - radius, y + radius, near, far) * view
}

/// The view matrix of a camera at the origin looking along `travel` (right-handed, looking down -z).
fn light_view(travel: Vec3, up: Vec3) -> Mat4 {
    Transform::IDENTITY
        .looking_to(travel, up)
        .to_matrix()
        .inverse()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOWARD: Vec3 = Vec3::new(0.4, 0.8, 0.45);

    fn project(matrix: Mat4, point: Vec3) -> Vec3 {
        matrix.project_point3(point)
    }

    #[test]
    fn the_region_around_the_viewer_fits_the_shadow_map() {
        let center = Vec3::new(3.0, 1.5, -2.0);
        let matrix = sun_shadow_matrix(TOWARD, center, 10.0, 2048);
        let toward = TOWARD.normalize();
        for direction in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, -Vec3::Y, -Vec3::Z] {
            // The texel snap can move the box by up to half a texel, so stay just inside the sphere.
            let ndc = project(matrix, center + direction * 9.9);
            assert!(
                ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0,
                "{direction}: {ndc}"
            );
            assert!((0.0..=1.0).contains(&ndc.z), "{direction}: depth {}", ndc.z);
        }
        // A caster well toward the sun is still in the depth range, and nearer the light than the centre.
        let caster = project(matrix, center + toward * 18.0);
        let middle = project(matrix, center);
        assert!((0.0..=1.0).contains(&caster.z));
        assert!(caster.z < middle.z);
    }

    #[test]
    fn the_box_does_not_move_for_sub_texel_viewer_motion() {
        let texel = 2.0 * 10.0 / 2048.0;
        // Start from a centre that sits exactly on a texel, and move along the light's own x axis.
        let to_world = light_view(-TOWARD.normalize(), Vec3::Y).inverse();
        let base = to_world.transform_point3(Vec3::new(5.0 * texel, 3.0 * texel, -4.0));
        let sideways = to_world.transform_vector3(Vec3::X);
        let a = sun_shadow_matrix(TOWARD, base, 10.0, 2048);
        let b = sun_shadow_matrix(TOWARD, base + sideways * texel * 0.2, 10.0, 2048);
        let c = sun_shadow_matrix(TOWARD, base + sideways * texel * 3.0, 10.0, 2048);
        assert!(a.abs_diff_eq(b, 1e-6), "a tiny move changed the shadow box");
        assert!(
            !a.abs_diff_eq(c, 1e-6),
            "a several-texel move did not move the box"
        );
    }

    #[test]
    fn a_sun_straight_overhead_still_builds_a_valid_matrix() {
        let matrix = sun_shadow_matrix(Vec3::Y, Vec3::ZERO, 10.0, 1024);
        assert!(matrix.is_finite());
        let ndc = project(matrix, Vec3::new(2.0, 0.0, 3.0));
        assert!(ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0);
    }

    #[test]
    fn a_degenerate_sun_direction_falls_back_instead_of_producing_nan() {
        assert!(sun_shadow_matrix(Vec3::ZERO, Vec3::ZERO, 10.0, 1024).is_finite());
    }

    #[test]
    fn params_scale_with_resolution() {
        let low = sun_shadow_params(TOWARD, Vec3::ZERO, 10.0, 1024, 0);
        let high = sun_shadow_params(TOWARD, Vec3::ZERO, 10.0, 2048, 1);
        assert!(low.enabled && high.enabled);
        assert!((low.texel_uv - 2.0 * high.texel_uv).abs() < 1e-9);
        assert!((low.normal_bias - 2.0 * high.normal_bias).abs() < 1e-6);
        assert_eq!(high.pcf_radius, 1);
    }

    #[test]
    fn default_params_are_off() {
        assert!(!ShadowParams::default().enabled);
    }
}
