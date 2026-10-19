//! Probes in open air, where the right answer can be worked out by hand: furnace, gradient, a plane below, bounce, emission,
//! determinism, and the grid helpers.
mod common;

use common::{close, one, overhead_sun, plane, scene, settings};
use ve_gi::{Baker, GridSpec, ProbeGrid, ProbeSh, Sky, bake};

#[test]
fn white_furnace() {
    let g = bake(
        scene([0.7, 0.4, 0.2]),
        GridSpec {
            origin: [0.0; 3],
            spacing: 1.0,
            dims: [2, 2, 2],
        },
        settings(256, 2),
    )
    .unwrap();
    for p in &g.probes {
        for c in 0..3 {
            close(p.dc[c], [0.7, 0.4, 0.2][c], 0.005);
            assert!(p.x[c].abs() < 0.0035 && p.y[c].abs() < 0.0035 && p.z[c].abs() < 0.0035);
        }
    }
    for (i, c) in g.sample([4.0; 3], [0.0, 1.0, 0.0]).iter().enumerate() {
        close(*c, [0.7, 0.4, 0.2][i], 0.005);
    }
}

#[test]
fn linear_sky_gradient() {
    // The sky is exactly 0.5 + 0.5 * y, so irradiance over pi for a normal n is 0.5 + (2/3) * 0.5 * n.y.
    let mut s = scene([0.0; 3]);
    s.sky = Sky {
        zenith: [1.0; 3],
        horizon: [0.5; 3],
        ground: [0.0; 3],
    };
    let g = bake(s, one(), settings(512, 1)).unwrap();
    close(g.probes[0].dc[0], 0.5, 0.01);
    close(g.probes[0].y[0], 1.0 / 3.0, 0.01);
    assert!(g.probes[0].x[0].abs() < 0.01 && g.probes[0].z[0].abs() < 0.01);
}

#[test]
fn a_black_ground_hides_the_lower_half_of_the_sky() {
    let g = bake(
        plane([0.0; 3], [0.0; 3], [1.0; 3], None),
        one(),
        settings(512, 1),
    )
    .unwrap();
    close(g.probes[0].dc[0], 0.5, 0.02);
    close(g.probes[0].y[0], 0.5, 0.02);
    close(g.sample([0.0, 1.0, 0.0], [0.0, 1.0, 0.0])[0], 1.0, 0.03);
    assert!(g.sample([0.0, 1.0, 0.0], [0.0, -1.0, 0.0])[0] < 0.03);
}

#[test]
fn a_sunlit_white_plane_sends_light_back_up() {
    let g = bake(
        plane([0.8; 3], [0.0; 3], [0.0; 3], Some(overhead_sun())),
        one(),
        settings(512, 2),
    )
    .unwrap();
    // The plane radiates 0.8 / PI * 3 = 0.7639 over the whole lower hemisphere.
    close(g.probes[0].dc[0], 0.382, 0.03);
    close(g.probes[0].y[0], -0.382, 0.03);
    close(g.sample([0.0, 1.0, 0.0], [0.0, -1.0, 0.0])[0], 0.7639, 0.04);
    assert!(g.sample([0.0, 1.0, 0.0], [0.0, 1.0, 0.0])[0] < 0.03);
}

#[test]
fn an_emissive_plane_lights_what_faces_it() {
    let g = bake(
        plane([0.0; 3], [2.0, 1.0, 0.5], [0.0; 3], None),
        one(),
        settings(512, 1),
    )
    .unwrap();
    for c in 0..3 {
        close(g.probes[0].dc[c], [1.0, 0.5, 0.25][c], 0.03);
        close(g.probes[0].y[c], -[1.0, 0.5, 0.25][c], 0.03);
    }
}

#[test]
fn the_same_input_gives_the_same_bits_however_the_work_is_sliced() {
    let s = plane([0.8; 3], [0.0; 3], [0.0; 3], Some(overhead_sun()));
    let spec = one();
    let a = bake(s.clone(), spec, settings(64, 2)).unwrap();
    let b = bake(s.clone(), spec, settings(64, 2)).unwrap();
    assert_eq!(a.probes, b.probes);
    let mut baker = Baker::new(s, spec, settings(64, 2)).unwrap();
    while !baker.step(7) {}
    assert_eq!(a.probes, baker.finish().unwrap().probes);
}

