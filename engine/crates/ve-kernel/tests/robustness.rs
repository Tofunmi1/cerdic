//! Robustness: arbitrary, mostly invalid input must produce `Ok` or a typed error, never a crash.
//! This is the stand-in for a fuzz target until `cargo-fuzz` is part of the toolchain.
#![cfg(not(target_family = "wasm"))]

use proptest::prelude::*;
use ve_kernel::{Mesh, Solid, simplify};

fn coordinate() -> impl Strategy<Value = f32> {
    prop_oneof![
        8 => -10.0_f32..10.0,
        1 => Just(f32::NAN),
        1 => Just(f32::INFINITY),
        1 => Just(f32::NEG_INFINITY),
        1 => Just(0.0),
    ]
}

fn mesh() -> impl Strategy<Value = Mesh> {
    (
        prop::collection::vec(prop::array::uniform3(coordinate()), 0..24),
        prop::collection::vec(0_u32..40, 0..72),
    )
        .prop_map(|(positions, indices)| Mesh { positions, indices })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn from_mesh_never_crashes(mesh in mesh()) {
        // Either outcome is acceptable; reaching the end means the kernel validated safely.
        let _ = Solid::from_mesh(&mesh);
    }

    #[test]
    fn simplify_never_crashes(mesh in mesh(), ratio in -1.0_f64..2.0, error in -1.0_f64..1.0) {
        let _ = simplify(&mesh, ratio, error);
    }

    #[test]
    fn well_formed_results_have_consistent_counts(mesh in mesh()) {
        if let Ok(solid) = Solid::from_mesh(&mesh) {
            let out = solid.to_mesh().unwrap();
            prop_assert_eq!(out.triangle_count(), solid.triangle_count());
            prop_assert_eq!(out.vertex_count(), solid.vertex_count());
            prop_assert!(out.indices.iter().all(|&i| (i as usize) < out.vertex_count()));
        }
    }
}
