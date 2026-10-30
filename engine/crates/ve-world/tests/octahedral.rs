//! `ve_world::lod::octahedral`: view directions packed into a square (ADR 0070 port 4, the octahedral impostors of RedGPU's foliage). A far tree is a grid of
//! pre-rendered frames, one per view direction; the frame to show, and how to blend three neighbouring frames, comes from this mapping.
#![expect(clippy::cast_precision_loss, reason = "small counts")]
use ve_world::lod::octahedral::{
    decode, encode, frame_direction, hemi_decode, hemi_encode, select_views,
};

fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / n, v[1] / n, v[2] / n]
}

/// The angle between two directions in degrees, from `atan2(|cross|, dot)`: unlike `acos(dot)` it stays exact for nearly equal directions and ignores
/// the tiny length error of fixtures that are only approximately unit vectors.
fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let cross_len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    cross_len.atan2(dot).to_degrees()
}

/// Directions spread over the whole sphere (a golden-angle spiral).
fn sphere(count: usize) -> Vec<[f64; 3]> {
    (0..count)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / count as f64;
            let r = (1.0 - y * y).sqrt();
            let a = i as f64 * 2.399_963_229_728_653;
            [r * a.cos(), y, r * a.sin()]
        })
        .collect()
}

fn upper(count: usize) -> Vec<[f64; 3]> {
    sphere(count * 2)
        .into_iter()
        .filter(|d| d[1] >= 0.0)
        .collect()
}

#[test]
fn the_full_sphere_mapping_round_trips_and_stays_in_the_unit_square() {
    for d in sphere(2000) {
        let uv = encode(d);
        assert!(
            (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]),
            "{uv:?}"
        );
        let back = decode(uv);
        assert!(angle(back, d) < 1e-6, "{d:?} -> {uv:?} -> {back:?}");
    }
}

#[test]
fn the_poles_land_where_the_mapping_says() {
    let top = encode([0.0, 1.0, 0.0]);
    assert!(
        (top[0] - 0.5).abs() < 1e-9 && (top[1] - 0.5).abs() < 1e-9,
        "{top:?}"
    );
    let bottom = encode([0.0, -1.0, 0.0]);
    let corner = (bottom[0] - bottom[0].round()).abs() < 1e-9
        && (bottom[1] - bottom[1].round()).abs() < 1e-9;
    assert!(
        corner,
        "straight down is a corner of the square: {bottom:?}"
    );
}

#[test]
fn decode_always_gives_a_unit_vector() {
    for i in 0..=20 {
        for j in 0..=20 {
            let d = decode([f64::from(i) / 20.0, f64::from(j) / 20.0]);
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-9, "{d:?}");
        }
    }
}

#[test]
fn the_hemisphere_mapping_round_trips_for_upward_directions_and_puts_up_in_the_middle() {
    let up = hemi_encode([0.0, 1.0, 0.0]);
    assert!(
        (up[0] - 0.5).abs() < 1e-9 && (up[1] - 0.5).abs() < 1e-9,
        "{up:?}"
    );
    for d in upper(1500) {
        let uv = hemi_encode(d);
        assert!(
            (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]),
            "{uv:?}"
        );
        assert!(angle(hemi_decode(uv), d) < 1e-6, "{d:?}");
    }
}

#[test]
fn every_hemisphere_decode_looks_up_or_along_the_horizon() {
    for i in 0..=20 {
        for j in 0..=20 {
            let d = hemi_decode([f64::from(i) / 20.0, f64::from(j) / 20.0]);
            assert!(d[1] >= -1e-9, "{d:?}");
        }
    }
}

#[test]
fn a_direction_below_the_horizon_maps_like_its_projection_onto_the_horizon() {
    let below = unit([0.6, -0.4, 0.3]);
    let on_horizon = unit([0.6, 0.0, 0.3]);
    let a = hemi_encode(below);
    let b = hemi_encode(on_horizon);
    assert!(
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9,
        "{a:?} {b:?}"
    );
}

#[test]
fn frame_directions_are_the_grid_vertices_of_the_square() {
    let grid = 9;
    for col in 0..grid {
        for row in 0..grid {
            let d = frame_direction(col, row, grid, false);
            let uv = encode(d);
            assert!(
                (uv[0] - col as f64 / (grid - 1) as f64).abs() < 1e-9,
                "{col} {row} {uv:?}"
            );
            assert!(
                (uv[1] - row as f64 / (grid - 1) as f64).abs() < 1e-9,
                "{col} {row} {uv:?}"
            );
        }
    }
    let hemi = frame_direction(4, 4, 9, true);
    assert!(
        angle(hemi, [0.0, 1.0, 0.0]) < 1e-9,
        "the middle frame of a hemisphere grid looks straight down at the top"
    );
}

#[test]
fn three_frames_are_chosen_with_weights_that_add_to_one() {
    for grid in [6, 9, 12] {
        for d in sphere(500) {
            let views = select_views(d, grid, false);
            let sum: f64 = views.iter().map(|v| v.weight).sum();
            assert!((sum - 1.0).abs() < 1e-9, "{sum}");
            assert!(
                views
                    .iter()
                    .all(|v| v.weight >= -1e-9 && v.col < grid && v.row < grid),
                "{views:?}"
            );
        }
    }
}

#[test]
fn a_direction_that_is_exactly_a_frame_picks_that_frame_alone() {
    let grid = 9;
    for (col, row) in [(0, 0), (3, 5), (8, 8), (4, 4)] {
        let d = frame_direction(col, row, grid, false);
        let views = select_views(d, grid, false);
        let hit = views
            .iter()
            .find(|v| v.col == col && v.row == row)
            .unwrap_or_else(|| panic!("frame ({col},{row}) not chosen: {views:?}"));
        assert!(hit.weight > 1.0 - 1e-9, "{hit:?}");
    }
}

#[test]
fn the_blend_of_the_chosen_frames_looks_roughly_where_asked() {
    let grid = 12;
    for d in sphere(400) {
        let views = select_views(d, grid, false);
        let mut avg = [0.0; 3];
        for v in &views {
            let f = frame_direction(v.col, v.row, grid, false);
            for k in 0..3 {
                avg[k] += f[k] * v.weight;
            }
        }
        assert!(
            angle(unit(avg), d) < 12.0,
            "{d:?}: blend looks {:.1} degrees away",
            angle(unit(avg), d)
        );
    }
}

#[test]
fn the_hemisphere_grid_chooses_frames_the_same_way() {
    let grid = 8;
    for d in upper(300) {
        let views = select_views(d, grid, true);
        let sum: f64 = views.iter().map(|v| v.weight).sum();
        assert!((sum - 1.0).abs() < 1e-9);
        let mut avg = [0.0; 3];
        for v in &views {
            let f = frame_direction(v.col, v.row, grid, true);
            for k in 0..3 {
                avg[k] += f[k] * v.weight;
            }
        }
        assert!(angle(unit(avg), d) < 14.0, "{d:?}");
    }
}

#[test]
fn bad_directions_do_not_panic_and_still_give_valid_frames() {
    for d in [
        [0.0, 0.0, 0.0],
        [f64::NAN, 1.0, 0.0],
        [f64::INFINITY, 0.0, 0.0],
    ] {
        let views = select_views(d, 8, false);
        let sum: f64 = views.iter().map(|v| v.weight).sum();
        assert!(
            (sum - 1.0).abs() < 1e-9 && views.iter().all(|v| v.col < 8 && v.row < 8),
            "{d:?} {views:?}"
        );
    }
}
