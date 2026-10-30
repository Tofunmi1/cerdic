//! Behaviour that separates real erosion from a token implementation (added after review of the first Codex pass). Do not weaken.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::unwrap_used,
    reason = "fixtures unwrap valid data and build terrain from small grid indices"
)]
use std::time::Instant;
use ve_world::erosion::{
    Constraint, Grid, Hydraulic, StreamPower, Thermal, coarse_world, halo_for, hydraulic,
    refine_tile, route_flow, stream_power,
};

const NEVER: &dyn Fn() -> bool = &|| false;

fn hash(x: i64, z: i64) -> f32 {
    let mut h = (x.wrapping_mul(374_761_393) ^ z.wrapping_mul(668_265_263)) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

/// Value noise on a 12-cell lattice with a fall toward +x and +z.
fn terrain(width: usize, depth: usize) -> Vec<f32> {
    let mut out = vec![0.0; width * depth];
    for j in 0..depth {
        for i in 0..width {
            let (fx, fz) = (i as f32 / 12.0, j as f32 / 12.0);
            let (x0, z0) = (fx.floor() as i64, fz.floor() as i64);
            let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
            let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
            let top = hash(x0, z0) * (1.0 - sx) + hash(x0 + 1, z0) * sx;
            let bottom = hash(x0, z0 + 1) * (1.0 - sx) + hash(x0 + 1, z0 + 1) * sx;
            out[j * width + i] =
                400.0 - 0.3 * i as f32 - 0.2 * j as f32 + 80.0 * (top * (1.0 - sz) + bottom * sz);
        }
    }
    out
}

fn grid(heights: &mut [f32], width: usize, depth: usize) -> Grid<'_> {
    Grid {
        heights,
        width,
        depth,
        cell: 2.0,
    }
}

#[test]
fn stream_power_cuts_channels_where_water_collects() {
    let base = terrain(96, 96);
    let mut h = base.clone();
    let flow = route_flow(&grid(&mut h.clone(), 96, 96), NEVER).unwrap();
    let params = StreamPower {
        k: 0.004,
        m: 0.5,
        uplift: 0.0,
        dt: 1.0,
        iterations: 30,
    };
    stream_power(&mut grid(&mut h, 96, 96), None, &params, NEVER).unwrap();
    let mut order: Vec<usize> = (0..96 * 96).collect();
    order.sort_by(|a, b| flow.area[*a].total_cmp(&flow.area[*b]));
    let drop =
        |cells: &[usize]| cells.iter().map(|i| base[*i] - h[*i]).sum::<f32>() / cells.len() as f32;
    let (low, high) = (
        drop(&order[..order.len() / 2]),
        drop(&order[order.len() * 95 / 100..]),
    );
    // Lowering a channel also pulls the slopes that drain to it, so hillslopes drop too; plain diffusion would give a ratio near 1.
    assert!(
        high > 2.0 * low.max(1e-3),
        "channels incise {high} m, hillslopes {low} m"
    );
}

#[test]
fn the_implicit_scheme_stays_stable_for_huge_steps() {
    let mut h = terrain(64, 64);
    let params = StreamPower {
        k: 50.0,
        m: 1.0,
        uplift: 0.0,
        dt: 50.0,
        iterations: 3,
    };
    stream_power(&mut grid(&mut h, 64, 64), None, &params, NEVER).unwrap();
    assert!(
        h.iter().all(|v| v.is_finite() && *v >= 0.0 && *v < 2000.0),
        "no blow-up, no negative heights"
    );
    // After the run every cell must still drain downhill or flat: no cell sits below all of its neighbours except at the border.
    for j in 1..63 {
        for i in 1..63 {
            let here = h[j * 64 + i];
            let lowest_neighbour = [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
            ]
            .iter()
            .map(|(dx, dz)| h[(j as i32 + dz) as usize * 64 + (i as i32 + dx) as usize])
            .fold(f32::MAX, f32::min);
            assert!(
                lowest_neighbour <= here + 1e-3 || here < 1e-3,
                "an eroded pit at {i},{j}"
            );
        }
    }
}

