//! Sun shadow geometry: cascaded orthographic shadow cameras.
//!
//! The view is cut into slices by distance. Each slice gets one orthographic sun camera fitted to the slice's bounding *sphere*, so a
//! cascade keeps the same size as the viewer turns, and its position snaps to whole texels as the viewer moves: shadow edges do not
//! shimmer. Near cascades cover little ground and are sharp; far cascades cover a lot and are soft.

use bevy_math::{Mat4, Quat, Vec3, proj};
use bevy_transform::components::Transform;

/// The most cascades one frame uses.
pub(crate) const MAX_CASCADES: usize = 3;

/// How far toward the sun every cascade reaches for casters, in metres at least, so a tall wall or tree outside a small near cascade
/// still shades it.
const CASTER_REACH: f32 = 80.0;

/// Nearest distance the cascades start from, in metres.
const NEAR: f32 = 0.1;

/// Shadow settings as the lights uniform carries them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShadowParams {
    /// Whether shadows are sampled at all.
    pub(crate) enabled: bool,
    /// How many cascades are in use (1 to `MAX_CASCADES`).
    pub(crate) cascades: usize,
    /// Per cascade: world position to shadow-map clip space (depth 0 near, 1 far).
    pub(crate) clip_from_world: [Mat4; MAX_CASCADES],
    /// Per cascade: the far edge of its slice, in metres along the viewing direction.
    pub(crate) splits: [f32; MAX_CASCADES],
    /// Per cascade: distance in metres to push the lookup position along the surface normal, against acne.
    pub(crate) normal_bias: [f32; MAX_CASCADES],
    /// Filter radius in taps: 0 is one hardware-filtered tap, 1 a 3 by 3 grid.
    pub(crate) pcf_radius: u32,
    /// One texel of the atlas in uv units, along x and y. The atlas lays the cascades side by side.
    pub(crate) texel_uv: [f32; 2],
    /// The viewing direction the splits are measured along.
    pub(crate) forward: Vec3,
}

impl Default for ShadowParams {
    fn default() -> Self {
        Self {
            enabled: false,
            cascades: 0,
            clip_from_world: [Mat4::IDENTITY; MAX_CASCADES],
            splits: [0.0; MAX_CASCADES],
            normal_bias: [0.0; MAX_CASCADES],
            pcf_radius: 0,
            texel_uv: [0.0; 2],
            forward: Vec3::NEG_Z,
        }
    }
}

/// How far in texels the lookup is pushed along the normal.
const NORMAL_BIAS_TEXELS: f32 = 1.5;

/// The far edge of each cascade for `count` cascades, in metres. Unused entries repeat the last.
#[must_use]
pub(crate) fn cascade_splits(count: usize) -> [f32; MAX_CASCADES] {
    let used: &[f32] = match count {
        0 | 1 => &[30.0],
        2 => &[12.0, 60.0],
        _ => &[8.0, 30.0, 100.0],
    };
    let mut splits = [used[used.len() - 1]; MAX_CASCADES];
    splits[..used.len()].copy_from_slice(used);
    splits
}

/// The bounding sphere of the slice of a view frustum between `near` and `far`: its centre's distance along the viewing direction and
/// its radius. `tan_x` and `tan_y` are the tangents of the half field of view.
#[must_use]
pub(crate) fn slice_sphere(near: f32, far: f32, tan_x: f32, tan_y: f32) -> (f32, f32) {
    let k2 = tan_x * tan_x + tan_y * tan_y;
    // On the axis, equally far from the nearest and the farthest corner; never beyond the far plane.
    let centre = (0.5 * (far + near) * (1.0 + k2)).min(far);
    let radius = ((far - centre).powi(2) + far * far * k2).sqrt();
    (centre, radius)
}

