//! Biomes: rule text, soft windows, normalised blending, and seams between tiles. Do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures")]
use ve_world::biome::{parse, weigh};
use ve_world::terrain::{HeightGrid, Noise, generate};

fn land(origin: [f32; 2], size: usize) -> HeightGrid {
    let mut grid = HeightGrid::new(size, size, 2.0, origin).unwrap();
    let noise = Noise {
        seed: 5,
        scale: 120.0,
        octaves: 4,
        roughness: 0.5,
        lacunarity: 2.0,
        ridged: false,
        warp: 0.0,
        basis: None,
    };
    generate(&mut grid, &noise, 100.0, &|| false).unwrap();
    grid
}

#[test]
fn rules_are_read_with_open_ends_and_refuse_nonsense_with_the_fix() {
    let biomes = parse("forest: height 0..80, slope 0..30, wet 0.45..1; rock: slope 38..").unwrap();
    assert_eq!(biomes.len(), 2);
    assert_eq!(biomes[0].name, "forest");
    assert_eq!(biomes[0].height.unwrap().high, 80.0);
    assert!(biomes[1].slope.unwrap().high.is_infinite());
    assert!(biomes[1].height.is_none());
    for (text, hint) in [
        ("", "at least one"),
        ("forest slope 0..30", "colon"),
        ("a: slope 0-30", "0..30"),
        ("a: steep 0..1", "unknown measure"),
        ("a: slope 30..10", "low end"),
        ("a: slope 0..1; a: slope 2..3", "twice"),
        ("bad name: slope 0..1", "letters"),
    ] {
        let message = parse(text).unwrap_err();
        assert!(message.contains(hint), "{text:?} gave {message:?}");
    }
}

#[test]
fn weights_follow_the_windows_and_never_exceed_one_in_total() {
    let grid = land([0.0, 0.0], 96);
    let heights = grid.heights();
    let mut sorted = heights.to_vec();
    sorted.sort_by(f32::total_cmp);
    let (q30, q70) = (sorted[sorted.len() * 3 / 10], sorted[sorted.len() * 7 / 10]);
    // An open-ended window softens its closed end by the blend of 50 m, so "well inside" starts 5 m in.
    let span = 20.0;
    let by_height = parse(&format!("low: height ..{q30}; high: height {q70}..")).unwrap();
    let w = weigh(&grid, &by_height, 0.2, 200.0, 1);
    let (mut low, mut high) = (0, 0);
    for k in 0..heights.len() {
        let sum = w.biomes[0][k] + w.biomes[1][k];
        assert!(
            (0.0..=1.0 + 1e-4).contains(&sum),
            "weights never exceed one: {sum}"
        );
        if heights[k] > q70 + span * 0.3 + 0.1 {
            high += 1;
            assert!(
                w.biomes[1][k] > 0.9 && w.biomes[0][k] < 0.01,
                "high ground is high at {}",
                heights[k]
            );
        }
        if heights[k] < q30 - span * 0.3 {
            low += 1;
            assert!(w.biomes[0][k] > 0.9 && w.biomes[1][k] < 0.01);
        }
    }
    assert!(
        low > 100 && high > 50,
        "the land has both low and high ground: {low} {high}"
    );
    let by_slope = parse("flat: slope 0..15; steep: slope 25..90").unwrap();
    let w = weigh(&grid, &by_slope, 0.2, 200.0, 1);
    let (mut flat, mut steep) = (0, 0);
    for k in 0..heights.len() {
        if w.slope[k] < 8.0 {
            flat += 1;
            assert!(w.biomes[0][k] > 0.9 && w.biomes[1][k] < 0.01);
        }
        if w.slope[k] > 35.0 {
            steep += 1;
            assert!(w.biomes[1][k] > 0.9 && w.biomes[0][k] < 0.01);
        }
    }
    assert!(flat > 100, "there is flat ground: {flat}");
    let _ = steep;
    // Overlapping rules are normalised so a place never holds more than 1 of the biomes together.
    let overlapping = parse("a: height 0..1000; b: height 0..1000").unwrap();
    let w = weigh(&grid, &overlapping, 0.2, 200.0, 1);
    for k in 0..heights.len() {
        assert!(
            (w.biomes[0][k] + w.biomes[1][k] - 1.0).abs() < 1e-4,
            "two full biomes split the place: {} {}",
            w.biomes[0][k],
            w.biomes[1][k]
        );
    }
}

#[test]
fn a_boundary_fades_over_the_blend_width_instead_of_cutting() {
    let grid = land([0.0, 0.0], 96);
    let biomes = parse("high: height 50..").unwrap();
    let sharp = weigh(&grid, &biomes, 0.0, 200.0, 1);
    let soft = weigh(&grid, &biomes, 1.0, 200.0, 1);
    let partial = |w: &ve_world::biome::Weights| {
        w.biomes[0]
            .iter()
            .filter(|v| **v > 0.05 && **v < 0.95)
            .count()
    };
    assert!(
        partial(&soft) > partial(&sharp) * 3 + 20,
        "a wide blend has many partly-weighted cells: {} against {}",
        partial(&soft),
        partial(&sharp)
    );
}

#[test]
fn two_tiles_that_share_an_edge_agree_on_it_and_a_window_is_cut_from_a_larger_one() {
    // The same ground seen through two overlapping windows gives the same weights where they overlap (away from each window's own border).
    let big = land([0.0, 0.0], 128);
    let small = land([64.0, 64.0], 64);
    let biomes =
        parse("forest: height 0..70, slope 0..25, wet 0.3..1; rock: slope 20..90").unwrap();
    let (wb, ws) = (
        weigh(&big, &biomes, 0.2, 150.0, 9),
        weigh(&small, &biomes, 0.2, 150.0, 9),
    );
    let mut checked = 0;
    for j in 1..63 {
        for i in 1..63 {
            let (bi, bj) = (i + 32, j + 32);
            for n in 0..2 {
                let (a, b) = (wb.biomes[n][bj * 128 + bi], ws.biomes[n][j * 64 + i]);
                assert!(
                    (a - b).abs() < 1e-4,
                    "biome {n} differs at cell ({i}, {j}): {a} against {b}"
                );
            }
            assert!((wb.wet[bj * 128 + bi] - ws.wet[j * 64 + i]).abs() < 1e-5);
            checked += 1;
        }
    }
    assert!(checked > 3000);
}

#[test]
fn the_seed_changes_the_moisture_pattern_and_the_same_seed_repeats_it() {
    let grid = land([0.0, 0.0], 64);
    let biomes = parse("wetland: wet 0.6..1").unwrap();
    let (a, b, c) = (
        weigh(&grid, &biomes, 0.2, 100.0, 1),
        weigh(&grid, &biomes, 0.2, 100.0, 1),
        weigh(&grid, &biomes, 0.2, 100.0, 2),
    );
    assert_eq!(a.biomes[0], b.biomes[0]);
    assert_ne!(a.wet, c.wet);
}
