//! `carve_river_descending`: a river whose water surface is a real river's: it only ever runs downhill along the path, is level across
//! its width, sits a fixed depth above its bed, and never floats above the ground beside it. The older `carve_river` takes each cell's
//! water height from the ground beside that cell, which on stepped ground (mesas, terraces, ridges) makes the water climb the steps.
//! Written before the body; do not weaken.
#![expect(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "fixtures build grids from small indices and compare exact copies of the same computation"
)]
use ve_world::river::{Ground, Path, RiverError, RiverOut, RiverProfile, carve_river_descending};

const NEVER: &dyn Fn() -> bool = &|| false;
const W: usize = 129;
const D: usize = 129;
const Z: usize = 64;

/// Ground at 50 m falling 0.04 m per metre toward +x, with a gentle ripple across z, a 12 m ridge across the river between x = 45 and 75
/// (the water must not climb it) and a 20 m step down at x = 100 (the water must not fall off it in one drop).
fn stepped() -> Vec<f32> {
    let mut out = Vec::with_capacity(W * D);
    for j in 0..D {
        for i in 0..W {
            let (x, z) = (i as f32, j as f32);
            let ridge = if (45.0..75.0).contains(&x) { 12.0 } else { 0.0 };
            let step = if x >= 100.0 { -20.0 } else { 0.0 };
            out.push(50.0 - 0.04 * x + ridge + step + 0.3 * (z * 0.2).sin());
        }
    }
    out
}

fn ground(heights: &mut [f32]) -> Ground<'_> {
    Ground {
        heights,
        width: W,
        depth: D,
        cell: 1.0,
        origin: [0.0, 0.0],
    }
}

fn profile() -> RiverProfile {
    RiverProfile {
        width: 8.0,
        depth: 4.0,
        bank_slope_deg: 40.0,
        bank_width: 8.0,
        meander_flatten: 0.3,
        water_level_offset: 0.5,
    }
}

fn east() -> Path {
    Path {
        points: vec![[10.0, 64.0], [60.0, 64.0], [118.0, 64.0]],
    }
}

fn west() -> Path {
    Path {
        points: vec![[118.0, 64.0], [60.0, 64.0], [10.0, 64.0]],
    }
}

fn at(x: usize, z: usize) -> usize {
    z * W + x
}

fn carve(path: &Path) -> (Vec<f32>, Vec<f32>, RiverOut) {
    let original = stepped();
    let mut heights = original.clone();
    let out = carve_river_descending(&mut ground(&mut heights), path, &profile(), NEVER).unwrap();
    (original, heights, out)
}

#[test]
fn the_water_only_runs_downhill_even_over_a_ridge_and_a_step() {
    let (_, _, out) = carve(&east());
    let level: Vec<f32> = (12..=116).map(|x| out.water_height[at(x, Z)]).collect();
    assert!(
        level.windows(2).all(|p| p[1] <= p[0] + 1e-3),
        "the water surface rose somewhere along an eastward river"
    );
    // Going the other way down the path is still downhill in path order.
    let (_, _, back) = carve(&west());
    let level: Vec<f32> = (12..=116)
        .rev()
        .map(|x| back.water_height[at(x, Z)])
        .collect();
    assert!(
        level.windows(2).all(|p| p[1] <= p[0] + 1e-3),
        "the water surface rose along a westward river"
    );
}

#[test]
fn the_water_surface_is_level_across_the_channel() {
    let (_, _, out) = carve(&east());
    for x in 15..=110 {
        let centre = out.water_height[at(x, Z)];
        for dz in 0..=3 {
            for z in [Z - dz, Z + dz] {
                let here = out.water_height[at(x, z)];
                assert!(
                    (here - centre).abs() < 1e-3,
                    "x {x} z {z}: {here} against {centre} at the centre line"
                );
            }
        }
    }
}

#[test]
fn the_bed_lies_the_water_depth_below_the_surface() {
    // Water depth is `depth - water_level_offset` = 3.5 m.
    let (_, heights, out) = carve(&east());
    for x in 20..=100 {
        let depth = out.water_height[at(x, Z)] - heights[at(x, Z)];
        assert!(
            (depth - 3.5).abs() < 0.1,
            "x {x}: the water is {depth} m deep"
        );
    }
}

#[test]
fn the_water_never_floats_above_the_ground_beside_it() {
    let (original, _, out) = carve(&east());
    for x in 12..=116 {
        // The surface is at least the offset below the ground the river replaced, on the centre line.
        assert!(
            out.water_height[at(x, Z)] <= original[at(x, Z)] - 0.5 + 1e-3,
            "x {x}: water above the old ground"
        );
    }
}

#[test]
fn the_surface_never_falls_faster_than_five_percent() {
    let (_, _, out) = carve(&east());
    for x in 12..116 {
        let drop = out.water_height[at(x, Z)] - out.water_height[at(x + 1, Z)];
        assert!(
            drop <= 0.05 + 1e-3,
            "x {x}: the water falls {drop} m in one metre"
        );
    }
}

