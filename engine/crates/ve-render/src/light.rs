//! Lights: components, per-frame extraction and packing into a fixed-size uniform block.

use bevy_color::{Color, LinearRgba};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Vec3, Vec4};
use bevy_render::Extract;
use bevy_render::render_resource::ShaderType;
use bevy_transform::components::GlobalTransform;

use crate::shadow_math::{MAX_CASCADES, ShadowParams};

/// Upper bound on point lights per frame. Fixed so the uniform block has a known size on WebGL2;
/// the quality tiers (M6) choose how many are actually used.
pub const MAX_POINT_LIGHTS: usize = 16;
/// Fixed WebGL2 uniform capacity for unshadowed spot lights.
pub const MAX_SPOT_LIGHTS: usize = 8;

/// A light infinitely far away, shining along its entity's forward (-Z) direction.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct VeDirectionalLight {
    /// Light colour.
    pub color: Color,
    /// Brightness; around 3 is a bright sun in the engine's default exposure.
    pub intensity: f32,
}

impl Default for VeDirectionalLight {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            intensity: 3.0,
        }
    }
}

/// A light radiating from its entity's position, fading to nothing at `range` metres.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct VePointLight {
    /// Light colour.
    pub color: Color,
    /// Brightness at one metre.
    pub intensity: f32,
    /// Distance in metres beyond which the light has no effect.
    pub range: f32,
}

impl Default for VePointLight {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            intensity: 10.0,
            range: 10.0,
        }
    }
}

/// A cone light, currently unshadowed. Angles are full cone widths in radians.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct VeSpotLight {
    /// Light colour.
    pub color: Color,
    /// Brightness at one metre.
    pub intensity: f32,
    /// Distance in metres where the light reaches zero.
    pub range: f32,
    /// Normalized world direction the cone points toward.
    pub direction: Vec3,
    /// Full cone angle at full intensity.
    pub inner_angle: f32,
    /// Full cone angle at zero intensity.
    pub outer_angle: f32,
}

/// Constant light that reaches every surface equally, standing in for the sky.
///
/// The sky and ground colours are packed as a first-order (L1) spherical-harmonic irradiance band.
/// This is a bounded, stable approximation of a two-colour environment: it captures the sky-to-ground
/// gradient a real environment would add, but no inter-surface bounce. Scenes that ask for light probes
/// (`probes.rs`) use traced irradiance instead.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct VeAmbientLight {
    /// Light colour.
    pub color: Color,
    /// Colour of diffuse light arriving from above; multiplied by `intensity`.
    pub sky_color: Color,
    /// Colour of diffuse light arriving from below; multiplied by `intensity`.
    pub ground_color: Color,
    /// Brightness.
    pub intensity: f32,
    /// How strongly surfaces reflect a procedural studio sky (a gradient with two soft light boxes),
    /// so metals and glossy paint have something to reflect. 0 turns it off.
    pub reflection: f32,
    /// Colour of the optional warm ground bounce; multiplied by `intensity`.
    pub bounce_color: Color,
    /// Strength of the optional ground bounce added to downward-facing surfaces, 0 to 1. 0 (the
    /// default) matches a plain two-colour hemisphere; this is an approximation, not GI.
    pub bounce: f32,
}

impl Default for VeAmbientLight {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            sky_color: Color::srgb(0.68, 0.76, 0.9),
            ground_color: Color::srgb(0.46, 0.39, 0.3),
            intensity: 0.15,
            reflection: 0.0,
            bounce_color: Color::srgb(0.6, 0.44, 0.28),
            bounce: 0.0,
        }
    }
}

/// One point light as seen by the shader.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub(crate) struct PointLightUniform {
    /// xyz: world position, w: range.
    pub(crate) position_range: Vec4,
    /// rgb: colour times intensity, w: unused.
    pub(crate) color: Vec4,
}

