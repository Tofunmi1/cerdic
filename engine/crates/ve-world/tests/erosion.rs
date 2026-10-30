//! Flow routing, stream power, thermal, hydraulic and two-scale tile erosion (ADR 0056). Written before the bodies; do not weaken.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "fixtures unwrap valid data and build terrain from small grid indices"
)]
use ve_world::erosion::{
    Constraint, ErosionError, Grid, Hydraulic, StreamPower, Thermal, coarse_world, halo_for,
    hydraulic, refine_tile, river_mask, route_flow, stream_power, thermal,
};

const NEVER: &dyn Fn() -> bool = &|| false;

fn hash(x: i64, z: i64) -> f32 {
    let mut h = (x.wrapping_mul(374_761_393) ^ z.wrapping_mul(668_265_263)) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

/// Smooth value noise on an 8-cell lattice plus a gentle fall toward +x, in metres (deterministic).
fn terrain(width: usize, depth: usize) -> Vec<f32> {
    let mut out = vec![0.0; width * depth];
    for j in 0..depth {
        for i in 0..width {
            let (fx, fz) = (i as f32 / 8.0, j as f32 / 8.0);
            let (x0, z0) = (fx.floor() as i64, fz.floor() as i64);
            let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
            let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
            let top = hash(x0, z0) * (1.0 - sx) + hash(x0 + 1, z0) * sx;
            let bottom = hash(x0, z0 + 1) * (1.0 - sx) + hash(x0 + 1, z0 + 1) * sx;
            out[j * width + i] = 300.0 - 0.25 * i as f32 + 60.0 * (top * (1.0 - sz) + bottom * sz);
        }
    }
    out
}

fn ramp(width: usize, depth: usize) -> Vec<f32> {
    (0..width * depth)
        .map(|k| 100.0 - (k % width) as f32)
        .collect()
}

fn grid(heights: &mut [f32], width: usize, depth: usize) -> Grid<'_> {
    Grid {
        heights,
        width,
        depth,
        cell: 2.0,
    }
}

// ---------- flow ----------

#[test]
fn flow_on_a_tilted_plane_runs_downhill_and_leaves_the_heights_alone() {
    let mut h = ramp(32, 32);
    let before = h.clone();
    let g = grid(&mut h, 32, 32);
    let flow = route_flow(&g, NEVER).unwrap();
    assert_eq!(h, before, "route_flow does not change the grid");
    for j in 0..32 {
        for i in 0..31 {
            assert_eq!(
                flow.receiver[j * 32 + i],
                (j * 32 + i + 1) as u32,
                "cell {i},{j} drains east"
            );
        }
        assert_eq!(
            flow.receiver[j * 32 + 31],
            u32::MAX,
            "the last column is the outlet"
        );
    }
    let outlet_area: f32 = (0..32).map(|j| flow.area[j * 32 + 31]).sum();
    assert!(
        (outlet_area - 32.0 * 32.0 * 4.0).abs() < 1.0,
        "all rain leaves: {outlet_area}"
    );
}

#[test]
fn area_grows_downstream() {
    let mut h = terrain(48, 48);
    let flow = route_flow(&grid(&mut h, 48, 48), NEVER).unwrap();
    for (cell, r) in flow.receiver.iter().enumerate() {
        if *r != u32::MAX {
            assert!(
                flow.area[*r as usize] >= flow.area[cell] + 4.0 - 1e-3,
                "cell {cell}"
            );
        }
    }
    assert!(flow.area.iter().all(|a| *a >= 4.0 - 1e-3));
}

#[test]
fn pits_are_filled_so_water_always_reaches_the_border() {
    let mut h: Vec<f32> = (0..32 * 32)
        .map(|k| {
            let (i, j) = ((k % 32) as f32 - 16.0, (k / 32) as f32 - 16.0);
            (i * i + j * j).sqrt()
        })
        .collect();
    let flow = route_flow(&grid(&mut h, 32, 32), NEVER).unwrap();
    for (cell, r) in flow.receiver.iter().enumerate() {
        let (i, j) = (cell % 32, cell / 32);
        if *r == u32::MAX {
            assert!(
                i == 0 || j == 0 || i == 31 || j == 31,
                "interior sink at {i},{j}"
            );
        }
    }
}

