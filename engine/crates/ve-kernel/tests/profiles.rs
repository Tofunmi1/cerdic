//! G3 profile conformance, before these operators are offered to providers.
use ve_kernel::{KernelError, Profile};

#[test]
fn g3_07_rectangle_offset_area_and_empty_erosion() {
    let profile = Profile::polygon(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).unwrap();
    assert!((profile.offset(-0.1).unwrap().area() - 0.64).abs() < 1e-6);
    assert!(profile.offset(-1.0).unwrap().is_empty());
    let hole = profile.offset(-0.1).unwrap();
    assert!((profile.difference(&hole).unwrap().area() - 0.36).abs() < 1e-6);
}

#[test]
fn g3_08_twisted_extrusion_volume_and_y_up_bounds() {
    let profile = Profile::polygon(&[[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]]).unwrap();
    let solid = profile
        .extrude(2.0, 128, std::f64::consts::FRAC_PI_2, [1.0; 2])
        .unwrap();
    assert!(
        (solid.volume() / 2.0 - 1.0).abs() < 1e-3,
        "twisted volume {}",
        solid.volume()
    );
    let bounds = solid.bounds().unwrap();
    assert!(bounds.min[1].abs() < 1e-6);
    assert!((bounds.max[1] - 2.0).abs() < 1e-6);
    assert!((bounds.max[0] - 0.5_f64.sqrt()).abs() < 1e-6);
    let tapered = profile.extrude(2.0, 1, 0.0, [0.5; 2]).unwrap();
    assert!((tapered.volume() - 2.0 * 1.75 / 3.0).abs() < 1e-6);
}

#[test]
fn g3_09_g3_10_revolve_matches_faceted_and_circular_volume() {
    let profile = Profile::polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 3.0], [1.0, 3.0]]).unwrap();
    let full = profile.revolve(64, std::f64::consts::TAU).unwrap();
    let step = std::f64::consts::TAU / 64.0;
    let faceted = 9.0 * 32.0 * step.sin();
    assert!((full.volume() / faceted - 1.0).abs() < 1e-6);
    assert!((full.volume() / (9.0 * std::f64::consts::PI) - 1.0).abs() < 2e-3);
    let accurate = profile.revolve(128, std::f64::consts::TAU).unwrap();
    assert!((accurate.volume() / (9.0 * std::f64::consts::PI) - 1.0).abs() < 1e-3);
    let half = profile.revolve(64, std::f64::consts::PI).unwrap();
    assert!(
        (half.volume() / full.volume() - 0.5).abs() < 1e-6,
        "half {} full {}",
        half.volume(),
        full.volume()
    );
    let bounds = full.bounds().unwrap();
    assert!((bounds.max[1] - 3.0).abs() < 1e-6);
    assert!((bounds.min[0] + 2.0).abs() < 1e-6);
}

#[test]
fn g3_11_g3_12_invalid_profiles_and_zero_height_have_defined_errors() {
    for points in [
        vec![[0.0, 0.0], [1.0, 1.0], [0.0, 1.0], [1.0, 0.0]],
        vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
        vec![[0.0, 0.0], [f64::NAN, 1.0], [1.0, 0.0]],
    ] {
        assert!(Profile::polygon(&points).is_err());
    }
    let profile = Profile::polygon(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]).unwrap();
    assert!(matches!(
        profile.extrude(0.0, 1, 0.0, [1.0; 2]),
        Err(KernelError::InvalidArgument(_))
    ));
    assert!(profile.revolve(513, std::f64::consts::TAU).is_err());
    assert!(profile.extrude(1.0, 257, 0.0, [1.0; 2]).is_err());
    let loops = profile.loops().unwrap();
    assert!((Profile::from_loops(&loops).unwrap().area() - 1.0).abs() < 1e-6);
    assert!(Profile::polygon(&vec![[0.0, 0.0]; 4097]).is_err());
    let negative_radius =
        Profile::polygon(&[[-1.0, 0.0], [1.0, 0.0], [1.0, 1.0], [-1.0, 1.0]]).unwrap();
    assert!(negative_radius.revolve(64, std::f64::consts::TAU).is_err());
    let empty = Profile::from_loops(&[]).unwrap();
    assert!(empty.is_empty());
    assert!(empty.extrude(1.0, 1, 0.0, [1.0; 2]).unwrap().is_empty());
}
