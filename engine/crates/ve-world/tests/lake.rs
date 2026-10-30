//! `ve_world::river::fill_basin`: a lake that fills the hollow it sits in (the fix for water that was a square slab sticking out of the ground). Starting from a seed
//! point, every connected cell of ground lower than the water level becomes water; the result is the same `RiverOut` rivers produce, so the river water mesher draws it with
//! depth tint, wet banks and foam, and the shoreline follows the ground instead of a square.
#![expect(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "small grids; flags are exact"
)]
use ve_world::river::{GroundRef, LakeProfile, RiverError, fill_basin};

const N: usize = 64;

/// A round bowl: height = k * r^2 - 1, lowest (-1) at the centre (32, 32), cells 1 m apart, origin at 0.
fn bowl(k: f32) -> Vec<f32> {
    let mut h = vec![0.0; N * N];
    for z in 0..N {
        for x in 0..N {
            let r2 = (x as f32 - 32.0).powi(2) + (z as f32 - 32.0).powi(2);
            h[z * N + x] = k * r2 - 1.0;
        }
    }
    h
}

fn ground(h: &[f32]) -> GroundRef<'_> {
    GroundRef {
        heights: h,
        width: N,
        depth: N,
        cell: 1.0,
        origin: [0.0, 0.0],
    }
}

fn profile(level: f32) -> LakeProfile {
    LakeProfile {
        level,
        shore_width: 3.0,
    }
}

fn never() -> bool {
    false
}

#[test]
fn the_water_fills_exactly_the_cells_below_the_level_around_the_seed() {
    let h = bowl(0.01); // below level 0 where 0.01 r^2 < 1, that is r < 10
    let out = fill_basin(&ground(&h), [32.0, 32.0], &profile(0.0), &never).unwrap();
    let wet: Vec<usize> = (0..N * N).filter(|&i| out.channel_mask[i] > 0.5).collect();
    let expected = std::f32::consts::PI * 100.0;
    assert!(
        (wet.len() as f32 - expected).abs() / expected < 0.08,
        "{} wet cells, a disc of radius 10 has {expected}",
        wet.len()
    );
    for &i in &wet {
        assert_eq!(out.water_height[i], 0.0, "water sits at the level");
        assert!(h[i] < 0.0 + 1e-6, "water only over ground below the level");
    }
}

#[test]
fn dry_cells_keep_their_ground_height_in_the_water_height_grid() {
    let h = bowl(0.01);
    let out = fill_basin(&ground(&h), [32.0, 32.0], &profile(0.0), &never).unwrap();
    for i in 0..N * N {
        if out.channel_mask[i] <= 0.5 {
            assert_eq!(out.water_height[i], h[i], "cell {i}");
        }
    }
}

#[test]
fn only_the_basin_the_seed_is_in_fills_not_another_pit_at_the_same_level() {
    let mut h = vec![5.0; N * N];
    for z in 0..N {
        for x in 0..N {
            let a = (x as f32 - 16.0).powi(2) + (z as f32 - 32.0).powi(2);
            let b = (x as f32 - 48.0).powi(2) + (z as f32 - 32.0).powi(2);
            if a < 49.0 || b < 49.0 {
                h[z * N + x] = -1.0;
            }
        }
    }
    let out = fill_basin(&ground(&h), [16.0, 32.0], &profile(0.0), &never).unwrap();
    for z in 0..N {
        for x in 0..N {
            let i = z * N + x;
            if out.channel_mask[i] > 0.5 {
                assert!(x < 32, "water leaked into the other pit at x = {x}");
            }
        }
    }
    let left = (0..N * N).filter(|&i| out.channel_mask[i] > 0.5).count();
    assert!(left > 100, "the seeded pit is filled: {left} cells");
}