#[test]
fn ground_outside_the_banks_is_untouched_and_nothing_is_raised_beside_the_channel() {
    let (original, heights, out) = carve(&east());
    // Beyond half the width plus the banks (4 + 8 = 12 m) nothing changed.
    for (x, z) in [
        (60, Z + 14),
        (60, Z - 14),
        (30, Z + 20),
        (90, Z - 20),
        (5, 5),
    ] {
        assert_eq!(heights[at(x, z)], original[at(x, z)], "({x}, {z}) changed");
    }
    // Outside the channel floor the ground is only ever cut, never built up.
    for x in 12..=116 {
        for z in 0..D {
            if out.channel_mask[at(x, z)] < 0.01 {
                assert!(
                    heights[at(x, z)] <= original[at(x, z)] + 1e-4,
                    "({x}, {z}) was raised"
                );
            }
        }
    }
}

#[test]
fn flow_runs_along_the_path_faster_where_the_water_is_steeper() {
    let (_, _, out) = carve(&east());
    let channel = |x: usize| at(x, Z);
    for x in 20..=100 {
        assert!(
            out.flow_speed[channel(x)] > 0.0,
            "x {x}: still water in a river"
        );
        assert!(
            out.flow_dir[channel(x)][0] > 0.9,
            "x {x}: the flow does not run east"
        );
    }
    // Nothing flows outside the channel.
    assert_eq!(out.flow_speed[at(60, Z + 20)], 0.0);
    // The long reach after the ridge has a gentler surface than the reach that has to catch up with the step: faster water there.
    let gentle = out.flow_speed[channel(30)];
    let steep = (80..=99)
        .map(|x| out.flow_speed[channel(x)])
        .fold(0.0_f32, f32::max);
    let falls = (80..=99)
        .map(|x| out.water_height[channel(x)] - out.water_height[channel(x + 1)])
        .fold(0.0_f32, f32::max);
    let flat = out.water_height[channel(30)] - out.water_height[channel(31)];
    if falls > flat + 0.005 {
        assert!(
            steep > gentle,
            "the steeper reach ({steep}) should run faster than the gentle one ({gentle})"
        );
    }
}

#[test]
fn it_is_deterministic_and_leaves_no_water_off_the_channel() {
    let (_, heights_a, a) = carve(&east());
    let (_, heights_b, b) = carve(&east());
    assert_eq!(heights_a, heights_b);
    assert_eq!(a, b);
    // Away from the channel the water height is the ground height (the field's own convention).
    assert_eq!(a.water_height[at(60, Z + 20)], heights_a[at(60, Z + 20)]);
    assert!(a.channel_mask.iter().all(|m| (0.0..=1.0).contains(m)));
    assert!(a.bank_mask.iter().all(|m| (0.0..=1.0).contains(m)));
}

#[test]
fn it_refuses_what_carve_river_refuses_and_can_be_cancelled() {
    let mut heights = stepped();
    let one_point = Path {
        points: vec![[10.0, 64.0]],
    };
    assert!(matches!(
        carve_river_descending(&mut ground(&mut heights), &one_point, &profile(), NEVER),
        Err(RiverError::BadPath { .. })
    ));
    let outside = Path {
        points: vec![[-50.0, -50.0], [-10.0, -10.0]],
    };
    assert!(matches!(
        carve_river_descending(&mut ground(&mut heights), &outside, &profile(), NEVER),
        Err(RiverError::BadPath { .. })
    ));
    let mut bad = profile();
    bad.width = 0.0;
    assert!(matches!(
        carve_river_descending(&mut ground(&mut heights), &east(), &bad, NEVER),
        Err(RiverError::BadParameter { .. })
    ));
    let mut bad = profile();
    bad.bank_slope_deg = 2.0;
    assert!(matches!(
        carve_river_descending(&mut ground(&mut heights), &east(), &bad, NEVER),
        Err(RiverError::BadParameter { .. })
    ));
    let mut heights = stepped();
    let cancelled =
        carve_river_descending(&mut ground(&mut heights), &east(), &profile(), &|| true);
    assert!(matches!(cancelled, Err(RiverError::Cancelled)));
}

#[test]
fn a_river_on_smooth_downhill_ground_follows_it_closely() {
    // On gently falling ground the new profile is the old behaviour's close cousin: the bed stays within a couple of metres of the old one.
    let smooth: Vec<f32> = (0..D)
        .flat_map(|_| (0..W).map(|i| 50.0 - 0.04 * i as f32))
        .collect();
    let mut a = smooth.clone();
    let mut b = smooth;
    ve_world::river::carve_river(&mut ground(&mut a), &east(), &profile(), NEVER).unwrap();
    carve_river_descending(&mut ground(&mut b), &east(), &profile(), NEVER).unwrap();
    for x in [30, 60, 90] {
        assert!(
            (a[at(x, Z)] - b[at(x, Z)]).abs() < 2.0,
            "x {x}: {} against {}",
            a[at(x, Z)],
            b[at(x, Z)]
        );
    }
}
