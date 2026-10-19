#![expect(
    clippy::cast_precision_loss,
    reason = "the test's random numbers come from 24 bits of a counter, exact in f32"
)]
use super::Frustum;
use bevy_math::{Mat4, Quat, Vec3, proj};
use bevy_transform::components::Transform;
use std::f32::consts::FRAC_PI_2;

/// A camera at the origin looking down -z, 90 degrees across, square, near 1, far 100 (finite, standard depth).
fn standard() -> Mat4 {
    proj::perspective(FRAC_PI_2, 1.0, 1.0, 100.0)
}

/// The same but reverse-Z with an infinite far plane, which is what the engine's cameras use.
fn reverse_infinite() -> Mat4 {
    proj::perspective_infinite_reverse(FRAC_PI_2, 1.0, 1.0)
}

#[test]
fn points_inside_and_outside_a_standard_perspective_frustum() {
    let frustum = Frustum::from_clip_from_world(standard());
    // Straight ahead, at several depths.
    for z in [-1.5, -10.0, -99.0] {
        assert!(
            frustum.intersects_sphere(Vec3::new(0.0, 0.0, z), 0.0),
            "z {z}"
        );
    }
    // Behind the camera, in front of the near plane, past the far plane.
    assert!(!frustum.intersects_sphere(Vec3::new(0.0, 0.0, 5.0), 0.0));
    assert!(!frustum.intersects_sphere(Vec3::new(0.0, 0.0, -0.5), 0.0));
    assert!(!frustum.intersects_sphere(Vec3::new(0.0, 0.0, -101.0), 0.0));
    // At depth 10 the 90 degree frustum is 10 wide each side: 9.9 is in, 10.1 out, on every side.
    for (x, y) in [(9.9, 0.0), (-9.9, 0.0), (0.0, 9.9), (0.0, -9.9)] {
        assert!(
            frustum.intersects_sphere(Vec3::new(x, y, -10.0), 0.0),
            "({x}, {y})"
        );
        assert!(
            !frustum.intersects_sphere(Vec3::new(x * 1.03, y * 1.03, -10.0), 0.0),
            "({x}, {y}) out"
        );
    }
}

#[test]
fn a_sphere_counts_when_it_reaches_into_the_frustum() {
    let frustum = Frustum::from_clip_from_world(standard());
    // Just outside the right side at depth 10 (x = 11), 2 m across: it reaches in; 0.5 m across it does not.
    let outside = Vec3::new(11.0, 0.0, -10.0);
    assert!(frustum.intersects_sphere(outside, 2.0 * std::f32::consts::SQRT_2));
    assert!(!frustum.intersects_sphere(outside, 0.5));
    // Behind the camera but big enough to reach the near plane.
    assert!(frustum.intersects_sphere(Vec3::new(0.0, 0.0, 3.0), 4.5));
    assert!(!frustum.intersects_sphere(Vec3::new(0.0, 0.0, 3.0), 3.0));
}

#[test]
fn reverse_z_with_an_infinite_far_plane_sees_the_same_sides() {
    let frustum = Frustum::from_clip_from_world(reverse_infinite());
    assert!(frustum.intersects_sphere(Vec3::new(0.0, 0.0, -2.0), 0.0));
    assert!(
        frustum.intersects_sphere(Vec3::new(0.0, 0.0, -100_000.0), 0.0),
        "no far plane"
    );
    assert!(
        !frustum.intersects_sphere(Vec3::new(0.0, 0.0, 5.0), 0.0),
        "behind"
    );
    assert!(
        !frustum.intersects_sphere(Vec3::new(0.0, 0.0, -0.5), 0.0),
        "in front of the near plane"
    );
    assert!(!frustum.intersects_sphere(Vec3::new(30.0, 0.0, -10.0), 1.0));
    assert!(frustum.intersects_sphere(Vec3::new(9.0, 0.0, -10.0), 0.0));
}

