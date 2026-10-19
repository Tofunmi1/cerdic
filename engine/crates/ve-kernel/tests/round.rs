//! B6: bounded Minkowski edge rounding.
use ve_kernel::{KernelError, Profile, Solid};

#[test]
fn convex_box_rounding_preserves_bounds_and_softens_corners() {
    let box_solid = Solid::cube([1.0, 1.0, 1.0], true).expect("box");
    let before = box_solid.volume();
    let rounded = box_solid.round(0.15, 12).expect("rounded box");
    let bounds = rounded.bounds().expect("bounds");
    assert!(
        bounds
            .size()
            .iter()
            .all(|extent| (extent - 1.0).abs() < 1.0e-6)
    );
    assert!(
        rounded.volume() < before,
        "rounding removes the sharp corner wedges"
    );
    assert!(
        rounded.volume() > 0.9,
        "a small radius retains most of the box"
    );
    assert!(rounded.triangle_count() > box_solid.triangle_count());
}

#[test]
fn round_refuses_a_radius_that_consumes_the_core() {
    let box_solid = Solid::cube([0.2, 0.4, 0.6], true).expect("box");
    assert!(matches!(
        box_solid.round(0.1, 12),
        Err(KernelError::InvalidArgument(_))
    ));
    assert!(matches!(
        box_solid.round(0.02, 25),
        Err(KernelError::InvalidArgument(_))
    ));
}

#[test]
fn profile_fillet_rounds_rectangle_corners_at_constant_outer_size() {
    let profile =
        Profile::polygon(&[[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]).expect("rectangle");
    let rounded = profile.fillet(0.2).expect("filleted profile");
    assert!((profile.area() - 4.0).abs() < 1.0e-9);
    assert!((rounded.area() - (4.0 - (4.0 - std::f64::consts::PI) * 0.04)).abs() < 0.002);
    assert_eq!(rounded.loops().expect("loops").len(), 1);
}
