#![expect(
    clippy::cast_precision_loss,
    reason = "the fixtures' counts and random numbers are small whole numbers, exact in f32"
)]
use super::{build, chunk_size_for, morton, spread};
use bevy_math::Vec3;

/// A scatter of `count` unit-radius spheres over a 400 by 400 field, from a fixed seed.
fn field(count: usize) -> Vec<(Vec3, f32)> {
    let mut seed = 7_u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1 << 24) as f32
    };
    (0..count)
        .map(|_| (Vec3::new(next() * 400.0, next() * 3.0, next() * 400.0), 1.0))
        .collect()
}

#[test]
fn the_chunk_size_follows_the_set_size_within_bounds() {
    assert_eq!(chunk_size_for(10), 32);
    assert_eq!(chunk_size_for(1000), 32);
    assert_eq!(chunk_size_for(16_000), 333);
    assert_eq!(chunk_size_for(1_000_000), 512);
    for count in [0, 1, 500, 5_000, 50_000, 262_144] {
        let size = chunk_size_for(count);
        assert!((32..=512).contains(&size));
    }
}

#[test]
fn bits_are_spread_two_apart() {
    assert_eq!(spread(0), 0);
    assert_eq!(spread(1), 1);
    assert_eq!(spread(0b11), 0b1001);
    assert_eq!(spread(0b111), 0b100_1001);
    assert_eq!(spread(0x3ff), 0x0924_9249);
    // Bits above the tenth are ignored.
    assert_eq!(spread(0x400), 0);
}

#[test]
fn a_morton_code_orders_points_along_the_z_curve() {
    let (low, size) = (Vec3::ZERO, Vec3::splat(10.0));
    assert_eq!(morton(Vec3::ZERO, low, size), 0);
    assert_eq!(morton(Vec3::splat(10.0), low, size), 0x3fff_ffff);
    // Nearer points in the same octant share high bits: two points in the lowest octant sort before one in the highest.
    let near_a = morton(Vec3::new(0.1, 0.1, 0.1), low, size);
    let near_b = morton(Vec3::new(0.3, 0.2, 0.1), low, size);
    let far = morton(Vec3::new(9.0, 9.0, 9.0), low, size);
    assert!(near_a < far && near_b < far);
    // A flat set (zero size on an axis) does not divide by zero.
    assert_eq!(
        morton(
            Vec3::new(5.0, 1.0, 5.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(10.0, 0.0, 10.0)
        ) & 0b010_010_010_010_010_010_010_010_010_010,
        0
    );
}

#[test]
fn every_copy_lands_in_exactly_one_chunk_and_the_order_is_a_permutation() {
    let spheres = field(1000);
    let (order, chunks) = build(&spheres, 64);
    let mut seen = order.clone();
    seen.sort_unstable();
    assert_eq!(
        seen,
        (0..1000).collect::<Vec<u32>>(),
        "a permutation of the copies"
    );
    assert_eq!(chunks.len(), 16, "1000 copies in chunks of 64");
    let mut next = 0;
    for chunk in &chunks {
        assert_eq!(
            chunk.range.start, next,
            "chunks are consecutive and gapless"
        );
        assert!(chunk.range.end > chunk.range.start && chunk.range.len() <= 64);
        next = chunk.range.end;
    }
    assert_eq!(next, 1000);
}

#[test]
fn a_chunk_sphere_holds_every_copy_in_it() {
    let spheres = field(2000);
    let (order, chunks) = build(&spheres, 50);
    for chunk in &chunks {
        for &index in &order[chunk.range.start as usize..chunk.range.end as usize] {
            let (centre, radius) = spheres[index as usize];
            assert!(
                chunk.centre.distance(centre) + radius <= chunk.radius + 1e-3,
                "copy {index} sticks out of its chunk"
            );
        }
    }
}

#[test]
fn chunks_are_spatially_compact_much_smaller_than_the_set() {
    let spheres = field(16_000);
    let size = super::chunk_size_for(spheres.len());
    let (_, chunks) = build(&spheres, size);
    let set_radius = 400.0 * std::f32::consts::FRAC_1_SQRT_2;
    let mean = chunks.iter().map(|c| c.radius).sum::<f32>() / chunks.len() as f32;
    assert!(
        mean < 0.5 * set_radius,
        "chunks average {mean} m against a set of {set_radius} m"
    );
    // A random split into the same chunk count would put copies from all over the field in each chunk.
    let random_mean = {
        let mut order: Vec<u32> = (0..16_000).collect();
        let mut seed = 99_u32;
        for i in (1..order.len()).rev() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            order.swap(i, (seed >> 8) as usize % (i + 1));
        }
        let radii: Vec<f32> = order
            .chunks(size)
            .map(|members| {
                let centre = members.iter().map(|&i| spheres[i as usize].0).sum::<Vec3>()
                    / members.len() as f32;
                members
                    .iter()
                    .map(|&i| spheres[i as usize].0.distance(centre))
                    .fold(0.0, f32::max)
            })
            .collect();
        radii.iter().sum::<f32>() / radii.len() as f32
    };
    assert!(
        mean < 0.5 * random_mean,
        "sorted {mean} m against unsorted {random_mean} m"
    );
}

#[test]
fn building_is_deterministic_and_handles_the_edges() {
    let spheres = field(500);
    assert_eq!(build(&spheres, 40), build(&spheres, 40));
    assert_eq!(build(&[], 10), (Vec::new(), Vec::new()));
    let one = build(&[(Vec3::new(3.0, 2.0, 1.0), 0.5)], 10);
    assert_eq!(one.0, vec![0]);
    assert_eq!(one.1.len(), 1);
    assert_eq!(one.1[0].centre, Vec3::new(3.0, 2.0, 1.0));
    assert!((one.1[0].radius - 0.5).abs() < 1e-6);
    // Identical centres (a stack of copies) do not break the sort or the sphere.
    let stack = vec![(Vec3::ONE, 2.0); 100];
    let (order, chunks) = build(&stack, 30);
    assert_eq!(
        order,
        (0..100).collect::<Vec<u32>>(),
        "ties keep input order"
    );
    assert!(chunks.iter().all(|c| (c.radius - 2.0).abs() < 1e-5));
    // A chunk size of zero is treated as one.
    assert_eq!(build(&spheres[..5], 0).1.len(), 5);
}
