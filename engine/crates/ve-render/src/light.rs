//! Lights: components, per-frame extraction and packing into a fixed-size uniform block.

use bevy_color::{Color, LinearRgba};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Vec3, Vec4};
use bevy_render::Extract;
use bevy_render::render_resource::ShaderType;
use bevy_transform::components::GlobalTransform;

use crate::shadow_math::ShadowParams;

/// Upper bound on point lights per frame. Fixed so the uniform block has a known size on WebGL2;
/// the quality tiers (M6) choose how many are actually used.
pub const MAX_POINT_LIGHTS: usize = 8;

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

/// Constant light that reaches every surface equally, standing in for the sky.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct VeAmbientLight {
    /// Light colour.
    pub color: Color,
    /// Brightness.
    pub intensity: f32,
    /// How strongly surfaces reflect a procedural studio sky (a gradient with two soft light boxes),
    /// so metals and glossy paint have something to reflect. 0 turns it off.
    pub reflection: f32,
}

impl Default for VeAmbientLight {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            intensity: 0.15,
            reflection: 0.0,
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
    /// xyz: unit vector pointing from the surface toward the sun.
    pub(crate) sun_direction: Vec4,
    /// rgb: sun colour times intensity (zero when there is no sun).
    pub(crate) sun_color: Vec4,
    /// x: number of valid entries in `points`.
    pub(crate) counts: Vec4,
    /// World position to the sun's shadow-map clip space.
    pub(crate) sun_shadow_from_world: Mat4,
    /// x: 1 when the sun's shadow map is sampled, y: filter radius in taps, z: one texel in uv units,
    /// w: normal-offset bias in metres.
    pub(crate) shadow_params: Vec4,
    pub(crate) points: [PointLightUniform; MAX_POINT_LIGHTS],
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
}

/// Replaces [`ExtractedLights`] with the lights of the main world.
pub(crate) fn extract_lights(
    mut out: ResMut<ExtractedLights>,
    ambient: Extract<Option<Res<VeAmbientLight>>>,
    suns: Extract<Query<(&VeDirectionalLight, &GlobalTransform)>>,
    points: Extract<Query<(&VePointLight, &GlobalTransform)>>,
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
}

fn scaled(color: LinearRgba, intensity: f32) -> Vec4 {
    Vec4::new(
        color.red * intensity,
        color.green * intensity,
        color.blue * intensity,
        0.0,
    )
}

