//! Probes among solid geometry: shadows, probes buried in solids, and bounces between surfaces.
#![expect(
    clippy::unwrap_used,
    reason = "the fixtures are known to be valid, so a failed bake is a test failure"
)]
mod common;

use common::{close, one, overhead_sun, plane, scene, settings};
use std::f32::consts::PI;
use ve_gi::{GridSpec, Material, Scene, bake};

/// Adds the twelve triangles of the box `min..max` to the scene, with `material`. Wound outward, or inward for a room.
fn add_box(s: &mut Scene, min: [f32; 3], max: [f32; 3], material: u32, inward: bool) {
    let base = u32::try_from(s.positions.len()).unwrap();
    // The corners in the order the faces below use: the four of the near (z low) face, then the four of the far one.
    for corner in [
        [0, 0, 0],
        [1, 0, 0],
        [1, 1, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 0, 1],
        [1, 1, 1],
        [0, 1, 1],
    ] {
        s.positions.push(std::array::from_fn(|a| {
            if corner[a] == 0 { min[a] } else { max[a] }
        }));
    }
    let outward = [
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7, 3,
        1, 2, 6, 1, 6, 5,
    ];
    for triangle in outward.chunks(3) {
        let (a, b, c) = (triangle[0], triangle[1], triangle[2]);
        // Swapping two corners of a triangle reverses which way it faces.
        let order = if inward { [a, c, b] } else { [a, b, c] };
        s.indices.extend(order.map(|i| base + i));
        s.triangle_material.push(material);
    }
}

const BLACK: Material = Material {
    albedo: [0.0; 3],
    emissive: [0.0; 3],
};

#[test]
fn a_cube_over_a_lit_floor_dims_the_light_coming_up_from_below() {
    // A sunlit white floor with a black cube 2 m above it. Under the cube the floor gets no sun, so it radiates less and the
    // lower hemisphere is dimmer. The `y` term is negative when the light comes from below, so a dimmer lower hemisphere moves
    // it TOWARD zero: about -0.18 under the cube against -0.38 (= -0.8 * 3 / PI / 2) away from it.
    let mut s = plane([0.8; 3], [0.0; 3], [0.0; 3], Some(overhead_sun()));
    s.materials.push(BLACK);
    add_box(&mut s, [-1.0, 2.0, -1.0], [1.0, 4.0, 1.0], 1, false);
    let under = bake(s.clone(), one(), settings(512, 2)).unwrap();
    let away_spec = GridSpec {
        origin: [10.0, 1.0, 0.0],
        ..one()
    };
    let away = bake(s, away_spec, settings(512, 2)).unwrap();
    let (a, b) = (under.probes[0].y[0], away.probes[0].y[0]);
    assert!(a > b + 0.05, "under the cube {a} against {b} away from it");
    assert!(a < 0.0, "light still comes from below");
    close(b, -0.8 * 3.0 / PI / 2.0, 0.03);
}

#[test]
fn a_probe_inside_a_solid_is_invalid_and_is_filled_from_open_air() {
    let mut s = scene([1.0; 3]);
    s.materials = vec![Material {
        albedo: [0.5; 3],
        emissive: [0.0; 3],
    }];
    add_box(&mut s, [-1.0; 3], [1.0; 3], 0, false);
    let spec = GridSpec {
        origin: [-1.5; 3],
        spacing: 1.5,
        dims: [3; 3],
    };
    let g = bake(s, spec, settings(128, 1)).unwrap();
    assert!(!g.valid[13], "the centre probe is inside the cube");
    assert!(g.valid[0], "a corner probe is in the open");
    let centre = g.probes[13].dc;
    assert!(
        centre
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.5).contains(v))
    );
}

#[test]
fn each_bounce_in_a_closed_room_adds_one_more_reflection() {
    // Walls that emit 1 and reflect half: the light inside is 1 after one pass, 1 + 0.5 after two, 1 + 0.5 + 0.25 + 0.125 after
    // four (the series 1 / (1 - 0.5) = 2 in the limit). The probes hold irradiance over pi, so they read this radiance directly.
    let grid = GridSpec {
        origin: [-1.0; 3],
        spacing: 1.0,
        dims: [3; 3],
    };
    for (bounces, expected) in [(1, 1.0_f32), (2, 1.5), (4, 1.875)] {
        let mut room = scene([0.0; 3]);
        room.materials = vec![Material {
            albedo: [0.5; 3],
            emissive: [1.0; 3],
        }];
        add_box(&mut room, [-2.0; 3], [2.0; 3], 0, true);
        let g = bake(room, grid, settings(256, bounces)).unwrap();
        for p in &g.probes {
            close(p.dc[0], expected, 0.03);
            let tilt = p.x[0].abs().max(p.y[0].abs()).max(p.z[0].abs());
            assert!(
                tilt < 0.04 * expected,
                "a room lit evenly from all sides has no direction: {p:?}"
            );
        }
    }
}

#[test]
fn a_lit_floor_does_not_invent_light_in_later_passes() {
    // A sunlit white floor under a black sky: the probe above it faces nothing else that could send light back, so the floor
    // adds no bounce light to itself and the answer is the same after one pass or five (no runaway, no leak).
    let floor = || plane([0.8; 3], [0.0; 3], [0.0; 3], Some(overhead_sun()));
    let once = bake(floor(), one(), settings(512, 1)).unwrap();
    let five = bake(floor(), one(), settings(512, 5)).unwrap();
    for channel in 0..3 {
        close(once.probes[0].dc[channel], 0.8 * 3.0 / PI / 2.0, 0.03);
        close(five.probes[0].dc[channel], once.probes[0].dc[channel], 0.01);
        close(five.probes[0].y[channel], once.probes[0].y[channel], 0.01);
    }
}

#[test]
fn a_deep_probe_takes_light_from_the_nearest_open_air() {
    // A 3 m black cube on a grid of 1 m: the middle probe is 1.5 m from every wall, buried, and two cells from the nearest probe
    // in open air. Rays inside the cube see only black walls (radiance 0), so a value well above zero after the fix-up can only
    // have come from the open air around it (a probe next to the cube sees most of the sky, which is 1).
    let mut s = scene([1.0; 3]);
    s.materials = vec![BLACK];
    add_box(&mut s, [-1.5; 3], [1.5; 3], 0, false);
    let spec = GridSpec {
        origin: [-3.0; 3],
        spacing: 1.0,
        dims: [7; 3],
    };
    let g = bake(s, spec, settings(128, 1)).unwrap();
    let centre = 3 + 7 * (3 + 7 * 3);
    assert!(!g.valid[centre], "the middle probe is inside the cube");
    assert!(g.valid[0], "a corner probe is in the open");
    let light = g.probes[centre].dc[0];
    assert!(light > 0.2 && light <= 1.01, "{light}");
}