#[test]
fn water_pools_at_the_bottom_of_a_bowl() {
    let mut h: Vec<f32> = (0..48 * 48)
        .map(|k| {
            let (i, j) = ((k % 48) as f32 - 24.0, (k / 48) as f32 - 24.0);
            (i * i + j * j) * 0.05
        })
        .collect();
    let params = Hydraulic {
        rain: 0.01,
        evaporation: 0.0,
        capacity: 1.0,
        dissolve: 0.3,
        deposit: 0.3,
        iterations: 80,
    };
    let hydrology = hydraulic(&mut grid(&mut h, 48, 48), &params, NEVER).unwrap();
    let centre = hydrology.water[24 * 48 + 24];
    let rim = hydrology.water[24 * 48 + 3];
    assert!(centre > 3.0 * rim.max(1e-4), "centre {centre}, rim {rim}");
}

#[test]
fn a_closed_basin_conserves_ground_plus_sediment() {
    // A 1000 m wall around a sloping floor: nothing can leave, so ground plus suspended sediment is conserved.
    let (w, d) = (40usize, 40usize);
    let mut h: Vec<f32> = (0..w * d)
        .map(|k| {
            let (i, j) = (k % w, k / w);
            if i < 3 || j < 3 || i >= w - 3 || j >= d - 3 {
                1000.0
            } else {
                100.0 - 1.5 * i as f32 + 3.0 * hash(i as i64, j as i64)
            }
        })
        .collect();
    let before: f64 = h.iter().map(|v| f64::from(*v)).sum();
    let params = Hydraulic {
        rain: 0.02,
        evaporation: 0.2,
        capacity: 2.0,
        dissolve: 0.4,
        deposit: 0.4,
        iterations: 60,
    };
    let hydrology = hydraulic(&mut grid(&mut h, w, d), &params, NEVER).unwrap();
    let after: f64 = h.iter().map(|v| f64::from(*v)).sum::<f64>()
        + hydrology
            .sediment
            .iter()
            .map(|v| f64::from(*v))
            .sum::<f64>();
    assert!(
        (after - before).abs() / before < 0.01,
        "{before} -> {after}"
    );
    let moved: f32 = (0..w * d)
        .map(|k| (h[k] - if k % w < 3 { 1000.0 } else { h[k] }).abs())
        .sum();
    let _ = moved;
}

#[test]
fn hydraulic_erosion_actually_moves_ground_on_a_slope() {
    let base = terrain(64, 64);
    let mut h = base.clone();
    let params = Hydraulic {
        rain: 0.02,
        evaporation: 0.2,
        capacity: 2.0,
        dissolve: 0.4,
        deposit: 0.4,
        iterations: 60,
    };
    hydraulic(&mut grid(&mut h, 64, 64), &params, NEVER).unwrap();
    let changed = base
        .iter()
        .zip(&h)
        .filter(|(a, b)| (*a - *b).abs() > 0.01)
        .count();
    assert!(
        changed > 64 * 64 / 4,
        "only {changed} cells changed by more than a centimetre"
    );
}

#[test]
#[ignore = "timing: run with `cargo test -p ve-world --release -- --ignored`"]
fn erosion_is_fast_enough_for_a_tile() {
    // Release builds only: `cargo test -p ve-erosion --release -- --ignored`. 512 x 512, 50 stream-power iterations under 5 s.
    let mut h = terrain(512, 512);
    let params = StreamPower {
        k: 0.002,
        m: 0.5,
        uplift: 0.0,
        dt: 1.0,
        iterations: 50,
    };
    let start = Instant::now();
    stream_power(&mut grid(&mut h, 512, 512), None, &params, NEVER).unwrap();
    let seconds = start.elapsed().as_secs_f32();
    assert!(seconds < 5.0, "stream power took {seconds} s");
    let thermal_params = Thermal {
        talus_deg: 35.0,
        rate: 0.5,
        iterations: 30,
    };
    let hydraulic_params = Hydraulic {
        rain: 0.01,
        evaporation: 0.3,
        capacity: 1.0,
        dissolve: 0.3,
        deposit: 0.3,
        iterations: 30,
    };
    let start = Instant::now();
    ve_world::erosion::thermal(&mut grid(&mut h, 512, 512), &thermal_params, NEVER).unwrap();
    hydraulic(&mut grid(&mut h, 512, 512), &hydraulic_params, NEVER).unwrap();
    let seconds = start.elapsed().as_secs_f32();
    assert!(seconds < 5.0, "thermal + hydraulic took {seconds} s");
}