/// Packs the extracted lights into the uniform block, keeping at most `max_points` point lights.
///
/// When there are more point lights than allowed, the ones closest to `viewer` are kept because
/// they contribute the most. The result is deterministic for equal inputs.
pub(crate) fn pack_lights(
    lights: &ExtractedLights,
    viewer: Vec3,
    max_points: usize,
    shadow: &ShadowParams,
) -> LightsUniform {
    let mut uniform = LightsUniform::default();
    if let Some(ambient) = lights.ambient {
        uniform.ambient = scaled(LinearRgba::from(ambient.color), ambient.intensity);
        uniform.ambient.w = ambient.reflection.max(0.0);
    }
    if let Some((forward, sun)) = &lights.sun {
        // The shader wants the direction toward the light, the opposite of where it points.
        let toward = (-*forward).normalize_or_zero();
        uniform.sun_direction = toward.extend(0.0);
        uniform.sun_color = scaled(LinearRgba::from(sun.color), sun.intensity);
        if shadow.enabled {
            uniform.sun_shadow_from_world = shadow.clip_from_world;
            #[expect(clippy::cast_precision_loss, reason = "the filter radius is 0 or 1")]
            let radius = shadow.pcf_radius as f32;
            uniform.shadow_params = Vec4::new(1.0, radius, shadow.texel_uv, shadow.normal_bias);
        }
    }

    let mut order: Vec<&ExtractedPoint> = lights.points.iter().collect();
    order.sort_by(|a, b| {
        a.position
            .distance_squared(viewer)
            .total_cmp(&b.position.distance_squared(viewer))
    });
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
    uniform
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32) -> ExtractedPoint {
        ExtractedPoint {
            position: Vec3::new(x, 0.0, 0.0),
            color: LinearRgba::WHITE,
            intensity: 1.0,
            range: 5.0,
        }
    }

    const NO_SHADOW: ShadowParams = ShadowParams {
        enabled: false,
        clip_from_world: Mat4::IDENTITY,
        pcf_radius: 0,
        texel_uv: 0.0,
        normal_bias: 0.0,
    };

    #[test]
    fn the_sky_reflection_strength_travels_in_the_ambient_alpha() {
        let pack = |reflection: Option<f32>| {
            let ambient =
                reflection.map_or_else(VeAmbientLight::default, |reflection| VeAmbientLight {
                    reflection,
                    ..VeAmbientLight::default()
                });
            let lights = ExtractedLights {
                ambient: Some(ambient),
                ..ExtractedLights::default()
            };
            pack_lights(&lights, Vec3::ZERO, MAX_POINT_LIGHTS, &NO_SHADOW)
        };
        let off = pack(None);
        assert!(
            off.ambient.w.abs() < f32::EPSILON,
            "off by default, so old scenes keep their look"
        );
        let on = pack(Some(1.5));
        assert!((on.ambient.w - 1.5).abs() < f32::EPSILON);
        let negative = pack(Some(-3.0));
        assert!(
            negative.ambient.w.abs() < f32::EPSILON,
            "negative strength is clamped to off"
        );
    }

    #[test]
    fn no_lights_pack_to_zero() {
        let uniform = pack_lights(
            &ExtractedLights::default(),
            Vec3::ZERO,
            MAX_POINT_LIGHTS,
            &NO_SHADOW,
        );
        assert_eq!(uniform, LightsUniform::default());
    }

    #[test]
    fn sun_direction_points_toward_the_light() {
        let lights = ExtractedLights {
            sun: Some((Vec3::NEG_Y, VeDirectionalLight::default())),
            ..Default::default()
        };
        let uniform = pack_lights(&lights, Vec3::ZERO, MAX_POINT_LIGHTS, &NO_SHADOW);
        assert!((uniform.sun_direction.y - 1.0).abs() < 1e-6);
        assert!(uniform.sun_color.x > 0.0);
        assert!(
            uniform.shadow_params.x.abs() < f32::EPSILON,
            "no shadows were enabled"
        );
    }

    #[test]
    fn enabled_shadows_are_packed_only_when_there_is_a_sun() {
        let shadow = ShadowParams {
            enabled: true,
            clip_from_world: Mat4::from_scale(Vec3::splat(2.0)),
            pcf_radius: 1,
            texel_uv: 0.25,
            normal_bias: 0.05,
        };
        let with_sun = ExtractedLights {
            sun: Some((Vec3::NEG_Y, VeDirectionalLight::default())),
            ..Default::default()
        };
        let packed = pack_lights(&with_sun, Vec3::ZERO, MAX_POINT_LIGHTS, &shadow);
        assert_eq!(packed.shadow_params, Vec4::new(1.0, 1.0, 0.25, 0.05));
        assert_eq!(packed.sun_shadow_from_world, shadow.clip_from_world);
        let without = pack_lights(
            &ExtractedLights::default(),
            Vec3::ZERO,
            MAX_POINT_LIGHTS,
            &shadow,
        );
        assert_eq!(without.shadow_params, Vec4::ZERO);
    }

    #[test]
    fn keeps_the_closest_lights_when_over_the_cap() {
        let lights = ExtractedLights {
            points: (0..12_u8).rev().map(|i| point(f32::from(i))).collect(),
            ..Default::default()
        };
        let uniform = pack_lights(&lights, Vec3::ZERO, 3, &NO_SHADOW);
        assert!((uniform.counts.x - 3.0).abs() < f32::EPSILON);
        let xs: Vec<f32> = uniform.points[..3]
            .iter()
            .map(|p| p.position_range.x)
            .collect();
        assert_eq!(xs, [0.0, 1.0, 2.0]);
        assert!(
            uniform.points[3..]
                .iter()
                .all(|p| *p == PointLightUniform::default())
        );
    }

    #[test]
    fn tier_cap_cannot_exceed_the_block_size() {
        let lights = ExtractedLights {
            points: (0..20_u8).map(|i| point(f32::from(i))).collect(),
            ..Default::default()
        };
        let uniform = pack_lights(&lights, Vec3::ZERO, 1000, &NO_SHADOW);
        assert!((uniform.counts.x - 8.0).abs() < f32::EPSILON);
    }
}
