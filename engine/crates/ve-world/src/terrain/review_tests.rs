use super::{HeightGrid, Mask, Noise, compute_mask, generate};

fn fnv1a(values: &[f32]) -> u64 {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

#[test]
fn generate_hash_is_stable() {
    let mut hashes = Vec::new();
    for seed in [1, 42, 0xdead_beef] {
        let mut grid = HeightGrid::new(129, 129, 1.0, [0.0, 0.0]).unwrap();
        let noise = Noise {
            seed,
            scale: 37.0,
            octaves: 5,
            roughness: 0.52,
            lacunarity: 2.0,
            ridged: false,
            warp: 0.0,
            basis: None,
        };
        generate(&mut grid, &noise, 120.0, &|| false).unwrap();
        hashes.push(fnv1a(grid.heights()));
    }
    assert_eq!(
        hashes,
        [
            10_564_738_071_198_853_707,
            3_173_558_287_248_542_844,
            10_110_277_755_925_135_599,
        ]
    );
}

#[test]
fn slope_and_occlusion_hashes_are_stable() {
    let mut grid = HeightGrid::new(129, 129, 1.0, [0.0, 0.0]).unwrap();
    let noise = Noise {
        seed: 42,
        scale: 37.0,
        octaves: 5,
        roughness: 0.52,
        lacunarity: 2.0,
        ridged: false,
        warp: 0.0,
        basis: None,
    };
    generate(&mut grid, &noise, 120.0, &|| false).unwrap();
    let slope = compute_mask(&grid, Mask::Slope, &|| false).unwrap();
    let occlusion = compute_mask(&grid, Mask::Occlusion, &|| false).unwrap();
    assert_eq!(
        [fnv1a(&slope), fnv1a(&occlusion)],
        [1_501_502_308_323_042_419, 7_176_336_250_419_348_776]
    );
}

#[test]
fn release_performance_measurements() {
    use std::time::Instant;

    let mut grid = HeightGrid::new(1025, 1025, 1.0, [0.0, 0.0]).unwrap();
    let noise = Noise {
        seed: 42,
        scale: 256.0,
        octaves: 5,
        roughness: 0.52,
        lacunarity: 2.0,
        ridged: false,
        warp: 0.0,
        basis: None,
    };
    let start = Instant::now();
    generate(&mut grid, &noise, 120.0, &|| false).unwrap();
    let generate_time = start.elapsed();

    let start = Instant::now();
    let mut occlusion_time = std::time::Duration::ZERO;
    for mask in [
        Mask::Slope,
        Mask::Curvature,
        Mask::Height,
        Mask::Aspect,
        Mask::Occlusion,
    ] {
        let mask_start = Instant::now();
        let values = compute_mask(&grid, mask, &|| false).unwrap();
        std::hint::black_box(values);
        if mask == Mask::Occlusion {
            occlusion_time = mask_start.elapsed();
        }
    }
    let masks_time = start.elapsed();

    let start = Instant::now();
    let mesh = super::to_mesh(&grid, 1);
    std::hint::black_box(mesh);
    let mesh_time = start.elapsed();
    eprintln!(
        "generate={generate_time:?} all_masks={masks_time:?} occlusion={occlusion_time:?} mesh={mesh_time:?}"
    );
}