#[test]
fn a_moved_and_turned_camera_moves_the_frustum_with_it() {
    let projection = reverse_infinite();
    // The camera stands at (100, 0, 0) and looks along +x (turned a quarter turn about y).
    let pose = Mat4::from_rotation_translation(
        Quat::from_rotation_y(-FRAC_PI_2),
        Vec3::new(100.0, 0.0, 0.0),
    );
    let frustum = Frustum::from_clip_from_world(projection * pose.inverse());
    assert!(
        frustum.intersects_sphere(Vec3::new(110.0, 0.0, 0.0), 0.0),
        "ahead along +x"
    );
    assert!(
        !frustum.intersects_sphere(Vec3::new(90.0, 0.0, 0.0), 0.0),
        "behind it"
    );
    assert!(
        !frustum.intersects_sphere(Vec3::new(0.0, 0.0, 0.0), 1.0),
        "the old straight-ahead spot is behind now"
    );
    assert!(frustum.intersects_sphere(Vec3::new(120.0, 5.0, -5.0), 0.0));
}

#[test]
fn an_orthographic_box_has_four_sides_and_sides_only_ignores_depth() {
    // A sun camera: 20 m square, looking down -y from above, depth 0 to 50 below it.
    let projection = proj::orthographic(-10.0, 10.0, -10.0, 10.0, 0.0, 50.0);
    let look = Transform::from_translation(Vec3::new(0.0, 100.0, 0.0))
        .looking_to(Vec3::NEG_Y, Vec3::Z)
        .to_matrix()
        .inverse();
    let full = Frustum::from_clip_from_world(projection * look);
    assert!(
        full.intersects_sphere(Vec3::new(0.0, 80.0, 0.0), 0.0),
        "in the box"
    );
    assert!(
        !full.intersects_sphere(Vec3::new(0.0, 150.0, 0.0), 0.0),
        "between the sun and the box: culled when depth counts"
    );
    assert!(
        !full.intersects_sphere(Vec3::new(0.0, 20.0, 0.0), 0.0),
        "below the box"
    );
    assert!(
        !full.intersects_sphere(Vec3::new(15.0, 80.0, 0.0), 0.0),
        "beside it"
    );
    // Sides only: a caster above (nearer the sun) still counts, so it can shade the box; one beside it still does not.
    let sides = full.sides_only();
    assert!(sides.intersects_sphere(Vec3::new(0.0, 150.0, 0.0), 0.0));
    assert!(sides.intersects_sphere(Vec3::new(0.0, 20.0, 0.0), 0.0));
    assert!(!sides.intersects_sphere(Vec3::new(15.0, 80.0, 0.0), 0.0));
    assert!(
        sides.intersects_sphere(Vec3::new(10.5, 80.0, 0.0), 1.0),
        "a sphere reaching in over the side"
    );
}

#[test]
fn culling_never_rejects_what_a_projection_shows() {
    // Cross-check against the projection itself: every point whose clip coordinates are inside the cube must pass.
    let matrix = reverse_infinite() * Mat4::from_translation(Vec3::new(0.0, -2.0, -3.0));
    let frustum = Frustum::from_clip_from_world(matrix);
    let mut shown = 0;
    let mut seed = 12345_u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1 << 24) as f32
    };
    for _ in 0..20_000 {
        let point = Vec3::new(
            next() * 80.0 - 40.0,
            next() * 40.0 - 20.0,
            next() * -90.0 + 10.0,
        );
        let clip = matrix * point.extend(1.0);
        let inside = clip.w > 0.0
            && clip.x.abs() <= clip.w
            && clip.y.abs() <= clip.w
            && clip.z >= 0.0
            && clip.z <= clip.w;
        if inside {
            shown += 1;
            assert!(
                frustum.intersects_sphere(point, 0.0),
                "{point} is shown but culled"
            );
        } else if clip.w > 0.0 && (clip.x.abs() > clip.w * 1.2 || clip.y.abs() > clip.w * 1.2) {
            assert!(
                !frustum.intersects_sphere(point, 0.0),
                "{point} is far outside but kept"
            );
        }
    }
    assert!(shown > 500, "the sample had {shown} visible points");
}