/// Every light in the scene, as one uniform block.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub(crate) struct LightsUniform {
    /// rgb: ambient colour times intensity, w: strength of the reflected studio sky.
    pub(crate) ambient: Vec4,
    /// rgb: constant (L0) term of the sky/ground irradiance band, already intensity-scaled.
    pub(crate) ambient_sh0: Vec4,
    /// rgb: first-order (L1) term of the sky/ground irradiance band, aligned to +Y.
    pub(crate) ambient_sh1: Vec4,
    /// rgb: warm ground-bounce colour times intensity, w: bounce strength (0 turns it off).
    pub(crate) ambient_bounce: Vec4,
    /// xyz: unit vector pointing from the surface toward the sun.
    pub(crate) sun_direction: Vec4,
    /// rgb: sun colour times intensity (zero when there is no sun).
    pub(crate) sun_color: Vec4,
    /// x: number of valid entries in `points`.
    pub(crate) counts: Vec4,
    /// World position to the sun's shadow-map clip space, one matrix per cascade.
    pub(crate) sun_shadow_from_world: [Mat4; MAX_CASCADES],
    /// x: number of shadow cascades (0 when the shadow map is not sampled), y: filter radius in taps, z and w: one texel of the
    /// atlas in uv units along x and y.
    pub(crate) shadow_params: Vec4,
    /// xyz: the far edge of each cascade along the viewing direction, in metres.
    pub(crate) shadow_splits: Vec4,
    /// xyz: each cascade's normal-offset bias in metres.
    pub(crate) shadow_bias: Vec4,
    /// xyz: the viewing direction the splits are measured along.
    pub(crate) shadow_forward: Vec4,
    /// xyz: world position of the first light probe, w: the distance between probes.
    pub(crate) gi_origin_spacing: Vec4,
    /// xyz: how many probes lie along each axis, w: 1 when a probe volume lights the scene, 0 for the constant sky ambient.
    pub(crate) gi_dims: Vec4,
    pub(crate) points: [PointLightUniform; MAX_POINT_LIGHTS],
    /// Bounded spot array. Angles are packed as outer and inner cosines.
    pub(crate) spots: [SpotLightUniform; MAX_SPOT_LIGHTS],
}

/// One spot light as consumed by the forward shader.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub(crate) struct SpotLightUniform {
    /// xyz: world position; w: range.
    pub(crate) position_range: Vec4,
    /// xyz: normalized ray direction; w: outer cone cosine.
    pub(crate) direction_outer: Vec4,
    /// rgb: colour times intensity; w: inner cone cosine.
    pub(crate) color_inner: Vec4,
}

/// A point light extracted from the world, before packing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ExtractedPoint {
    pub(crate) position: Vec3,
    pub(crate) color: LinearRgba,
    pub(crate) intensity: f32,
    pub(crate) range: f32,
}

/// The lights visible this frame.
#[derive(Resource, Default, Debug)]
pub(crate) struct ExtractedLights {
    pub(crate) ambient: Option<VeAmbientLight>,
    pub(crate) sun: Option<(Vec3, VeDirectionalLight)>,
    pub(crate) points: Vec<ExtractedPoint>,
    pub(crate) spots: Vec<ExtractedSpot>,
}

/// A spot light extracted from the main world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ExtractedSpot {
    pub(crate) position: Vec3,
    pub(crate) direction: Vec3,
    pub(crate) color: LinearRgba,
    pub(crate) intensity: f32,
    pub(crate) range: f32,
    pub(crate) inner_angle: f32,
    pub(crate) outer_angle: f32,
}