/// The enabled parameters for a sun shining from `toward_sun` onto a camera at `camera` looking along `forward`.
///
/// `resolution` is the side of one cascade in texels; the atlas is `resolution * cascades` wide.
#[must_use]
pub(crate) fn sun_cascades(
    toward_sun: Vec3,
    camera: Vec3,
    forward: Vec3,
    tan: [f32; 2],
    cascades: usize,
    resolution: u32,
    pcf_radius: u32,
) -> ShadowParams {
    let count = cascades.clamp(1, MAX_CASCADES);
    let forward = forward.normalize_or(Vec3::NEG_Z);
    let splits = cascade_splits(count);
    #[expect(
        clippy::cast_precision_loss,
        reason = "shadow map sizes are far below 2^24"
    )]
    let texels = resolution.max(1) as f32;
    let mut clip_from_world = [Mat4::IDENTITY; MAX_CASCADES];
    let mut normal_bias = [0.0; MAX_CASCADES];
    let mut near = NEAR;
    for index in 0..count {
        let (centre, radius) = slice_sphere(near, splits[index], tan[0], tan[1]);
        clip_from_world[index] = sun_shadow_matrix(
            toward_sun,
            camera + forward * centre,
            radius,
            resolution,
            CASTER_REACH.max(2.0 * radius),
        );
        normal_bias[index] = NORMAL_BIAS_TEXELS * 2.0 * radius / texels;
        near = splits[index];
    }
    #[expect(clippy::cast_precision_loss, reason = "at most three cascades")]
    let atlas_width = texels * count as f32;
    ShadowParams {
        enabled: true,
        cascades: count,
        clip_from_world,
        splits,
        normal_bias,
        pcf_radius,
        texel_uv: [1.0 / atlas_width, 1.0 / texels],
        forward,
    }
}

/// The matrix taking world positions to one cascade's shadow-map clip space.
///
/// The box is `2 * radius` wide and tall, centred on `center` (snapped to texel steps in the light's own x and y), and reaches `radius`
/// past the centre away from the sun and `reach` toward it, so casters between the sun and the visible region still land in the map.
#[must_use]
pub(crate) fn sun_shadow_matrix(
    toward_sun: Vec3,
    center: Vec3,
    radius: f32,
    resolution: u32,
    reach: f32,
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
    let near = -(centre_in_light.z + reach.max(radius));
    let far = -(centre_in_light.z - radius);
    proj::orthographic(x - radius, x + radius, y - radius, y + radius, near, far) * view
}