#[test]
fn the_river_mask_ramps_with_area() {
    let mut h = ramp(32, 32);
    let flow = route_flow(&grid(&mut h, 32, 32), NEVER).unwrap();
    // The first row drains 32 cells of 4 m2 = 128 m2 into cell 31; the ramp is 0 below min_area/2 and 1 above min_area*2.
    let mask = river_mask(&flow, 32, 32, 50.0);
    assert_eq!(mask.len(), 32 * 32);
    assert!(mask.iter().all(|v| (0.0..=1.0).contains(v)));
    assert!(
        mask[31] > 0.99,
        "128 m2 is above twice min_area (100): a river"
    );
    // With min_area 200 the ramp runs from 100 to 400 m2, so 128 m2 is on its foot: wet but far from a river.
    let foot = river_mask(&flow, 32, 32, 200.0);
    assert!(foot[31] > 0.0 && foot[31] < 0.2, "{}", foot[31]);
    assert!(
        river_mask(&flow, 32, 32, 256.0)[31] < 1e-6,
        "128 m2 is exactly min_area / 2 of 256: the ramp starts there"
    );
    assert!(mask[0] < 0.01, "the first cell is not");
}

// ---------- stream power ----------

fn power(k: f32) -> StreamPower {
    StreamPower {
        k,
        m: 0.5,
        uplift: 0.5,
        dt: 1.0,
        iterations: 10,
    }
}

#[test]
fn with_no_erodibility_only_the_uplift_map_acts() {
    let mut h = vec![10.0; 32 * 32];
    let map: Vec<f32> = (0..32 * 32)
        .map(|k| if k % 32 < 16 { 0.0 } else { 1.0 })
        .collect();
    stream_power(&mut grid(&mut h, 32, 32), Some(&map), &power(0.0), NEVER).unwrap();
    assert!((h[0] - 10.0).abs() < 1e-5, "left half has no uplift");
    assert!(
        (h[31] - 15.0).abs() < 1e-4,
        "right half rose by uplift * dt * iterations = 5: {}",
        h[31]
    );
}

#[test]
fn erosion_lowers_terrain_keeps_it_non_negative_and_is_deterministic() {
    let base = terrain(64, 64);
    let mut a = base.clone();
    let mut b = base.clone();
    let mut params = power(0.01);
    params.uplift = 0.0;
    stream_power(&mut grid(&mut a, 64, 64), None, &params, NEVER).unwrap();
    stream_power(&mut grid(&mut b, 64, 64), None, &params, NEVER).unwrap();
    assert_eq!(a, b);
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
    assert!(
        mean(&a) < mean(&base) - 0.01,
        "{} against {}",
        mean(&a),
        mean(&base)
    );
    assert!(a.iter().all(|v| v.is_finite() && *v >= 0.0));
}

#[test]
fn stream_power_refuses_bad_parameters() {
    let mut h = terrain(16, 16);
    let mut bad_m = power(0.01);
    bad_m.m = 0.7;
    assert!(matches!(
        stream_power(&mut grid(&mut h, 16, 16), None, &bad_m, NEVER),
        Err(ErosionError::BadParameter { name: "m", .. })
    ));
    assert!(matches!(
        stream_power(&mut grid(&mut h, 16, 16), None, &power(-1.0), NEVER),
        Err(ErosionError::BadParameter { name: "k", .. })
    ));
    let short = vec![0.0; 10];
    assert!(matches!(
        stream_power(&mut grid(&mut h, 16, 16), Some(&short), &power(0.01), NEVER),
        Err(ErosionError::BadParameter { .. })
    ));
}

// ---------- thermal ----------

#[test]
fn thermal_erosion_conserves_material_and_relaxes_steep_slopes() {
    let mut h = vec![0.0; 40 * 40];
    h[20 * 40 + 20] = 60.0;
    let total: f32 = h.iter().sum();
    let params = Thermal {
        talus_deg: 30.0,
        rate: 0.5,
        iterations: 400,
    };
    thermal(&mut grid(&mut h, 40, 40), &params, NEVER).unwrap();
    let after: f32 = h.iter().sum();
    assert!((after - total).abs() / total < 1e-3, "{total} -> {after}");
    let limit = 35.0_f32.to_radians().tan();
    for j in 1..39 {
        for i in 1..39 {
            for (di, dj) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                let (ni, nj) = ((i as i32 + di) as usize, (j as i32 + dj) as usize);
                let distance = 2.0 * ((di * di + dj * dj) as f32).sqrt();
                assert!(
                    (h[j * 40 + i] - h[nj * 40 + ni]).abs() / distance <= limit,
                    "slope at {i},{j}"
                );
            }
        }
    }
}

