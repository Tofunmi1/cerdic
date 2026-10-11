//! Behavioural tests for the safe kernel API (native). The same corpus runs on wasm in `cargo xtask parity`.

use std::f64::consts::PI;

use ve_kernel::{KernelError, Mesh, Solid, simplify};

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn cube_basics() {
    let cube = Solid::cube([1.0, 1.0, 1.0], true).unwrap();
    assert!(close(cube.volume(), 1.0, 1e-9));
    assert_eq!(cube.triangle_count(), 12);
    assert_eq!(cube.component_count(), 1);
    assert!(!cube.is_empty());
    let bounds = cube.bounds().unwrap();
    assert!(bounds.size().iter().all(|s| close(*s, 1.0, 1e-9)));
    assert!(bounds.min.iter().all(|m| close(*m, -0.5, 1e-9)));
}

#[test]
fn sphere_volume_is_close_to_exact() {
    let sphere = Solid::sphere(0.65, 48).unwrap();
    let exact = 4.0 / 3.0 * PI * 0.65_f64.powi(3);
    assert!(close(sphere.volume(), exact, exact * 0.02));
}

#[test]
fn cylinder_volume_and_cone() {
    let cylinder = Solid::cylinder(2.0, 0.5, 0.5, 96, false).unwrap();
    assert!(close(cylinder.volume(), PI * 0.25 * 2.0, 0.02));
    let cone = Solid::cylinder(2.0, 0.5, 0.0, 96, false).unwrap();
    assert!(close(cone.volume(), PI * 0.25 * 2.0 / 3.0, 0.02));
}

#[test]
fn booleans_obey_set_algebra() {
    let a = Solid::cube([2.0, 2.0, 2.0], true).unwrap();
    let b = a.translate([1.0, 0.0, 0.0]).unwrap();
    let union = a.union(&b).unwrap();
    let inter = a.intersect(&b).unwrap();
    let diff = a.difference(&b).unwrap();
    assert!(close(union.volume(), 12.0, 1e-6));
    assert!(close(inter.volume(), 4.0, 1e-6));
    assert!(close(diff.volume(), 4.0, 1e-6));
    assert!(close(
        union.volume() + inter.volume(),
        a.volume() + b.volume(),
        1e-6
    ));
}

#[test]
fn disjoint_intersection_is_empty_not_an_error() {
    let a = Solid::cube([1.0, 1.0, 1.0], false).unwrap();
    let far = a.translate([10.0, 0.0, 0.0]).unwrap();
    let inter = a.intersect(&far).unwrap();
    assert!(inter.is_empty());
    assert_eq!(inter.to_mesh().unwrap().triangle_count(), 0);
}

#[test]
fn transforms_change_what_they_should() {
    let cube = Solid::cube([1.0, 2.0, 3.0], false).unwrap();
    assert!(close(
        cube.translate([5.0, 5.0, 5.0]).unwrap().volume(),
        6.0,
        1e-9
    ));
    assert!(close(
        cube.rotate([0.0, 90.0, 0.0]).unwrap().volume(),
        6.0,
        1e-6
    ));
    assert!(close(
        cube.scale([2.0, 2.0, 2.0]).unwrap().volume(),
        48.0,
        1e-6
    ));
    let sized = cube
        .rotate([0.0, 90.0, 0.0])
        .unwrap()
        .bounds()
        .unwrap()
        .size();
    assert!(close(sized[0], 3.0, 1e-6) && close(sized[2], 1.0, 1e-6));
}

#[test]
fn mesh_round_trip_preserves_volume() {
    let diff = Solid::cube([1.0, 1.0, 1.0], true)
        .unwrap()
        .difference(&Solid::sphere(0.65, 48).unwrap())
        .unwrap();
    let mesh = diff.to_mesh().unwrap();
    assert_eq!(mesh.triangle_count(), diff.triangle_count());
    let rebuilt = Solid::from_mesh(&mesh).unwrap();
    assert!(close(rebuilt.volume(), diff.volume(), 1e-4));
}

#[test]
fn simplify_reduces_triangles() {
    let mesh = Solid::sphere(1.0, 64).unwrap().to_mesh().unwrap();
    let simple = simplify(&mesh, 0.25, 0.05).unwrap();
    assert!(!simple.is_empty());
    assert!(simple.triangle_count() < mesh.triangle_count());
}

#[test]
fn rejects_invalid_arguments_with_typed_errors() {
    let invalid = |r: Result<Solid, KernelError>| matches!(r, Err(KernelError::InvalidArgument(_)));
    assert!(invalid(Solid::cube([-1.0, 1.0, 1.0], false)));
    assert!(invalid(Solid::cube([f64::NAN, 1.0, 1.0], false)));
    assert!(invalid(Solid::cube([0.0, 1.0, 1.0], false)));
    assert!(invalid(Solid::sphere(1.0, 2)));
    assert!(invalid(Solid::sphere(1.0, 5000)));
    assert!(invalid(Solid::sphere(f64::INFINITY, 16)));
    assert!(invalid(Solid::cylinder(1.0, 0.0, 0.0, 16, false)));
    assert!(invalid(Solid::cylinder(1.0, -0.1, 1.0, 16, false)));
    let cube = Solid::cube([1.0, 1.0, 1.0], false).unwrap();
    assert!(invalid(cube.scale([0.0, 1.0, 1.0])));
    assert!(invalid(cube.translate([f64::NAN, 0.0, 0.0])));
    assert!(invalid(cube.rotate([f64::INFINITY, 0.0, 0.0])));
}

#[test]
fn rejects_bad_meshes() {
    let tri = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let open = Mesh {
        positions: tri.clone(),
        indices: vec![0, 1, 2],
    };
    assert_eq!(
        Solid::from_mesh(&open).unwrap_err(),
        KernelError::NotManifold
    );
    let out_of_range = Mesh {
        positions: tri.clone(),
        indices: vec![0, 1, 7],
    };
    assert_eq!(
        Solid::from_mesh(&out_of_range).unwrap_err(),
        KernelError::OutOfBounds
    );
    let nan = Mesh {
        positions: vec![[f32::NAN, 0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    assert_eq!(Solid::from_mesh(&nan).unwrap_err(), KernelError::NonFinite);
    let ragged = Mesh {
        positions: tri,
        indices: vec![0, 1],
    };
    assert!(matches!(
        Solid::from_mesh(&ragged),
        Err(KernelError::InvalidArgument(_))
    ));
    assert!(matches!(
        Solid::from_mesh(&Mesh::default()),
        Err(KernelError::InvalidArgument(_))
    ));
}

#[test]
fn simplify_rejects_bad_parameters() {
    let mesh = Solid::cube([1.0, 1.0, 1.0], false)
        .unwrap()
        .to_mesh()
        .unwrap();
    for ratio in [0.0, -1.0, 1.5, f64::NAN] {
        assert!(matches!(
            simplify(&mesh, ratio, 0.1),
            Err(KernelError::InvalidArgument(_))
        ));
    }
    assert!(matches!(
        simplify(&mesh, 0.5, -1.0),
        Err(KernelError::InvalidArgument(_))
    ));
}

#[test]
fn solids_can_move_between_threads() {
    let cube = Solid::cube([1.0, 1.0, 1.0], false).unwrap();
    let volume = std::thread::spawn(move || cube.volume()).join().unwrap();
    assert!(close(volume, 1.0, 1e-9));
}
