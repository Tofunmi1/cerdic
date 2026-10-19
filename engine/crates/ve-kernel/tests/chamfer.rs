//! B6: bounded octahedral edge chamfering.
use ve_kernel::{KernelError, Solid};

#[test]
fn chamfering_a_box_preserves_bounds_and_cuts_more_than_rounding() {
    let box_solid = Solid::cube([1.0, 1.0, 1.0], true).expect("box");
    let before = box_solid.volume();
    let width = 0.15;
    let chamfered = box_solid.chamfer(width).expect("chamfered box");
    let rounded = box_solid.round(width, 12).expect("rounded box");
    let bounds = chamfered.bounds().expect("bounds");
    assert!(
        bounds
            .size()
            .iter()
            .all(|extent| (extent - 1.0).abs() < 1.0e-6),
        "the chamfer keeps the bounding box: {:?}",
        bounds.size()
    );
    assert!(
        chamfered.volume() < before,
        "the chamfer removes sharp material"
    );
    assert!(
        chamfered.volume() > 0.8,
        "a small chamfer retains most of the box, got {}",
        chamfered.volume()
    );
    assert!(
        chamfered.volume() < rounded.volume(),
        "a flat chamfer removes more than a round of the same size: {} vs {}",
        chamfered.volume(),
        rounded.volume()
    );
    assert!(chamfered.triangle_count() > box_solid.triangle_count());
}

#[test]
fn the_same_chamfer_twice_is_identical() {
    let box_solid = Solid::cube([1.2, 1.0, 0.8], true).expect("box");
    let a = box_solid.chamfer(0.1).expect("first");
    let b = box_solid.chamfer(0.1).expect("second");
    assert_eq!(a.triangle_count(), b.triangle_count());
    assert!((a.volume() - b.volume()).abs() < 1.0e-12);
}

#[test]
fn chamfer_refuses_a_width_that_consumes_the_core() {
    let box_solid = Solid::cube([0.2, 0.4, 0.6], true).expect("box");
    assert!(matches!(
        box_solid.chamfer(0.1),
        Err(KernelError::InvalidArgument(_))
    ));
    assert!(matches!(
        box_solid.chamfer(0.0),
        Err(KernelError::InvalidArgument(_))
    ));
    assert!(matches!(
        box_solid.chamfer(-0.1),
        Err(KernelError::InvalidArgument(_))
    ));
    assert!(matches!(
        box_solid.chamfer(f64::NAN),
        Err(KernelError::InvalidArgument(_))
    ));
}