#[test]
fn thermal_refuses_a_talus_angle_outside_one_to_eighty_nine() {
    let mut h = terrain(16, 16);
    for angle in [0.0, 90.0, -5.0] {
        let params = Thermal {
            talus_deg: angle,
            rate: 0.5,
            iterations: 1,
        };
        assert!(
            matches!(
                thermal(&mut grid(&mut h, 16, 16), &params, NEVER),
                Err(ErosionError::BadParameter { .. })
            ),
            "{angle}"
        );
    }
}

// ---------- hydraulic ----------

fn rain(amount: f32) -> Hydraulic {
    Hydraulic {
        rain: amount,
        evaporation: 0.3,
        capacity: 1.0,
        dissolve: 0.3,
        deposit: 0.3,
        iterations: 30,
    }
}

#[test]
fn without_rain_nothing_moves() {
    let base = terrain(32, 32);
    let mut h = base.clone();
    let hydrology = hydraulic(&mut grid(&mut h, 32, 32), &rain(0.0), NEVER).unwrap();
    assert_eq!(h, base);
    assert!(hydrology.water.iter().all(|w| *w == 0.0));
}

#[test]
fn rain_makes_water_and_sediment_and_is_deterministic() {
    let base = terrain(48, 48);
    let (mut a, mut b) = (base.clone(), base.clone());
    let x = hydraulic(&mut grid(&mut a, 48, 48), &rain(0.01), NEVER).unwrap();
    let y = hydraulic(&mut grid(&mut b, 48, 48), &rain(0.01), NEVER).unwrap();
    assert_eq!(a, b);
    assert_eq!(x, y);
    assert_eq!((x.water.len(), x.sediment.len()), (48 * 48, 48 * 48));
    assert!(x.water.iter().all(|w| w.is_finite() && *w >= 0.0));
    assert!(x.sediment.iter().all(|s| s.is_finite() && *s >= 0.0));
    assert!(x.water.iter().any(|w| *w > 0.0));
    assert_ne!(a, base);
    assert!(a.iter().all(|v| v.is_finite()));
}

// ---------- shared rules ----------

#[test]
fn every_function_can_be_cancelled() {
    let mut h = terrain(32, 32);
    let always = &|| true;
    assert!(matches!(
        route_flow(&grid(&mut h, 32, 32), always),
        Err(ErosionError::Cancelled)
    ));
    assert!(matches!(
        stream_power(&mut grid(&mut h, 32, 32), None, &power(0.01), always),
        Err(ErosionError::Cancelled)
    ));
    let t = Thermal {
        talus_deg: 30.0,
        rate: 0.5,
        iterations: 5,
    };
    assert!(matches!(
        thermal(&mut grid(&mut h, 32, 32), &t, always),
        Err(ErosionError::Cancelled)
    ));
    assert!(matches!(
        hydraulic(&mut grid(&mut h, 32, 32), &rain(0.01), always),
        Err(ErosionError::Cancelled)
    ));
}

#[test]
fn a_wrong_slice_or_a_huge_grid_is_refused() {
    let mut h = vec![0.0; 10];
    assert!(matches!(
        route_flow(&grid(&mut h, 4, 4), NEVER),
        Err(ErosionError::BadShape { .. })
    ));
    let mut tiny = vec![0.0; 9];
    assert!(matches!(
        route_flow(&grid(&mut tiny, 3, 3), NEVER),
        Err(ErosionError::BadShape { .. })
    ));
    let mut big = vec![0.0; 2049 * 4];
    let error = route_flow(&grid(&mut big, 2049, 4), NEVER).unwrap_err();
    assert!(
        matches!(error, ErosionError::TooLarge { .. })
            && error.to_string().contains("smaller tiles")
    );
}

