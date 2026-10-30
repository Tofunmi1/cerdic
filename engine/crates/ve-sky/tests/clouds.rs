//! Fixed expectations for the cloud layer. The hash oracle is a literal port of the shader's `u32` pair arithmetic.
#![expect(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    reason = "exact corner values, small integer sample indices and short names in the shader port"
)]
use ve_sky::{cloud_amount, hash_cell, noise2};

// ---- the shader's hash, as written in sky.wgsl (u64 as [low, high] u32 pairs) ----
fn mul32_high(a: u32, b: u32) -> u32 {
    let (a0, a1) = (a & 0xffff, a >> 16);
    let (b0, b1) = (b & 0xffff, b >> 16);
    let p0 = a0.wrapping_mul(b0);
    let t1 = a1.wrapping_mul(b0).wrapping_add(p0 >> 16);
    let t2 = a0.wrapping_mul(b1).wrapping_add(t1 & 0xffff);
    a1.wrapping_mul(b1)
        .wrapping_add(t1 >> 16)
        .wrapping_add(t2 >> 16)
}
fn mul64(a: [u32; 2], b: [u32; 2]) -> [u32; 2] {
    let low = a[0].wrapping_mul(b[0]);
    let high = mul32_high(a[0], b[0])
        .wrapping_add(a[1].wrapping_mul(b[0]))
        .wrapping_add(a[0].wrapping_mul(b[1]));
    [low, high]
}
fn shr64(v: [u32; 2], bits: u32) -> [u32; 2] {
    [(v[0] >> bits) | (v[1] << (32 - bits)), v[1] >> bits]
}
fn xor(a: [u32; 2], b: [u32; 2]) -> [u32; 2] {
    [a[0] ^ b[0], a[1] ^ b[1]]
}
fn shader_hash_cell(cell: [i32; 3], seed: u32) -> u32 {
    let mut hash = mul64([seed, 0], [0x7f4a_7c15, 0x9e37_79b9]);
    for c in cell {
        hash = xor(hash, [c as u32, if c >= 0 { 0 } else { 0xffff_ffff }]);
        hash = mul64(hash, [0x1ce4_e5b9, 0xbf58_476d]);
        hash = xor(hash, shr64(hash, 31));
    }
    hash = mul64(hash, [0x1331_11eb, 0x94d0_49bb]);
    hash = xor(hash, shr64(hash, 29));
    hash[1] >> 8
}

