//! Scenes and checks shared by the probe tests.
use ve_gi::{GridSpec, Material, Scene, Settings, Sky, Sun};

/// An empty scene under a sky of one colour.
pub(crate) fn scene(sky: [f32; 3]) -> Scene {
    Scene {
        positions: vec![],
        indices: vec![],
        triangle_material: vec![],
        materials: vec![],
        sun: None,
        sky: Sky {
            zenith: sky,
            horizon: sky,
            ground: sky,
        },
    }
}

pub(crate) fn settings(rays: u32, bounces: u32) -> Settings {
    Settings {
        rays_per_probe: rays,
        bounces,
    }
}

/// A 400 m square plane at y = 0 (material 0), under a sky of one colour, optionally lit by a sun.
pub(crate) fn plane(
    albedo: [f32; 3],
    emissive: [f32; 3],
    sky: [f32; 3],
    sun: Option<Sun>,
) -> Scene {
    let mut s = scene(sky);
    s.positions = vec![
        [-200.0, 0.0, -200.0],
        [200.0, 0.0, -200.0],
        [200.0, 0.0, 200.0],
        [-200.0, 0.0, 200.0],
    ];
    s.indices = vec![0, 2, 1, 0, 3, 2];
    s.triangle_material = vec![0, 0];
    s.materials = vec![Material { albedo, emissive }];
    s.sun = sun;
    s
}

/// The sun straight overhead with irradiance 3.
pub(crate) fn overhead_sun() -> Sun {
    Sun {
        direction: [0.0, 1.0, 0.0],
        irradiance: [3.0; 3],
    }
}

/// One probe at (0, 1, 0).
pub(crate) fn one() -> GridSpec {
    GridSpec {
        origin: [0.0, 1.0, 0.0],
        spacing: 1.0,
        dims: [1, 1, 1],
    }
}

/// Asserts `a` equals `b` within the fraction `fraction` of `b` (or of 0.0001 when `b` is about zero).
pub(crate) fn close(a: f32, b: f32, fraction: f32) {
    assert!(
        (a - b).abs() <= fraction * b.abs().max(0.0001),
        "{a} != {b} within {fraction}"
    );
}
