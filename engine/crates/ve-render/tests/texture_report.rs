//! `MaterialTextures::report`: what the scene's baked maps take, by size, against the budget.
use ve_render::{BUDGET_BYTES, MaterialTextures, TIER_SIZES, map_cost};

fn map(tier: usize) -> Vec<u8> {
    let side = TIER_SIZES[tier] as usize;
    vec![128; side * side * 4]
}

#[test]
fn an_empty_scene_reports_nothing_used() {
    let report = MaterialTextures::default().report();
    assert_eq!(report.maps, [0, 0, 0]);
    assert_eq!(report.bytes, [0, 0, 0]);
    assert_eq!(report.total_bytes, 0);
    assert_eq!(report.budget_bytes, BUDGET_BYTES);
    assert!(report.used_fraction.abs() < 1e-9);
    assert_eq!(report.astc_6x6_estimate_bytes, 0);
}

#[test]
fn maps_are_counted_and_costed_by_size() {
    let mut textures = MaterialTextures::default();
    textures.add(map(0)).unwrap();
    textures.add(map(0)).unwrap();
    textures.add(map(1)).unwrap();
    textures.add(map(2)).unwrap();
    let report = textures.report();
    assert_eq!(report.maps, [2, 1, 1]);
    let big = map_cost(map(0).len()).unwrap();
    let mid = map_cost(map(1).len()).unwrap();
    let small = map_cost(map(2).len()).unwrap();
    assert_eq!(report.bytes, [2 * big, mid, small]);
    assert_eq!(report.total_bytes, 2 * big + mid + small);
    assert_eq!(report.total_bytes, textures.bytes());
    let expected = report.total_bytes as f64 / BUDGET_BYTES as f64;
    assert!((f64::from(report.used_fraction) - expected).abs() < 1e-6);
}

#[test]
fn the_compressed_estimate_is_about_a_ninth() {
    // ASTC 6x6 is 128 bits per 36 texels, 3.56 bits a texel against 32 for RGBA8: 0.111 of the size, mips included.
    let mut textures = MaterialTextures::default();
    for _ in 0..4 {
        textures.add(map(0)).unwrap();
    }
    let report = textures.report();
    let ratio = report.astc_6x6_estimate_bytes as f64 / report.total_bytes as f64;
    assert!((0.10..0.125).contains(&ratio), "ratio {ratio}");
}

#[test]
fn clearing_the_maps_clears_the_report() {
    let mut textures = MaterialTextures::default();
    textures.add(map(1)).unwrap();
    textures.clear();
    let report = textures.report();
    assert_eq!(report.maps, [0, 0, 0]);
    assert_eq!(report.total_bytes, 0);
}

#[test]
fn a_full_budget_reads_as_nearly_one() {
    let mut textures = MaterialTextures::default();
    while textures.add(map(0)).is_some() {}
    let report = textures.report();
    assert!(report.used_fraction > 0.9 && report.used_fraction <= 1.0);
    assert_eq!(
        report.maps[0],
        BUDGET_BYTES / map_cost(map(0).len()).unwrap()
    );
}
