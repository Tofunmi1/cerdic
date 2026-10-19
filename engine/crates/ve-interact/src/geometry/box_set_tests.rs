#![expect(
    clippy::cast_precision_loss,
    reason = "the test's random numbers come from 24 bits of a counter, exact in f32"
)]
use super::BoxSet;
use crate::{Ray, ray_affine_box};
use glam::{Mat4, Quat, Vec3};
use std::sync::Arc;

/// A deterministic spread of placed, turned and scaled copies over a 100 by 100 field.
fn copies(count: usize) -> Vec<Mat4> {
    let mut seed = 31_u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1 << 24) as f32
    };
    (0..count)
        .map(|_| {
            Mat4::from_scale_rotation_translation(
                Vec3::new(0.5 + next(), 0.5 + 2.0 * next(), 0.5 + next()),
                Quat::from_rotation_y(next() * std::f32::consts::TAU),
                Vec3::new(next() * 100.0, 0.0, next() * 100.0),
            )
        })
        .collect()
}

const MIN: Vec3 = Vec3::new(-0.5, 0.0, -0.5);
const MAX: Vec3 = Vec3::new(0.5, 2.0, 0.5);

fn brute_force(ray: &Ray, matrices: &[Mat4]) -> Option<(f32, usize)> {
    matrices
        .iter()
        .enumerate()
        .filter_map(|(i, m)| ray_affine_box(ray, MIN, MAX, *m).map(|d| (d, i)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

#[test]
fn picking_finds_exactly_what_testing_every_copy_would() {
    let matrices = copies(3000);
    let set = BoxSet::new(Arc::from(matrices.clone()), MIN, MAX);
    assert_eq!(set.len(), 3000);
    let mut hits = 0;
    let mut seed = 5_u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1 << 24) as f32
    };
    for _ in 0..400 {
        // Rays from above the field aimed at random points of it, and some from the side.
        let target = Vec3::new(next() * 100.0, next() * 2.0, next() * 100.0);
        let origin = if next() < 0.5 {
            Vec3::new(next() * 100.0, 30.0, next() * 100.0)
        } else {
            Vec3::new(-20.0, 1.0 + next(), next() * 100.0)
        };
        let ray = Ray::new(origin, target - origin).unwrap();
        let fast = set.pick(&ray);
        let exact = brute_force(&ray, &matrices);
        match (fast, exact) {
            (Some((a, i)), Some((b, j))) => {
                assert!((a - b).abs() < 1e-4, "distance {a} against {b}");
                assert_eq!(i, j, "the same copy");
                hits += 1;
            }
            (None, None) => {}
            other => panic!("disagree: {other:?}"),
        }
    }
    assert!(hits > 40, "the sample hit {hits} times");
}

#[test]
fn a_ray_that_misses_everything_finds_nothing_and_an_empty_set_is_safe() {
    let set = BoxSet::new(Arc::from(copies(500)), MIN, MAX);
    let sky = Ray::new(Vec3::new(50.0, 10.0, 50.0), Vec3::Y).unwrap();
    assert_eq!(set.pick(&sky), None);
    let empty = BoxSet::new(Arc::from(Vec::<Mat4>::new()), MIN, MAX);
    assert!(empty.is_empty());
    assert_eq!(empty.pick(&sky), None);
    assert_eq!(empty.world_bounds(), None);
}

#[test]
fn the_nearer_of_two_copies_in_line_wins_and_is_named() {
    let matrices = vec![
        Mat4::from_translation(Vec3::new(0.0, 0.0, -20.0)),
        Mat4::from_translation(Vec3::new(0.0, 0.0, -10.0)),
        Mat4::from_translation(Vec3::new(8.0, 0.0, -5.0)),
    ];
    let set = BoxSet::new(Arc::from(matrices), MIN, MAX);
    let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z).unwrap();
    let (distance, index) = set.pick(&ray).unwrap();
    assert_eq!(index, 1);
    assert!(
        (distance - 9.5).abs() < 1e-4,
        "the near face of the box at -10 is at 9.5"
    );
}

#[test]
fn copies_behind_the_ray_and_invalid_matrices_are_never_picked() {
    let mut matrices = vec![Mat4::from_translation(Vec3::new(0.0, 0.0, 10.0))];
    matrices.push(
        Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0))
            * Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0)),
    );
    matrices.push(Mat4::from_translation(Vec3::new(f32::NAN, 0.0, -5.0)));
    let set = BoxSet::new(Arc::from(matrices), MIN, MAX);
    let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z).unwrap();
    assert_eq!(
        set.pick(&ray),
        None,
        "behind, mirrored and non-finite copies"
    );
}

#[test]
fn the_world_bounds_hold_every_copy() {
    let matrices = vec![
        Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
        Mat4::from_translation(Vec3::new(-4.0, 5.0, 3.0)),
        Mat4::from_translation(Vec3::new(f32::NAN, 0.0, 0.0)),
    ];
    let set = BoxSet::new(Arc::from(matrices), MIN, MAX);
    let (low, high) = set.world_bounds().unwrap();
    assert!(low.abs_diff_eq(Vec3::new(-4.5, 0.0, -0.5), 1e-5), "{low}");
    assert!(high.abs_diff_eq(Vec3::new(10.5, 7.0, 3.5), 1e-5), "{high}");
}

#[test]
fn a_sheared_copy_is_still_found_inside_its_sphere() {
    let mut matrix = Mat4::from_translation(Vec3::new(0.0, 0.0, -10.0));
    matrix.y_axis.x = 1.5;
    let set = BoxSet::new(Arc::from(vec![matrix]), MIN, MAX);
    // The top of the sheared box leans to +x by 3 at height 2: a ray there still hits it.
    let ray = Ray::new(Vec3::new(3.0, 1.95, 0.0), Vec3::NEG_Z).unwrap();
    assert!(set.pick(&ray).is_some());
    assert_eq!(set.pick(&ray).map(|(_, i)| i), Some(0));
}