/// Replaces [`ExtractedLights`] with the lights of the main world.
pub(crate) fn extract_lights(
    mut out: ResMut<ExtractedLights>,
    ambient: Extract<Option<Res<VeAmbientLight>>>,
    suns: Extract<Query<(&VeDirectionalLight, &GlobalTransform)>>,
    points: Extract<Query<(&VePointLight, &GlobalTransform)>>,
    spots: Extract<Query<(&VeSpotLight, &GlobalTransform)>>,
) {
    out.ambient = ambient.as_deref().copied();
    // With several suns, the brightest wins; the rest are ignored (one shadow-capable sun by design).
    out.sun = suns
        .iter()
        .max_by(|a, b| a.0.intensity.total_cmp(&b.0.intensity))
        .map(|(light, transform)| (transform.forward().as_vec3(), *light));
    out.points.clear();
    out.points
        .extend(points.iter().map(|(light, transform)| ExtractedPoint {
            position: transform.translation(),
            color: LinearRgba::from(light.color),
            intensity: light.intensity,
            range: light.range,
        }));
    out.spots.clear();
    out.spots
        .extend(spots.iter().map(|(light, transform)| ExtractedSpot {
            position: transform.translation(),
            direction: light.direction.normalize_or_zero(),
            color: LinearRgba::from(light.color),
            intensity: light.intensity,
            range: light.range,
            inner_angle: light.inner_angle,
            outer_angle: light.outer_angle,
        }));
}

fn scaled(color: LinearRgba, intensity: f32) -> Vec4 {
    Vec4::new(
        color.red * intensity,
        color.green * intensity,
        color.blue * intensity,
        0.0,
    )
}

fn point_order(a: &ExtractedPoint, b: &ExtractedPoint, viewer: Vec3) -> std::cmp::Ordering {
    a.position
        .distance_squared(viewer)
        .total_cmp(&b.position.distance_squared(viewer))
        .then_with(|| a.position.x.total_cmp(&b.position.x))
        .then_with(|| a.position.y.total_cmp(&b.position.y))
        .then_with(|| a.position.z.total_cmp(&b.position.z))
        .then_with(|| a.range.total_cmp(&b.range))
        .then_with(|| a.intensity.total_cmp(&b.intensity))
        .then_with(|| a.color.red.total_cmp(&b.color.red))
        .then_with(|| a.color.green.total_cmp(&b.color.green))
        .then_with(|| a.color.blue.total_cmp(&b.color.blue))
}

fn compare_spots(a: &ExtractedSpot, b: &ExtractedSpot, viewer: Vec3) -> std::cmp::Ordering {
    a.position
        .distance_squared(viewer)
        .total_cmp(&b.position.distance_squared(viewer))
        .then_with(|| a.position.x.total_cmp(&b.position.x))
        .then_with(|| a.position.y.total_cmp(&b.position.y))
        .then_with(|| a.position.z.total_cmp(&b.position.z))
        .then_with(|| a.direction.x.total_cmp(&b.direction.x))
        .then_with(|| a.direction.y.total_cmp(&b.direction.y))
        .then_with(|| a.direction.z.total_cmp(&b.direction.z))
        .then_with(|| a.range.total_cmp(&b.range))
        .then_with(|| a.inner_angle.total_cmp(&b.inner_angle))
        .then_with(|| a.outer_angle.total_cmp(&b.outer_angle))
        .then_with(|| a.intensity.total_cmp(&b.intensity))
        .then_with(|| a.color.red.total_cmp(&b.color.red))
        .then_with(|| a.color.green.total_cmp(&b.color.green))
        .then_with(|| a.color.blue.total_cmp(&b.color.blue))
}

/// Writes the cascaded shadow settings into the uniform block.
fn pack_shadow(uniform: &mut LightsUniform, shadow: &ShadowParams) {
    uniform.sun_shadow_from_world = shadow.clip_from_world;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the cascade count is at most 3 and the filter radius 0 or 1"
    )]
    let (cascades, radius) = (shadow.cascades as f32, shadow.pcf_radius as f32);
    uniform.shadow_params = Vec4::new(cascades, radius, shadow.texel_uv[0], shadow.texel_uv[1]);
    uniform.shadow_splits = Vec4::new(shadow.splits[0], shadow.splits[1], shadow.splits[2], 0.0);
    let bias = shadow.normal_bias;
    uniform.shadow_bias = Vec4::new(bias[0], bias[1], bias[2], 0.0);
    uniform.shadow_forward = shadow.forward.extend(0.0);
}

