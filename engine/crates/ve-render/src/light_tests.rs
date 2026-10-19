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
    cascades: 0,
    clip_from_world: [Mat4::IDENTITY; MAX_CASCADES],
    splits: [0.0; MAX_CASCADES],
    normal_bias: [0.0; MAX_CASCADES],
    pcf_radius: 0,
    texel_uv: [0.0; 2],
    forward: Vec3::NEG_Z,
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
        pack_lights(
            &lights,
            Vec3::ZERO,
            MAX_POINT_LIGHTS,
            MAX_SPOT_LIGHTS,
            &NO_SHADOW,
        )
    };
    let off = pack(None);
    assert!(off.ambient.w.abs() < f32::EPSILON);
    let on = pack(Some(1.5));
    assert!((on.ambient.w - 1.5).abs() < f32::EPSILON);
    let negative = pack(Some(-3.0));
    assert!(negative.ambient.w.abs() < f32::EPSILON);
}

/// Mirrors the shader's L1 irradiance so the normal response is testable without a GPU.
fn irradiance(uniform: &LightsUniform, normal: Vec3) -> Vec3 {
    uniform.ambient_sh0.truncate() + uniform.ambient_sh1.truncate() * normal.y.clamp(-1.0, 1.0)
}

#[test]
fn sky_and_ground_colours_pack_as_an_l1_band() {
    let ambient = VeAmbientLight {
        intensity: 2.0,
        sky_color: Color::linear_rgba(0.5, 0.6, 0.7, 1.0),
        ground_color: Color::linear_rgba(0.2, 0.3, 0.4, 1.0),
        ..VeAmbientLight::default()
    };
    let packed = pack_lights(
        &ExtractedLights {
            ambient: Some(ambient),
            ..Default::default()
        },
        Vec3::ZERO,
        MAX_POINT_LIGHTS,
        MAX_SPOT_LIGHTS,
        &NO_SHADOW,
    );
    // Constant term is the intensity-scaled mean, the L1 term the half-difference.
    assert!((packed.ambient_sh0.x - 0.7).abs() < 1e-5);
    assert!((packed.ambient_sh1.x - 0.3).abs() < 1e-5);
    // Normal response: up-facing normals see the sky colour, down-facing the ground colour.
    let up = irradiance(&packed, Vec3::Y);
    let down = irradiance(&packed, Vec3::NEG_Y);
    assert!((up.x - 1.0).abs() < 1e-5);
    assert!((down.x - 0.4).abs() < 1e-5);
    assert!(up.x > down.x);
}

#[test]
fn ground_bounce_is_off_by_default_and_bounded() {
    let pack = |ambient: VeAmbientLight| {
        pack_lights(
            &ExtractedLights {
                ambient: Some(ambient),
                ..Default::default()
            },
            Vec3::ZERO,
            MAX_POINT_LIGHTS,
            MAX_SPOT_LIGHTS,
            &NO_SHADOW,
        )
    };
    let off = pack(VeAmbientLight::default());
    assert!(off.ambient_bounce.w.abs() < f32::EPSILON);
    let warm = pack(VeAmbientLight {
        intensity: 2.0,
        bounce_color: Color::linear_rgba(0.6, 0.4, 0.2, 1.0),
        bounce: 0.5,
        ..VeAmbientLight::default()
    });
    assert!((warm.ambient_bounce.w - 0.5).abs() < 1e-5);
    assert!((warm.ambient_bounce.x - 1.2).abs() < 1e-5);
    // A negative strength clamps to off rather than inverting the term.
    let clamped = pack(VeAmbientLight {
        bounce: -2.0,
        ..VeAmbientLight::default()
    });
    assert!(clamped.ambient_bounce.w.abs() < f32::EPSILON);
}

