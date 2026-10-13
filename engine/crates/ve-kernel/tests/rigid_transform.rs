//! G4d's kernel primitive is independent of socket interface/default decisions.
use ve_kernel::Solid;
#[test]
fn g4d_rigid_transform_preserves_volume_and_round_trips() {
    let solid = Solid::cube([2.0, 1.0, 3.0], true).unwrap();
    let matrix = [0.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0];
    let moved = solid.transform(matrix).unwrap();
    assert!((moved.volume() - 6.0).abs() < 1e-9);
    let bounds = moved.bounds().unwrap();
    assert!(
        bounds
            .size()
            .iter()
            .zip([1.0, 2.0, 3.0])
            .all(|(a, b)| (a - b).abs() < 1e-9)
    );
    let inverse = [
        0.0, 1.0, 0.0, -2.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, -3.0,
    ];
    assert_eq!(
        moved.transform(inverse).unwrap().bounds().unwrap(),
        solid.bounds().unwrap()
    );
}
#[test]
fn g4d_rigid_transform_refuses_scale_reflection_and_nonfinite() {
    let solid = Solid::cube([1.0; 3], true).unwrap();
    let mut matrix = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    matrix[0] = 2.0;
    assert!(solid.transform(matrix).is_err());
    matrix[0] = -1.0;
    assert!(solid.transform(matrix).is_err());
    matrix[0] = f64::NAN;
    assert!(solid.transform(matrix).is_err());
}
