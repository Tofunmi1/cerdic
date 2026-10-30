//! `ve_world::terrain_lod`: which landscape tiles to draw, and which to load and drop, as the camera moves (ADR 0070 port 6, the tile selection of RedGPU's
//! landscape and the CDLOD idea behind it). A quadtree over the world; tiles near the camera are small, far tiles are big; a tile that is about to be replaced by a
//! coarser one reports a morph factor so the shader can blend the two heights without a pop.
#![expect(clippy::cast_precision_loss, reason = "small counts")]
use std::collections::BTreeSet;
use ve_world::terrain_lod::{LodOptions, SelectedTile, diff_tiles, select_tiles};

const WORLD: ([f64; 2], [f64; 2]) = ([0.0, 0.0], [1024.0, 1024.0]);

fn options() -> LodOptions {
    LodOptions {
        tile_size: 32.0,
        levels: 6,
        base_range: 48.0,
        range_ratio: 2.0,
        morph: 0.3,
    }
}

fn select(camera: [f64; 3]) -> Vec<SelectedTile> {
    select_tiles(WORLD.0, WORLD.1, camera, &options(), [0.0, 40.0])
}

fn area(t: &SelectedTile) -> f64 {
    t.size * t.size
}

fn overlaps(a: &SelectedTile, b: &SelectedTile) -> bool {
    a.min[0] < b.min[0] + b.size - 1e-9
        && b.min[0] < a.min[0] + a.size - 1e-9
        && a.min[1] < b.min[1] + b.size - 1e-9
        && b.min[1] < a.min[1] + a.size - 1e-9
}

#[test]
fn the_selected_tiles_cover_the_world_exactly_once() {
    for camera in [
        [512.0, 10.0, 512.0],
        [5.0, 3.0, 1000.0],
        [2000.0, 300.0, -500.0],
    ] {
        let tiles = select(camera);
        let total: f64 = tiles.iter().map(area).sum();
        assert!(
            (total - 1024.0 * 1024.0).abs() < 1e-6,
            "{camera:?}: {total}"
        );
        for (i, a) in tiles.iter().enumerate() {
            for b in &tiles[i + 1..] {
                assert!(!overlaps(a, b), "{camera:?}: {a:?} overlaps {b:?}");
            }
        }
    }
}

#[test]
fn tiles_are_finer_near_the_camera_and_coarser_far_away() {
    let tiles = select([100.0, 10.0, 100.0]);
    let near = tiles
        .iter()
        .filter(|t| {
            ((t.min[0] + t.size / 2.0 - 100.0).hypot(t.min[1] + t.size / 2.0 - 100.0)) < 60.0
        })
        .map(|t| t.level)
        .min()
        .unwrap();
    let far = tiles
        .iter()
        .filter(|t| {
            ((t.min[0] + t.size / 2.0 - 100.0).hypot(t.min[1] + t.size / 2.0 - 100.0)) > 700.0
        })
        .map(|t| t.level)
        .max()
        .unwrap();
    assert!(far > near, "near level {near}, far level {far}");
    assert_eq!(near, 0, "the tiles at the camera are the finest");
}

#[test]
fn neighbouring_tiles_differ_by_at_most_one_level() {
    let tiles = select([300.0, 12.0, 700.0]);
    for (i, a) in tiles.iter().enumerate() {
        for b in &tiles[i + 1..] {
            let touch_x = (a.min[0] + a.size - b.min[0]).abs() < 1e-9
                || (b.min[0] + b.size - a.min[0]).abs() < 1e-9;
            let touch_z = (a.min[1] + a.size - b.min[1]).abs() < 1e-9
                || (b.min[1] + b.size - a.min[1]).abs() < 1e-9;
            let share_z =
                a.min[1] < b.min[1] + b.size - 1e-9 && b.min[1] < a.min[1] + a.size - 1e-9;
            let share_x =
                a.min[0] < b.min[0] + b.size - 1e-9 && b.min[0] < a.min[0] + a.size - 1e-9;
            if (touch_x && share_z) || (touch_z && share_x) {
                assert!(a.level.abs_diff(b.level) <= 1, "{a:?} next to {b:?}");
            }
        }
    }
}

