use super::{BUDGET_BYTES, MaterialTextures, TIER_SIZES, TIER_STRIDE, map_cost, tier_of_len};
use crate::textures_gpu::{allocated_layers, downsample, mip_levels};

/// A map of the given side, filled with `fill`.
fn map(side: u32, fill: u8) -> Vec<u8> {
    vec![fill; side as usize * side as usize * 4]
}

#[test]
fn the_three_sizes_are_recognised_and_nothing_else() {
    for (tier, side) in TIER_SIZES.into_iter().enumerate() {
        assert_eq!(tier_of_len(map(side, 0).len()), Some(tier));
    }
    assert_eq!(tier_of_len(16), None);
    assert_eq!(tier_of_len(0), None);
    assert_eq!(tier_of_len(128 * 128 * 4), None, "128 is not a size");
    assert_eq!(tier_of_len(2048 * 2048 * 4), None, "2048 is not a size");
    assert_eq!(tier_of_len(1024 * 512 * 4), None, "not square");
}

#[test]
fn a_maps_cost_counts_its_whole_mip_chain() {
    // 1024 + 512 + ... + 1 texels squared is (4^11 - 1) / 3.
    let large = map_cost(map(1024, 0).len()).unwrap_or(0);
    assert_eq!(large, (4_usize.pow(11) - 1) / 3 * 4);
    assert!(large > 1024 * 1024 * 4 && large < 1024 * 1024 * 4 * 4 / 3 + 8);
    // Each size down is a quarter of the one above, give or take the last levels.
    let medium = map_cost(map(512, 0).len()).unwrap_or(0);
    let small = map_cost(map(256, 0).len()).unwrap_or(0);
    assert!(medium * 4 >= large - 16 && medium * 4 <= large + 16);
    assert!(small * 4 >= medium - 16 && small * 4 <= medium + 16);
    assert_eq!(map_cost(10), None);
}

#[test]
fn ids_carry_the_tier_and_large_ids_are_plain_layers() {
    let mut textures = MaterialTextures::default();
    assert_eq!(textures.add(map(1024, 1)), Some(0));
    assert_eq!(textures.add(map(1024, 2)), Some(1));
    assert_eq!(textures.add(map(512, 3)), Some(TIER_STRIDE));
    assert_eq!(textures.add(map(256, 4)), Some(2 * TIER_STRIDE));
    assert_eq!(textures.add(map(256, 5)), Some(2 * TIER_STRIDE + 1));
    assert_eq!(textures.add(map(512, 6)), Some(TIER_STRIDE + 1));
    assert_eq!(textures.len(), 6);
    assert!(!textures.is_empty());
    assert_eq!(textures.add(vec![0; 16]), None, "a wrong size is refused");
    assert_eq!(textures.len(), 6);
}

#[test]
fn the_budget_is_bytes_so_small_maps_fit_in_greater_number() {
    let large_cost = map_cost(map(1024, 0).len()).unwrap_or(1);
    let small_cost = map_cost(map(256, 0).len()).unwrap_or(1);
    let mut textures = MaterialTextures::default();
    let mut large = 0;
    while textures.add(map(1024, 0)).is_some() {
        large += 1;
        assert!(large < 1000, "the budget must stop it");
    }
    assert_eq!(large, BUDGET_BYTES / large_cost);
    assert!(large >= 32, "the old 32 maps still fit ({large})");
    assert!(textures.bytes() <= BUDGET_BYTES);
    // The leftover room still takes small maps, but not as many as an empty budget would.
    let room = BUDGET_BYTES - textures.bytes();
    let mut small = 0;
    while textures.add(map(256, 0)).is_some() {
        small += 1;
    }
    assert_eq!(small, room / small_cost);
    let mut fresh = MaterialTextures::default();
    let mut only_small = 0;
    while fresh.add(map(256, 0)).is_some() {
        only_small += 1;
    }
    assert!(
        only_small > 10 * large,
        "{only_small} small maps against {large} large ones"
    );
    assert_eq!(only_small, BUDGET_BYTES / small_cost);
}

#[test]
fn clearing_forgets_every_map_and_frees_the_budget() {
    let mut textures = MaterialTextures::default();
    while textures.add(map(512, 0)).is_some() {}
    assert!(textures.add(map(512, 0)).is_none(), "full");
    textures.clear();
    assert!(textures.is_empty());
    assert_eq!(textures.bytes(), 0);
    assert_eq!(
        textures.add(map(512, 0)),
        Some(TIER_STRIDE),
        "ids start again"
    );
}

#[test]
fn the_allocated_layer_count_is_never_one_or_a_multiple_of_six() {
    for used in 0..=600 {
        let layers = allocated_layers(used);
        assert!(layers >= used.max(2), "{used} maps fit in {layers} layers");
        assert!(
            layers != 1 && layers % 6 != 0,
            "{used} maps would allocate {layers} layers, which wgpu's GL backend misreads"
        );
        assert!(layers <= used.max(2) + 1, "at most one spare layer");
    }
    assert_eq!(allocated_layers(6), 7);
    assert_eq!(allocated_layers(12), 13);
    assert_eq!(allocated_layers(5), 5);
}

#[test]
fn mip_chains_run_down_to_one_texel() {
    assert_eq!(mip_levels(1024), 11);
    assert_eq!(mip_levels(512), 10);
    assert_eq!(mip_levels(256), 9);
    assert_eq!(mip_levels(1), 1);
}

#[test]
fn downsampling_averages_two_by_two_blocks() {
    let pixels = [
        0, 0, 0, 255, 100, 100, 100, 255, //
        200, 200, 200, 255, 100, 100, 100, 255,
    ];
    let half = downsample(&pixels, 2);
    assert_eq!(half, vec![100, 100, 100, 255]);
}
