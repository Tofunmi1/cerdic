//! Exact-ray tests for V3 gizmo constraints and snapping.

use glam::Vec2;
use glam::Vec3;
use ve_interact::{
    CameraView, GizmoConstraint as Constraint, GizmoDelta as Delta, GizmoDrag, GizmoMode as Mode,
    GizmoSnap as Snap, Ray,
};

fn ray(origin: Vec3, direction: Vec3) -> Ray {
    Ray {
        origin,
        direction: direction.normalize(),
    }
}

#[test]
fn axis_translation_is_snapped_in_metres() {
    let constraint = Constraint::Axis {
        origin: Vec3::ZERO,
        direction: Vec3::X,
    };
    let mut drag = GizmoDrag::begin(
        Mode::Translate,
        constraint,
        &ray(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Y),
    )
    .expect("axis sample");
    let result = drag.update(
        &ray(Vec3::new(1.13, 1.0, 0.0), Vec3::NEG_Y),
        Snap {
            grid: Some(0.25),
            ..Snap::default()
        },
    );
    assert_eq!(result, Some(Delta::Translation(Vec3::X * 1.25)));
}

#[test]
fn plane_drag_snaps_each_world_coordinate() {
    let constraint = Constraint::Plane {
        origin: Vec3::ZERO,
        normal: Vec3::Z,
    };
    let mut drag = GizmoDrag::begin(
        Mode::Translate,
        constraint,
        &ray(Vec3::new(0.0, 0.0, 2.0), Vec3::NEG_Z),
    )
    .expect("plane sample");
    let result = drag.update(
        &ray(Vec3::new(0.62, 0.37, 2.0), Vec3::NEG_Z),
        Snap {
            grid: Some(0.25),
            ..Snap::default()
        },
    );
    assert_eq!(result, Some(Delta::Translation(Vec3::new(0.5, 0.25, 0.0))));
}

#[test]
fn ring_rotation_snaps_and_accumulates_past_angle_wrap() {
    let constraint = Constraint::Ring {
        center: Vec3::ZERO,
        normal: Vec3::Z,
    };
    let point_ray = |angle: f32| ray(Vec3::new(angle.cos(), angle.sin(), 2.0), Vec3::NEG_Z);
    let mut drag = GizmoDrag::begin(Mode::Rotate, constraint, &point_ray(3.0)).expect("ring start");
    let first = drag
        .update(&point_ray(-3.0), Snap::default())
        .expect("first update");
    let second = drag
        .update(
            &point_ray(-2.8),
            Snap {
                angle: Some(0.1),
                ..Snap::default()
            },
        )
        .expect("second update");
    let Delta::Rotation { axis, radians } = first else {
        panic!("rotation delta expected")
    };
    assert!(axis.abs_diff_eq(Vec3::Z, 1.0e-6));
    assert!((radians - (2.0 * std::f32::consts::PI - 6.0)).abs() < 1.0e-5);
    let Delta::Rotation { radians, .. } = second else {
        panic!("rotation delta expected")
    };
    assert!((radians - 0.5).abs() < 1.0e-5);
}

#[test]
fn degenerate_rays_do_not_start_or_corrupt_a_drag() {
    let parallel = ray(Vec3::ZERO, Vec3::X);
    let axis = Constraint::Axis {
        origin: Vec3::ZERO,
        direction: Vec3::X,
    };
    assert!(GizmoDrag::begin(Mode::Translate, axis, &parallel).is_none());
    let plane = Constraint::Plane {
        origin: Vec3::ZERO,
        normal: Vec3::Z,
    };
    let mut drag =
        GizmoDrag::begin(Mode::Translate, plane, &ray(Vec3::Z, Vec3::NEG_Z)).expect("plane start");
    assert!(drag.update(&parallel, Snap::default()).is_none());
    assert_eq!(
        drag.update(&ray(Vec3::X, Vec3::NEG_Z), Snap::default()),
        Some(Delta::Translation(Vec3::X))
    );
}

#[test]
fn scale_is_positive_and_can_snap() {
    let constraint = Constraint::Axis {
        origin: Vec3::ZERO,
        direction: Vec3::Y,
    };
    let mut drag = GizmoDrag::begin(
        Mode::Scale,
        constraint,
        &ray(Vec3::new(2.0, 0.0, 0.0), Vec3::NEG_X),
    )
    .expect("axis start");
    let result = drag.update(
        &ray(Vec3::new(2.0, 0.51, 0.0), Vec3::NEG_X),
        Snap {
            scale: Some(0.25),
            ..Snap::default()
        },
    );
    assert_eq!(
        result,
        Some(Delta::Scale {
            axis: Vec3::Y,
            factor: 1.5
        })
    );
    let minimum = drag.update(
        &ray(Vec3::new(2.0, -10.0, 0.0), Vec3::NEG_X),
        Snap::default(),
    );
    assert_eq!(
        minimum,
        Some(Delta::Scale {
            axis: Vec3::Y,
            factor: 1.0e-4
        })
    );
}

#[test]
fn camera_projection_and_pointer_rays_are_inverse() {
    let view = CameraView {
        position: Vec3::new(0.0, 0.0, 3.0),
        rotation: glam::Quat::IDENTITY,
        vertical_fov: std::f32::consts::FRAC_PI_2,
    };
    let point = Vec3::new(1.0, 0.5, 0.0);
    let projected = view
        .project(point, 2.0)
        .expect("point is in front of the view");
    let projected_xy = Vec2::new(projected.x, projected.y);
    assert!(projected_xy.abs_diff_eq(Vec2::splat(1.0 / 6.0), 1.0e-6));
    let ray = view.ray(projected_xy, 2.0).expect("camera ray is valid");
    assert!(ray.direction.dot((point - view.position).normalize()) > 1.0 - 1.0e-6);
    assert!(
        view.project(Vec3::new(0.0, 0.0, 4.0), 2.0).is_none(),
        "points behind the camera are hidden"
    );
}