/// The viewing direction and the tangents of the half field of view (x, then y) of a camera turned by `rotation` with the projection
/// `clip_from_view`. A projection with no usable scale counts as a tangent of 1.
#[must_use]
pub(crate) fn view_frustum(rotation: Quat, clip_from_view: Mat4) -> (Vec3, [f32; 2]) {
    let tan = |scale: f32| {
        if scale.abs() > 1e-4 {
            (1.0 / scale).abs()
        } else {
            1.0
        }
    };
    (
        rotation * Vec3::NEG_Z,
        [tan(clip_from_view.x_axis.x), tan(clip_from_view.y_axis.y)],
    )
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
    /// A 60 degree vertical field of view at 16:9.
    const TAN: [f32; 2] = [0.5774 * 16.0 / 9.0, 0.5774];

    fn project(matrix: Mat4, point: Vec3) -> Vec3 {
        matrix.project_point3(point)
    }

    #[test]
    fn the_region_around_a_centre_fits_the_shadow_map() {
        let center = Vec3::new(3.0, 1.5, -2.0);
        let matrix = sun_shadow_matrix(TOWARD, center, 10.0, 2048, 20.0);
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
    fn the_box_does_not_move_for_sub_texel_motion() {
        let texel = 2.0 * 10.0 / 2048.0;
        // Start from a centre that sits exactly on a texel, and move along the light's own x axis.
        let to_world = light_view(-TOWARD.normalize(), Vec3::Y).inverse();
        let base = to_world.transform_point3(Vec3::new(5.0 * texel, 3.0 * texel, -4.0));
        let sideways = to_world.transform_vector3(Vec3::X);
        let a = sun_shadow_matrix(TOWARD, base, 10.0, 2048, 20.0);
        let b = sun_shadow_matrix(TOWARD, base + sideways * texel * 0.2, 10.0, 2048, 20.0);
        let c = sun_shadow_matrix(TOWARD, base + sideways * texel * 3.0, 10.0, 2048, 20.0);
        assert!(a.abs_diff_eq(b, 1e-6), "a tiny move changed the shadow box");
        assert!(
            !a.abs_diff_eq(c, 1e-6),
            "a several-texel move did not move the box"
        );
    }

    #[test]
    fn a_sun_straight_overhead_still_builds_a_valid_matrix() {
        let matrix = sun_shadow_matrix(Vec3::Y, Vec3::ZERO, 10.0, 1024, 20.0);
        assert!(matrix.is_finite());
        let ndc = project(matrix, Vec3::new(2.0, 0.0, 3.0));
        assert!(ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0);
    }

    #[test]
    fn a_degenerate_sun_direction_falls_back_instead_of_producing_nan() {
        assert!(sun_shadow_matrix(Vec3::ZERO, Vec3::ZERO, 10.0, 1024, 20.0).is_finite());
    }

    #[test]
    fn the_splits_grow_and_unused_entries_repeat_the_last() {
        for count in 1..=MAX_CASCADES {
            let splits = cascade_splits(count);
            assert!(
                splits.windows(2).all(|pair| pair[0] <= pair[1]),
                "{splits:?}"
            );
            assert!(splits[0] > NEAR);
            assert!((splits[count - 1] - splits[MAX_CASCADES - 1]).abs() < 1e-6);
        }
        assert!(
            cascade_splits(3)[0] < cascade_splits(1)[0],
            "more cascades means a sharper first one"
        );
    }

    #[test]
    fn a_slice_sphere_holds_every_corner_of_its_slice() {
        for (near, far) in [(0.1, 8.0), (8.0, 30.0), (30.0, 100.0)] {
            let (centre, radius) = slice_sphere(near, far, TAN[0], TAN[1]);
            assert!(centre <= far + 1e-4);
            for depth in [near, far] {
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    let corner = Vec3::new(sx * depth * TAN[0], sy * depth * TAN[1], depth);
                    let distance = (corner - Vec3::new(0.0, 0.0, centre)).length();
                    assert!(
                        distance <= radius + 1e-3,
                        "{near}-{far}: corner {distance} > radius {radius}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_point_in_view_lands_inside_the_cascade_that_owns_its_distance() {
        let camera = Vec3::new(1.0, 1.6, 2.0);
        let forward = Vec3::new(0.3, -0.1, -1.0).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        let params = sun_cascades(TOWARD, camera, forward, TAN, 3, 2048, 1);
        assert!(params.enabled && params.cascades == 3);
        let mut near = NEAR;
        for index in 0..3 {
            let far = params.splits[index];
            for depth in [near + 0.01, 0.5 * (near + far), far - 0.01] {
                for (sx, sy) in [(-0.95, -0.95), (0.95, -0.95), (0.0, 0.0), (0.95, 0.95)] {
                    let point = camera
                        + forward * depth
                        + right * (sx * depth * TAN[0])
                        + up * (sy * depth * TAN[1]);
                    let ndc = project(params.clip_from_world[index], point);
                    assert!(
                        ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z),
                        "cascade {index} at depth {depth}: {ndc}"
                    );
                }
            }
            near = far;
        }
    }

    #[test]
    fn a_tall_caster_toward_the_sun_still_reaches_the_nearest_cascade() {
        let params = sun_cascades(TOWARD, Vec3::ZERO, Vec3::NEG_Z, TAN, 3, 1024, 0);
        let caster = Vec3::new(0.0, 0.0, -3.0) + TOWARD.normalize() * 40.0;
        let ndc = project(params.clip_from_world[0], caster);
        assert!(
            (0.0..=1.0).contains(&ndc.z),
            "a wall 40 m up the sun's ray is outside the depth range: {ndc}"
        );
    }

    #[test]
    fn params_scale_with_resolution_and_cascade_count() {
        let low = sun_cascades(TOWARD, Vec3::ZERO, Vec3::NEG_Z, TAN, 2, 1024, 0);
        let high = sun_cascades(TOWARD, Vec3::ZERO, Vec3::NEG_Z, TAN, 2, 2048, 1);
        assert!((low.texel_uv[0] - 2.0 * high.texel_uv[0]).abs() < 1e-9);
        assert!((low.texel_uv[1] - 2.0 * high.texel_uv[1]).abs() < 1e-9);
        assert!((low.normal_bias[0] - 2.0 * high.normal_bias[0]).abs() < 1e-6);
        assert_eq!(high.pcf_radius, 1);
        let three = sun_cascades(TOWARD, Vec3::ZERO, Vec3::NEG_Z, TAN, 3, 1024, 0);
        assert!(
            (low.texel_uv[0] * 2.0 - three.texel_uv[0] * 3.0).abs() < 1e-9,
            "the atlas is as wide as its cascades"
        );
    }

    #[test]
    fn default_params_are_off() {
        assert!(!ShadowParams::default().enabled);
    }
}