#[test]
fn no_lights_pack_to_zero() {
    let uniform = pack_lights(
        &ExtractedLights::default(),
        Vec3::ZERO,
        MAX_POINT_LIGHTS,
        MAX_SPOT_LIGHTS,
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
    let uniform = pack_lights(
        &lights,
        Vec3::ZERO,
        MAX_POINT_LIGHTS,
        MAX_SPOT_LIGHTS,
        &NO_SHADOW,
    );
    assert!((uniform.sun_direction.y - 1.0).abs() < 1e-6);
    assert!(uniform.sun_color.x > 0.0);
    assert!(uniform.shadow_params.x.abs() < f32::EPSILON);
}

#[test]
fn enabled_shadows_are_packed_only_when_there_is_a_sun() {
    let shadow = ShadowParams {
        enabled: true,
        cascades: 2,
        clip_from_world: [
            Mat4::from_scale(Vec3::splat(2.0)),
            Mat4::from_scale(Vec3::splat(4.0)),
            Mat4::IDENTITY,
        ],
        splits: [10.0, 50.0, 50.0],
        normal_bias: [0.05, 0.2, 0.2],
        pcf_radius: 1,
        texel_uv: [0.125, 0.25],
        forward: Vec3::X,
    };
    let with_sun = ExtractedLights {
        sun: Some((Vec3::NEG_Y, VeDirectionalLight::default())),
        ..Default::default()
    };
    let packed = pack_lights(
        &with_sun,
        Vec3::ZERO,
        MAX_POINT_LIGHTS,
        MAX_SPOT_LIGHTS,
        &shadow,
    );
    assert_eq!(packed.shadow_params, Vec4::new(2.0, 1.0, 0.125, 0.25));
    assert_eq!(packed.shadow_splits, Vec4::new(10.0, 50.0, 50.0, 0.0));
    assert_eq!(packed.shadow_bias, Vec4::new(0.05, 0.2, 0.2, 0.0));
    assert_eq!(packed.shadow_forward, Vec4::new(1.0, 0.0, 0.0, 0.0));
    assert_eq!(packed.sun_shadow_from_world, shadow.clip_from_world);
    let without = pack_lights(
        &ExtractedLights::default(),
        Vec3::ZERO,
        MAX_POINT_LIGHTS,
        MAX_SPOT_LIGHTS,
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
    let uniform = pack_lights(&lights, Vec3::ZERO, 3, MAX_SPOT_LIGHTS, &NO_SHADOW);
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
fn the_uniform_block_stays_within_the_webgl2_minimum() {
    // 368-byte shared block (ambient, L0/L1 sky/ground band, bounce, sun, three cascade matrices and the
    // shadow settings) + 16 x 32-byte point records + 8 x 48-byte spot records; documented in
    // docs/modeling/limits.md. WebGL2 guarantees at least 16 KiB per uniform block, so the fixed
    // arrays must fit it.
    assert_eq!(std::mem::size_of::<LightsUniform>(), 1_264);
}

#[test]
fn tier_cap_cannot_exceed_the_block_size() {
    let lights = ExtractedLights {
        points: (0..20_u8).map(|i| point(f32::from(i))).collect(),
        ..Default::default()
    };
    let uniform = pack_lights(&lights, Vec3::ZERO, 1000, MAX_SPOT_LIGHTS, &NO_SHADOW);
    assert!((uniform.counts.x - 16.0).abs() < f32::EPSILON);
}

#[test]
fn spot_lights_are_nearest_first_bounded_and_keep_soft_cone_angles() {
    let spots = (0..10_u8)
        .rev()
        .map(|i| ExtractedSpot {
            position: Vec3::new(f32::from(i), 0.0, 0.0),
            direction: Vec3::NEG_Y,
            color: LinearRgba::WHITE,
            intensity: 2.0,
            range: 5.0,
            inner_angle: 0.4,
            outer_angle: 0.8,
        })
        .collect();
    let lights = ExtractedLights {
        spots,
        ..Default::default()
    };
    let uniform = pack_lights(
        &lights,
        Vec3::ZERO,
        MAX_POINT_LIGHTS,
        MAX_SPOT_LIGHTS,
        &NO_SHADOW,
    );
    assert!((uniform.counts.y - 8.0).abs() < f32::EPSILON);
    assert!(uniform.spots[0].position_range.x.abs() < f32::EPSILON);
    assert!((uniform.spots[7].position_range.x - 7.0).abs() < f32::EPSILON);
    assert!(uniform.spots[0].color_inner.w > uniform.spots[0].direction_outer.w);
    assert!(uniform.spots[0].color_inner.x > 0.0);
}

#[test]
fn equal_distance_lights_pack_identically_regardless_of_query_order() {
    let dim = point(2.0);
    let bright = ExtractedPoint {
        intensity: 4.0,
        ..dim
    };
    let first = ExtractedLights {
        points: vec![bright, dim],
        ..Default::default()
    };
    let second = ExtractedLights {
        points: vec![dim, bright],
        ..Default::default()
    };
    let pack = |lights| {
        pack_lights(
            lights,
            Vec3::ZERO,
            MAX_POINT_LIGHTS,
            MAX_SPOT_LIGHTS,
            &NO_SHADOW,
        )
    };
    assert_eq!(pack(&first), pack(&second));
}