#[test]
fn a_higher_camera_sees_no_more_tiles_than_a_lower_one() {
    let low = select([512.0, 5.0, 512.0]).len();
    let high = select([512.0, 400.0, 512.0]).len();
    assert!(high <= low, "{high} against {low}");
}

#[test]
fn a_camera_far_outside_the_world_selects_few_coarse_tiles() {
    let tiles = select([20_000.0, 50.0, 20_000.0]);
    assert!(tiles.len() <= 16, "{}", tiles.len());
    assert!(
        tiles.iter().all(|t| t.level >= 3),
        "{:?}",
        tiles.iter().map(|t| t.level).collect::<Vec<_>>()
    );
}

#[test]
fn the_morph_factor_is_zero_near_and_rises_to_one_at_the_edge_of_a_tiles_range() {
    let tiles = select([512.0, 10.0, 512.0]);
    assert!(
        tiles.iter().all(|t| (0.0..=1.0).contains(&t.morph)),
        "{:?}",
        tiles.iter().map(|t| t.morph).collect::<Vec<_>>()
    );
    assert!(
        tiles.iter().any(|t| t.morph > 0.0),
        "some tile is morphing toward its coarser parent"
    );
    let finest_at_camera = tiles
        .iter()
        .filter(|t| {
            t.min[0] <= 512.0
                && 512.0 < t.min[0] + t.size
                && t.min[1] <= 512.0
                && 512.0 < t.min[1] + t.size
        })
        .collect::<Vec<_>>();
    assert!(
        finest_at_camera.iter().all(|t| t.morph < 1e-9),
        "{finest_at_camera:?}"
    );
}

#[test]
fn the_same_input_gives_the_same_tiles_in_the_same_order() {
    let a = select([123.0, 9.0, 456.0]);
    let b = select([123.0, 9.0, 456.0]);
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}

#[test]
fn tile_coordinates_match_their_position_and_size() {
    for t in select([700.0, 20.0, 300.0]) {
        let size = 32.0 * f64::from(1_u32 << t.level);
        assert!((t.size - size).abs() < 1e-9, "{t:?}");
        assert!(
            (t.min[0] - t.x as f64 * size).abs() < 1e-9
                && (t.min[1] - t.z as f64 * size).abs() < 1e-9,
            "{t:?}"
        );
    }
}

#[test]
fn the_diff_between_two_selections_lists_what_to_load_and_what_to_drop() {
    let a = select([200.0, 10.0, 200.0]);
    assert!(diff_tiles(&a, &a).load.is_empty() && diff_tiles(&a, &a).unload.is_empty());
    let b = select([260.0, 10.0, 200.0]);
    let d = diff_tiles(&a, &b);
    let keys_a: BTreeSet<_> = a.iter().map(|t| (t.level, t.x, t.z)).collect();
    let keys_b: BTreeSet<_> = b.iter().map(|t| (t.level, t.x, t.z)).collect();
    let load: BTreeSet<_> = d.load.iter().map(|t| (t.level, t.x, t.z)).collect();
    let unload: BTreeSet<_> = d.unload.iter().map(|t| (t.level, t.x, t.z)).collect();
    assert_eq!(load, keys_b.difference(&keys_a).copied().collect());
    assert_eq!(unload, keys_a.difference(&keys_b).copied().collect());
    assert!(
        !load.is_empty() && !unload.is_empty(),
        "moving 60 m changes some tiles"
    );
    assert!(load.len() < keys_b.len(), "most tiles stay");
}

#[test]
fn bad_input_gives_no_tiles_instead_of_a_panic() {
    assert!(
        select_tiles(
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0, 0.0],
            &options(),
            [0.0, 1.0]
        )
        .is_empty()
    );
    assert!(
        select_tiles(
            WORLD.0,
            WORLD.1,
            [f64::NAN, 0.0, 0.0],
            &options(),
            [0.0, 1.0]
        )
        .is_empty()
    );
    let zero = LodOptions {
        tile_size: 0.0,
        ..options()
    };
    assert!(select_tiles(WORLD.0, WORLD.1, [1.0, 1.0, 1.0], &zero, [0.0, 1.0]).is_empty());
}
