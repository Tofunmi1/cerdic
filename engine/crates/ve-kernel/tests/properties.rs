//! Property tests: algebraic laws that must hold for any valid input, not just hand-picked cases.
#![cfg(not(target_family = "wasm"))]

use proptest::prelude::*;
use ve_kernel::Solid;

fn dim() -> impl Strategy<Value = f64> {
    0.2_f64..4.0
}

fn offset() -> impl Strategy<Value = f64> {
    -3.0_f64..3.0
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn cube_volume_is_the_product_of_its_edges(x in dim(), y in dim(), z in dim()) {
        let cube = Solid::cube([x, y, z], false).unwrap();
        prop_assert!((cube.volume() - x * y * z).abs() < 1e-6 * (1.0 + x * y * z));
    }

    #[test]
    fn translation_preserves_volume(x in dim(), y in dim(), z in dim(), dx in offset(), dy in offset(), dz in offset()) {
        let cube = Solid::cube([x, y, z], true).unwrap();
        let moved = cube.translate([dx, dy, dz]).unwrap();
        prop_assert!((cube.volume() - moved.volume()).abs() < 1e-6 * (1.0 + cube.volume()));
    }

    #[test]
    fn union_and_intersection_obey_inclusion_exclusion(
        x in dim(), y in dim(), z in dim(), dx in offset(), dy in offset(), dz in offset()
    ) {
        let a = Solid::cube([x, y, z], true).unwrap();
        let b = a.translate([dx, dy, dz]).unwrap();
        let union = a.union(&b).unwrap();
        let inter = a.intersect(&b).unwrap();
        let lhs = union.volume() + inter.volume();
        let rhs = a.volume() + b.volume();
        prop_assert!((lhs - rhs).abs() < 1e-5 * (1.0 + rhs));
    }

    #[test]
    fn difference_never_grows_and_union_never_shrinks(
        x in dim(), y in dim(), z in dim(), dx in offset(), dy in offset(), dz in offset()
    ) {
        let a = Solid::cube([x, y, z], true).unwrap();
        let b = Solid::sphere(1.0, 24).unwrap().translate([dx, dy, dz]).unwrap();
        let diff = a.difference(&b).unwrap();
        let union = a.union(&b).unwrap();
        prop_assert!(diff.volume() <= a.volume() + 1e-6);
        prop_assert!(union.volume() + 1e-6 >= a.volume().max(b.volume()));
    }

    #[test]
    fn mesh_round_trip_keeps_volume(x in dim(), y in dim(), z in dim()) {
        let cube = Solid::cube([x, y, z], false).unwrap();
        let rebuilt = Solid::from_mesh(&cube.to_mesh().unwrap()).unwrap();
        prop_assert!((cube.volume() - rebuilt.volume()).abs() < 1e-4 * (1.0 + cube.volume()));
    }
}