fn window(world: &[f32], width: usize, x0: usize, z0: usize, w: usize, d: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(w * d);
    for j in z0..z0 + d {
        out.extend_from_slice(&world[j * width + x0..j * width + x0 + w]);
    }
    out
}

/// Refines two neighbouring 128-cell tiles of a 256-cell world with the given halo; returns (rms, max) of the disagreement on the
/// shared edge (world columns 127 and 128).
fn edge_error(halo: usize, strong: bool) -> (f32, f32) {
    const SIDE: usize = 256;
    let world = terrain(SIDE, SIDE);
    let mut coarse: Vec<f32> = (0..SIDE / 4)
        .flat_map(|j| (0..SIDE / 4).map(move |i| (i, j)))
        .map(|(i, j)| world[j * 4 * SIDE + i * 4])
        .collect();
    let power = StreamPower {
        k: 0.002,
        m: 0.5,
        uplift: 0.0,
        dt: 1.0,
        iterations: 20,
    };
    coarse_world(
        &mut Grid {
            heights: &mut coarse,
            width: SIDE / 4,
            depth: SIDE / 4,
            cell: 8.0,
        },
        &power,
        NEVER,
    )
    .unwrap();
    let constraint = Constraint {
        coarse_heights: &coarse,
        coarse_width: SIDE / 4,
        coarse_depth: SIDE / 4,
        coarse_cell: 8.0,
        origin: [0.0, 0.0],
        strength: 0.2,
    };
    let (iterations, rain) = if strong { (40, 0.05) } else { (20, 0.01) };
    let thermal = Thermal {
        talus_deg: 30.0,
        rate: 0.5,
        iterations,
    };
    let water = Hydraulic {
        rain,
        evaporation: 0.3,
        capacity: 2.0,
        dissolve: 0.4,
        deposit: 0.4,
        iterations,
    };
    // Both tiles contain world columns 127 and 128 (the left one ends `halo` cells past them, the right one starts `halo` cells before).
    let width = 129 + halo;
    let refine = |x0: usize| {
        let mut cells = window(&world, SIDE, x0, 0, width, 128 + halo);
        let mut tile = Grid {
            heights: &mut cells,
            width,
            depth: 128 + halo,
            cell: 2.0,
        };
        refine_tile(
            &mut tile,
            [x0 as f32 * 2.0, 0.0],
            &ve_world::erosion::Halo { cells: halo },
            &constraint,
            &thermal,
            &water,
            NEVER,
        )
        .unwrap();
        cells
    };
    let right_start = 127 - halo;
    let (left, right) = (refine(0), refine(right_start));
    let (mut sum, mut worst, mut n) = (0.0_f64, 0.0_f32, 0.0_f64);
    for j in 0..128 {
        for x in [127usize, 128] {
            let d = (left[j * width + x] - right[j * width + (x - right_start)]).abs();
            sum += f64::from(d * d);
            worst = worst.max(d);
            n += 1.0;
        }
    }
    ((sum / n).sqrt() as f32, worst)
}

#[test]
fn the_halo_is_what_hides_the_seam() {
    let needed = halo_for(
        &Thermal {
            talus_deg: 30.0,
            rate: 0.5,
            iterations: 40,
        },
        &Hydraulic {
            rain: 0.05,
            evaporation: 0.3,
            capacity: 2.0,
            dissolve: 0.4,
            deposit: 0.4,
            iterations: 40,
        },
    )
    .cells;
    assert!(
        needed > 0 && needed <= 96,
        "a usable halo for 40 iterations: {needed}"
    );
    let (bare_rms, bare_max) = edge_error(0, true);
    let (halo_rms, halo_max) = edge_error(needed, true);
    assert!(
        bare_max > 0.05,
        "the test is meaningful: without a halo the edge disagrees ({bare_rms} rms, {bare_max} max)"
    );
    assert!(
        halo_rms <= 0.5 * bare_rms && halo_max <= 0.5 * bare_max,
        "halo {halo_rms}/{halo_max} against bare {bare_rms}/{bare_max}"
    );
    assert!(
        halo_rms <= 0.05 && halo_max <= 0.3,
        "with the halo the seam is invisible: {halo_rms} rms, {halo_max} max"
    );
}