#[test]
fn the_halo_grows_with_the_iterations() {
    let few = halo_for(
        &Thermal {
            talus_deg: 30.0,
            rate: 0.5,
            iterations: 5,
        },
        &rain(0.01),
    );
    let mut many_rain = rain(0.01);
    many_rain.iterations = 120;
    let many = halo_for(
        &Thermal {
            talus_deg: 30.0,
            rate: 0.5,
            iterations: 100,
        },
        &many_rain,
    );
    assert!(few.cells > 0 && many.cells > few.cells);
}

// ---------- tiles ----------

fn window(world: &[f32], width: usize, x0: usize, z0: usize, w: usize, d: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(w * d);
    for j in z0..z0 + d {
        out.extend_from_slice(&world[j * width + x0..j * width + x0 + w]);
    }
    out
}

#[test]
fn refined_tiles_agree_on_their_shared_edge_and_with_the_whole_world() {
    const SIDE: usize = 256;
    let world = terrain(SIDE, SIDE);
    // The coarse world: every 4th fine cell, eroded once as a whole.
    let (cw, mut coarse) = (SIDE / 4, Vec::new());
    for j in 0..SIDE / 4 {
        for i in 0..cw {
            coarse.push(world[j * 4 * SIDE + i * 4]);
        }
    }
    let params = StreamPower {
        k: 0.002,
        m: 0.5,
        uplift: 0.0,
        dt: 1.0,
        iterations: 20,
    };
    coarse_world(
        &mut Grid {
            heights: &mut coarse,
            width: cw,
            depth: cw,
            cell: 8.0,
        },
        &params,
        NEVER,
    )
    .unwrap();
    let constraint = Constraint {
        coarse_heights: &coarse,
        coarse_width: cw,
        coarse_depth: cw,
        coarse_cell: 8.0,
        origin: [0.0, 0.0],
        strength: 0.5,
    };
    let thermal_params = Thermal {
        talus_deg: 35.0,
        rate: 0.5,
        iterations: 20,
    };
    let hydraulic_params = Hydraulic {
        rain: 0.01,
        evaporation: 0.5,
        capacity: 1.0,
        dissolve: 0.3,
        deposit: 0.3,
        iterations: 20,
    };
    let halo = halo_for(&thermal_params, &hydraulic_params);
    let h = halo.cells;
    assert!(h > 0 && h < 64, "a usable halo: {h}");

    let refine = |x0: usize, z0: usize, w: usize, d: usize| {
        let mut cells = window(&world, SIDE, x0, z0, w, d);
        let mut tile = Grid {
            heights: &mut cells,
            width: w,
            depth: d,
            cell: 2.0,
        };
        refine_tile(
            &mut tile,
            [x0 as f32 * 2.0, z0 as f32 * 2.0],
            &halo,
            &constraint,
            &thermal_params,
            &hydraulic_params,
            NEVER,
        )
        .unwrap();
        cells
    };
    let left = refine(0, 0, 128 + h, 128 + h);
    let right = refine(128 - h, 0, 128 + h, 128 + h);
    let whole = refine(0, 0, SIDE, SIDE);

    let stats = |pairs: &mut dyn Iterator<Item = (f32, f32)>| {
        let (mut sum, mut worst, mut n) = (0.0_f64, 0.0_f32, 0.0_f64);
        for (a, b) in pairs {
            let d = (a - b).abs();
            sum += f64::from(d * d);
            worst = worst.max(d);
            n += 1.0;
        }
        ((sum / n).sqrt() as f32, worst)
    };
    // The shared edge: world columns 127 and 128 for rows 0..128.
    let (rms, worst) = stats(&mut (0..128).flat_map(|j| {
        let (l, r) = (&left, &right);
        [127usize, 128]
            .into_iter()
            .map(move |x| (l[j * (128 + h) + x], r[j * (128 + h) + (x - (128 - h))]))
    }));
    assert!(
        rms <= 0.05 && worst <= 0.3,
        "shared edge: rms {rms}, max {worst}"
    );
    // The left tile's own area against the same area refined inside the whole world.
    let (rms, worst) = stats(&mut (0..128).flat_map(|j| {
        let (l, w) = (&left, &whole);
        (0..128).map(move |x| (l[j * (128 + h) + x], w[j * SIDE + x]))
    }));
    assert!(
        rms <= 0.05 && worst <= 0.3,
        "alone against whole: rms {rms}, max {worst}"
    );
}