/// Packs the extracted lights into the uniform block, keeping at most `max_points` point lights.
///
/// When there are more point lights than allowed, the ones closest to `viewer` are kept because
/// they contribute the most. The result is deterministic for equal inputs.
pub(crate) fn pack_lights(
    lights: &ExtractedLights,
    viewer: Vec3,
    max_points: usize,
    max_spots: usize,
    shadow: &ShadowParams,
) -> LightsUniform {
    let mut uniform = LightsUniform::default();
    if let Some(ambient) = lights.ambient {
        uniform.ambient = scaled(LinearRgba::from(ambient.color), ambient.intensity);
        uniform.ambient.w = ambient.reflection.max(0.0);
        // A two-colour hemisphere is exactly the L1 band along Y, so the constant and directional
        // terms below are the mean and half-difference of the sky and ground irradiance.
        let sky = LinearRgba::from(ambient.sky_color);
        let ground = LinearRgba::from(ambient.ground_color);
        let half = 0.5 * ambient.intensity;
        let mid = Vec4::new(
            (sky.red + ground.red) * half,
            (sky.green + ground.green) * half,
            (sky.blue + ground.blue) * half,
            0.0,
        );
        let dir = Vec4::new(
            (sky.red - ground.red) * half,
            (sky.green - ground.green) * half,
            (sky.blue - ground.blue) * half,
            0.0,
        );
        uniform.ambient_sh0 = mid;
        uniform.ambient_sh1 = dir;
        uniform.ambient_bounce = scaled(LinearRgba::from(ambient.bounce_color), ambient.intensity);
        uniform.ambient_bounce.w = ambient.bounce.max(0.0);
    }
    if let Some((forward, sun)) = &lights.sun {
        // The shader wants the direction toward the light, the opposite of where it points.
        let toward = (-*forward).normalize_or_zero();
        uniform.sun_direction = toward.extend(0.0);
        uniform.sun_color = scaled(LinearRgba::from(sun.color), sun.intensity);
        if shadow.enabled {
            pack_shadow(&mut uniform, shadow);
        }
    }

    let mut order: Vec<&ExtractedPoint> = lights.points.iter().collect();
    order.sort_by(|a, b| point_order(a, b, viewer));
    let count = order.len().min(max_points.min(MAX_POINT_LIGHTS));
    for (slot, light) in uniform.points.iter_mut().zip(order.iter().take(count)) {
        slot.position_range = light.position.extend(light.range.max(0.0));
        slot.color = scaled(light.color, light.intensity);
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "count is at most MAX_POINT_LIGHTS"
    )]
    {
        uniform.counts.x = count as f32;
    }
    let mut spot_order: Vec<&ExtractedSpot> = lights.spots.iter().collect();
    spot_order.sort_by(|a, b| compare_spots(a, b, viewer));
    let spot_count = spot_order.len().min(max_spots.min(MAX_SPOT_LIGHTS));
    for (slot, light) in uniform
        .spots
        .iter_mut()
        .zip(spot_order.iter().take(spot_count))
    {
        slot.position_range = light.position.extend(light.range.max(0.0));
        slot.direction_outer = light
            .direction
            .normalize_or_zero()
            .extend((light.outer_angle * 0.5).cos());
        slot.color_inner = scaled(light.color, light.intensity);
        slot.color_inner.w = (light.inner_angle * 0.5).cos();
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "count is at most MAX_SPOT_LIGHTS"
    )]
    {
        uniform.counts.y = spot_count as f32;
    }
    uniform
}

#[cfg(test)]
#[path = "light_tests.rs"]
mod tests;
