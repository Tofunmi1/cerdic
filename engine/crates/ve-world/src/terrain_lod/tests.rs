//! Edge cases beyond the supplied integration contract.
use super::{LodOptions, SelectedTile, diff_tiles, select_tiles};

fn options() -> LodOptions {
    LodOptions {
        tile_size: 8.0,
        levels: 7,
        base_range: 5.0,
        range_ratio: 1.0,
        morph: 0.25,
    }
}

fn neighbours(a: &SelectedTile, b: &SelectedTile) -> bool {
    let end_a = a.min.map(|v| v + a.size);
    let end_b = b.min.map(|v| v + b.size);
    let touches =
        |i: usize| (end_a[i] - b.min[i]).abs() < 1e-9 || (end_b[i] - a.min[i]).abs() < 1e-9;
    let shares = |i: usize| a.min[i] < end_b[i] && b.min[i] < end_a[i];
    (touches(0) && shares(1)) || (touches(1) && shares(0))
}

#[test]
fn balance_handles_small_ranges_negative_roots_and_partial_edges() {
    let min = [-333.0_f64, -177.0_f64];
    let max = [731.0_f64, 529.0_f64];
    for ratio in [0.4, 1.0, 1.2, 2.0, 4.0] {
        let options = LodOptions {
            range_ratio: ratio,
            ..options()
        };
        let tiles = select_tiles(min, max, [-30.0, 3.0, -70.0], &options, [0.0, 10.0]);
        let area: f64 = tiles
            .iter()
            .map(|t| {
                (max[0].min(t.min[0] + t.size) - min[0].max(t.min[0]))
                    * (max[1].min(t.min[1] + t.size) - min[1].max(t.min[1]))
            })
            .sum();
        assert!((area - (max[0] - min[0]) * (max[1] - min[1])).abs() < 1e-9);
        assert!(
            tiles
                .windows(2)
                .all(|pair| (pair[0].level, pair[0].z, pair[0].x)
                    < (pair[1].level, pair[1].z, pair[1].x))
        );
        for (i, a) in tiles.iter().enumerate() {
            for b in &tiles[i + 1..] {
                let overlap = (0..2).all(|axis| {
                    a.min[axis] < b.min[axis] + b.size && b.min[axis] < a.min[axis] + a.size
                });
                assert!(!overlap);
                if neighbours(a, b) {
                    assert!(
                        a.level.abs_diff(b.level) <= 1,
                        "ratio {ratio}: {a:?}, {b:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn morph_has_exact_start_midpoint_end_and_disabled_values() {
    let options = LodOptions {
        tile_size: 10.0,
        levels: 1,
        base_range: 10.0,
        range_ratio: 2.0,
        morph: 0.5,
    };
    for (x, expected) in [
        (-2.0, 0.0),
        (-5.0, 0.0),
        (-7.5, 0.5),
        (-10.0, 1.0),
        (-20.0, 1.0),
    ] {
        let tiles = select_tiles([0.0; 2], [10.0; 2], [x, 0.0, 5.0], &options, [0.0, 1.0]);
        assert_eq!(tiles.len(), 1);
        assert!((tiles[0].morph - expected).abs() < 1e-12);
    }
    for morph in [0.0, 1.0] {
        let options = LodOptions { morph, ..options };
        let tiles = select_tiles(
            [0.0; 2],
            [10.0; 2],
            [5.0, 1000.0, 5.0],
            &options,
            [0.0, 1.0],
        );
        assert!(
            tiles[0].morph.abs() < 1e-12,
            "the camera's x/z tile stays unmorphed"
        );
        let outside = select_tiles([0.0; 2], [10.0; 2], [-5.0, 0.0, 5.0], &options, [0.0, 1.0]);
        assert!((outside[0].morph - morph * 0.5).abs() < 1e-12);
    }
    let narrow = LodOptions {
        morph: f64::MIN_POSITIVE,
        ..options
    };
    let edge = select_tiles([0.0; 2], [10.0; 2], [-10.0, 0.0, 5.0], &narrow, [0.0, 1.0]);
    assert!((edge[0].morph - 1.0).abs() < 1e-12);
}

#[test]
fn root_bounds_preserve_sign_when_coordinate_division_underflows() {
    let options = LodOptions {
        tile_size: f64::MAX / 4.0,
        levels: 1,
        ..options()
    };
    let tiles = select_tiles([-1e-100; 2], [1e-100; 2], [0.0; 3], &options, [0.0, 1.0]);
    assert_eq!(tiles.len(), 4);
    assert_eq!(
        tiles.iter().map(|t| (t.x, t.z)).collect::<Vec<_>>(),
        [(-1, -1), (0, -1), (-1, 0), (0, 0)]
    );
}

#[test]
fn invalid_and_unrepresentable_inputs_return_empty() {
    let options = options();
    let invalid = [
        LodOptions {
            levels: 0,
            ..options
        },
        LodOptions {
            levels: usize::MAX,
            ..options
        },
        LodOptions {
            tile_size: f64::INFINITY,
            ..options
        },
        LodOptions {
            tile_size: f64::MIN_POSITIVE,
            ..options
        },
        LodOptions {
            base_range: 0.0,
            ..options
        },
        LodOptions {
            range_ratio: f64::NAN,
            ..options
        },
        LodOptions {
            range_ratio: -1.0,
            ..options
        },
        LodOptions {
            morph: -0.1,
            ..options
        },
        LodOptions {
            morph: 1.1,
            ..options
        },
    ];
    for options in invalid {
        assert!(select_tiles([0.0; 2], [1024.0; 2], [0.0; 3], &options, [0.0, 10.0]).is_empty());
    }
    for (min, max, camera, heights) in [
        ([0.0; 2], [f64::INFINITY; 2], [0.0; 3], [0.0, 1.0]),
        ([10.0; 2], [0.0; 2], [0.0; 3], [0.0, 1.0]),
        ([0.0; 2], [1024.0; 2], [0.0, f64::INFINITY, 0.0], [0.0, 1.0]),
        ([0.0; 2], [1024.0; 2], [0.0; 3], [1.0, 0.0]),
        ([0.0; 2], [1024.0; 2], [0.0; 3], [0.0, f64::NAN]),
    ] {
        assert!(select_tiles(min, max, camera, &options, heights).is_empty());
    }
}

#[test]
fn diff_ignores_morph_and_deduplicates_keys_in_sorted_order() {
    let tiles = select_tiles(
        [0.0; 2],
        [1024.0; 2],
        [31.0, 0.0, 31.0],
        &options(),
        [0.0, 10.0],
    );
    let mut changed: Vec<_> = tiles
        .iter()
        .rev()
        .map(|t| SelectedTile {
            morph: 1.0 - t.morph,
            ..*t
        })
        .collect();
    changed.extend_from_within(..);
    assert_eq!(diff_tiles(&tiles, &changed), super::TileDiff::default());
    assert_eq!(diff_tiles(&[], &changed).load.len(), tiles.len());
    assert_eq!(diff_tiles(&tiles, &[]).unload, tiles);
    assert_eq!(diff_tiles(&[], &[]), super::TileDiff::default());
}