#[test]
fn a_seed_on_dry_ground_is_refused_and_says_so() {
    let h = bowl(0.01);
    match fill_basin(&ground(&h), [2.0, 2.0], &profile(0.0), &never) {
        Err(RiverError::BadParameter { name, .. }) => {
            assert!(name.contains("seed") || name.contains("at"), "{name}")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_seed_outside_the_grid_is_refused() {
    let h = bowl(0.01);
    assert!(fill_basin(&ground(&h), [-5.0, 32.0], &profile(0.0), &never).is_err());
    assert!(fill_basin(&ground(&h), [f32::NAN, 32.0], &profile(0.0), &never).is_err());
}

#[test]
fn a_lake_has_no_current() {
    let h = bowl(0.01);
    let out = fill_basin(&ground(&h), [32.0, 32.0], &profile(0.0), &never).unwrap();
    assert!(out.flow_speed.iter().all(|&s| s == 0.0));
    assert!(out.flow_dir.iter().all(|d| d[0] == 0.0 && d[1] == 0.0));
}

#[test]
fn the_wet_bank_is_strongest_at_the_shore_and_fades_to_nothing_over_the_shore_width() {
    let h = bowl(0.01);
    let out = fill_basin(&ground(&h), [32.0, 32.0], &profile(0.0), &never).unwrap();
    let at = |r: f32| {
        let i = 32 * N + (32.0 + r).round() as usize;
        out.bank_mask[i]
    };
    // The shore is at r = 10; the cell just outside is wet, three metres out it is dry again, and it falls steadily between.
    assert!(at(10.5) > 0.7, "{}", at(10.5));
    assert!(
        at(11.0) >= at(12.0) && at(12.0) >= at(13.0),
        "{} {} {}",
        at(11.0),
        at(12.0),
        at(13.0)
    );
    assert!(at(14.5) < 1e-3, "{}", at(14.5));
    assert!(at(20.0) == 0.0);
    for i in 0..N * N {
        assert!(
            (0.0..=1.0).contains(&out.bank_mask[i]) && (0.0..=1.0).contains(&out.channel_mask[i])
        );
    }
}

#[test]
fn a_level_above_the_rim_floods_to_the_edge_of_the_grid_without_a_panic() {
    // Even the corners (r^2 = 2 * 32^2) stay below level 0: 0.0002 * 2048 - 1 = -0.59, so the water covers everything.
    let h = bowl(0.0002);
    let out = fill_basin(&ground(&h), [32.0, 32.0], &profile(0.0), &never).unwrap();
    assert!(
        out.channel_mask[0] > 0.5 && out.channel_mask[N * N - 1] > 0.5,
        "water reaches the corners"
    );
}

#[test]
fn a_level_below_the_lowest_ground_has_no_water_and_says_so() {
    let h = bowl(0.01);
    assert!(matches!(
        fill_basin(&ground(&h), [32.0, 32.0], &profile(-5.0), &never),
        Err(RiverError::BadParameter { .. })
    ));
}

#[test]
fn bad_parameters_are_refused() {
    let h = bowl(0.01);
    for p in [
        LakeProfile {
            level: f32::NAN,
            shore_width: 3.0,
        },
        LakeProfile {
            level: 0.0,
            shore_width: -1.0,
        },
        LakeProfile {
            level: 0.0,
            shore_width: f32::INFINITY,
        },
    ] {
        assert!(
            fill_basin(&ground(&h), [32.0, 32.0], &p, &never).is_err(),
            "{p:?}"
        );
    }
}

#[test]
fn the_same_input_gives_the_same_water_and_a_cancel_stops_it() {
    let h = bowl(0.01);
    let a = fill_basin(&ground(&h), [32.0, 32.0], &profile(0.0), &never).unwrap();
    let b = fill_basin(&ground(&h), [32.0, 32.0], &profile(0.0), &never).unwrap();
    assert_eq!(a, b);
    assert!(matches!(
        fill_basin(&ground(&h), [32.0, 32.0], &profile(0.0), &|| true),
        Err(RiverError::Cancelled)
    ));
}