#[test]
#[expect(
    clippy::cast_precision_loss,
    reason = "a probe count along an axis is at most 64"
)]
fn a_fitted_grid_covers_the_box_and_respects_its_limits() {
    let g = GridSpec::fit([-5.0, 0.0, -5.0], [5.0, 4.0, 5.0], 1.0, 64).unwrap();
    close(g.spacing, 1.0, 1e-6);
    assert!(g.origin[0] <= -6.0 && g.origin[1] <= -1.0 && g.origin[2] <= -6.0);
    let far = std::array::from_fn::<_, 3, _>(|i| g.origin[i] + g.spacing * (g.dims[i] - 1) as f32);
    assert!(far[0] >= 6.0 && far[1] >= 5.0 && far[2] >= 6.0);
    let large = GridSpec::fit([0.0; 3], [1000.0; 3], 0.5, 16).unwrap();
    assert!(large.dims.iter().all(|&v| v <= 16) && large.spacing > 0.5);
    assert!(GridSpec::fit([-1.0; 3], [1.0; 3], -1.0, 16).is_err());
    assert!(GridSpec::fit([f32::NAN; 3], [1.0; 3], 1.0, 16).is_err());
}

#[test]
fn a_lookup_interpolates_between_probes_and_clamps_outside_the_grid() {
    let spec = GridSpec {
        origin: [0.0; 3],
        spacing: 1.0,
        dims: [2, 1, 1],
    };
    let red = |dc| ProbeSh {
        dc: [dc, 0.0, 0.0],
        ..ProbeSh::default()
    };
    let g = ProbeGrid::from_parts(spec, vec![red(1.0), red(3.0)], vec![true; 2], [0.0; 3]);
    close(g.sample([0.5, 0.0, 0.0], [0.0, 1.0, 0.0])[0], 2.0, 1e-5);
    close(g.sample([0.0; 3], [0.0; 3])[0], 1.0, 1e-5);
    close(g.sample([-100.0; 3], [0.0; 3])[0], 1.0, 1e-6);
}

#[test]
fn a_lookup_ignores_invalid_probes_and_falls_back_when_none_is_usable() {
    let spec = GridSpec {
        origin: [0.0; 3],
        spacing: 1.0,
        dims: [2, 1, 1],
    };
    let red = |dc| ProbeSh {
        dc: [dc, 0.0, 0.0],
        ..ProbeSh::default()
    };
    let one_bad = ProbeGrid::from_parts(
        spec,
        vec![red(1.0), red(100.0)],
        vec![true, false],
        [0.0; 3],
    );
    close(
        one_bad.sample([0.5, 0.0, 0.0], [0.0, 1.0, 0.0])[0],
        1.0,
        1e-5,
    );
    let none = ProbeGrid::from_parts(
        spec,
        vec![red(1.0), red(3.0)],
        vec![false; 2],
        [0.25, 0.5, 0.75],
    );
    let fallback = none.sample([0.5, 0.0, 0.0], [0.0, 1.0, 0.0]);
    for (got, expected) in fallback.into_iter().zip([0.25, 0.5, 0.75]) {
        close(got, expected, 1e-6);
    }
}

#[test]
fn a_baked_grid_agrees_with_pi_normalised_irradiance() {
    // Sanity on the convention: under a sky of 2 every probe reads 2 for every normal, so a surface of albedo 0.5 reflects 1.
    let g = bake(scene([2.0; 3]), one(), settings(256, 1)).unwrap();
    for normal in [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0]] {
        close(g.sample([0.0, 1.0, 0.0], normal)[0], 2.0, 0.01);
    }
}

#[test]
#[ignore = "a timing note: run it with --release --ignored --nocapture"]
fn release_performance_note() {
    let start = std::time::Instant::now();
    let _ = bake(
        plane([0.8; 3], [0.0; 3], [0.0; 3], None),
        GridSpec {
            origin: [0.0; 3],
            spacing: 1.0,
            dims: [16, 8, 16],
        },
        settings(128, 2),
    );
    eprintln!("elapsed: {:?}", start.elapsed());
}