#[test]
fn the_hash_matches_the_shaders_u32_pair_arithmetic() {
    for seed in [0_u32, 1, 13, 101, 999_999, 0xdead_beef] {
        for x in [-70_000, -5, -1, 0, 1, 2, 77, 1_000_003] {
            for y in [-9, 0, 3, 123_456] {
                for z in [0, 1, -1] {
                    assert_eq!(
                        hash_cell([x, y, z], seed),
                        shader_hash_cell([x, y, z], seed),
                        "cell [{x}, {y}, {z}] seed {seed}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_hash_has_24_bits_and_depends_on_every_input() {
    let base = hash_cell([3, 4, 0], 7);
    assert!(base < 1 << 24);
    assert_ne!(base, hash_cell([4, 4, 0], 7));
    assert_ne!(base, hash_cell([3, 5, 0], 7));
    assert_ne!(base, hash_cell([3, 4, 1], 7));
    assert_ne!(base, hash_cell([3, 4, 0], 8));
}

#[test]
fn noise_is_the_corner_value_at_integer_points() {
    for (x, y) in [(0, 0), (3, -2), (-7, 5)] {
        let corner = hash_cell([x, y, 0], 11) as f32 / 16_777_216.0;
        assert_eq!(noise2([x as f32, y as f32], 11), corner, "at ({x}, {y})");
    }
}

#[test]
fn noise_blends_the_four_corners_with_the_smooth_weights() {
    let corner = |x: i32, y: i32| hash_cell([x, y, 0], 5) as f32 / 16_777_216.0;
    // At f = (0.5, 0.5) the weights are 0.5, so the value is the plain average of the corners.
    let average = (corner(2, 3) + corner(3, 3) + corner(2, 4) + corner(3, 4)) / 4.0;
    assert!((noise2([2.5, 3.5], 5) - average).abs() < 1e-6);
    // At f = (0.25, 0): w = 0.25^2 * (3 - 0.5) = 0.15625 along x, y weight 0.
    let w = 0.156_25_f32;
    let expected = corner(2, 3) * (1.0 - w) + corner(3, 3) * w;
    assert!((noise2([2.25, 3.0], 5) - expected).abs() < 1e-6);
}

#[test]
fn noise_stays_in_range_and_varies() {
    let values: Vec<f32> = (0..4000)
        .map(|i| noise2([(i as f32) * 0.37 - 400.0, (i as f32) * 0.113 + 5.0], 3))
        .collect();
    assert!(values.iter().all(|v| (0.0..1.0).contains(v)));
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    assert!((0.4..0.6).contains(&mean), "mean {mean}");
    assert!(values.iter().any(|v| *v < 0.2) && values.iter().any(|v| *v > 0.8));
}

/// Directions over the upper hemisphere (y from 0.1 to 1), spiralling by the golden angle.
fn sky_directions(count: usize) -> Vec<[f32; 3]> {
    (0..count)
        .map(|i| {
            let y = 0.1 + 0.9 * (i as f32 + 0.5) / count as f32;
            let r = (1.0 - y * y).sqrt();
            let angle = i as f32 * 2.399_963;
            [r * angle.cos(), y, r * angle.sin()]
        })
        .collect()
}

fn mean_cloud(cover: f32, seed: u32) -> f32 {
    let directions = sky_directions(3000);
    directions
        .iter()
        .map(|d| cloud_amount(*d, cover, seed))
        .sum::<f32>()
        / directions.len() as f32
}

#[test]
fn there_is_no_cloud_at_or_below_the_horizon() {
    assert_eq!(cloud_amount([1.0, 0.0, 0.0], 1.0, 3), 0.0);
    assert_eq!(cloud_amount([0.6, -0.2, 0.7], 1.0, 3), 0.0);
    assert_eq!(cloud_amount([0.0, -1.0, 0.0], 1.0, 3), 0.0);
}

#[test]
fn the_amount_is_between_zero_and_seventy_eight_percent() {
    for cover in [0.0, 0.3, 0.7, 1.0] {
        for d in sky_directions(500) {
            let a = cloud_amount(d, cover, 9);
            assert!((0.0..=0.78 + 1e-6).contains(&a), "cover {cover}: {a}");
        }
    }
}

#[test]
fn more_cover_means_more_cloud() {
    let (clear, half, full) = (
        mean_cloud(0.0, 13),
        mean_cloud(0.5, 13),
        mean_cloud(1.0, 13),
    );
    assert!(clear < 0.05, "cover 0 should be nearly clear: {clear}");
    assert!(
        half > clear && full > half,
        "clear {clear} half {half} full {full}"
    );
    assert!(full > 0.3, "cover 1 should be mostly cloud: {full}");
}

#[test]
fn the_seed_changes_the_pattern_and_the_same_seed_repeats_it() {
    let directions = sky_directions(400);
    let a: Vec<f32> = directions
        .iter()
        .map(|d| cloud_amount(*d, 0.6, 1))
        .collect();
    let b: Vec<f32> = directions
        .iter()
        .map(|d| cloud_amount(*d, 0.6, 1))
        .collect();
    let c: Vec<f32> = directions
        .iter()
        .map(|d| cloud_amount(*d, 0.6, 2))
        .collect();
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn the_pattern_is_smooth_between_neighbouring_directions() {
    // Clouds are soft: a step of 0.001 rad never jumps by more than a few percent.
    for d in sky_directions(300) {
        let a = cloud_amount(d, 0.6, 4);
        let tilted = [d[0] + 0.001, d[1], d[2]];
        let n = (tilted[0] * tilted[0] + tilted[1] * tilted[1] + tilted[2] * tilted[2]).sqrt();
        let b = cloud_amount([tilted[0] / n, tilted[1] / n, tilted[2] / n], 0.6, 4);
        assert!((a - b).abs() < 0.08, "{a} against {b}");
    }
}
