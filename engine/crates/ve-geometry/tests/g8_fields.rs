//! G8 field expressions, reference samples, analytic meshing and bounded refusals.
#![expect(
    clippy::unwrap_used,
    reason = "small fixed numeric fixtures should fail immediately on invalid setup"
)]
use std::collections::BTreeSet;
use ve_geometry::{DistanceGuarantee, Field};
use ve_kernel::{ExecutionContext, FieldError, KernelError, evaluate_tape};

fn sphere_volume(edge: f64) -> (f64, f64) {
    let field = Field::sphere([0.0; 3], 0.7).unwrap();
    let context = ExecutionContext::new().unwrap();
    let solid = ve_geometry::field::mesh(&field, [[-1.0; 3], [1.0; 3]], edge, &context).unwrap();
    (solid.volume(), solid.surface_area())
}

fn euler_characteristic(mesh: &ve_kernel::Mesh) -> isize {
    let edges = mesh
        .indices
        .chunks_exact(3)
        .flat_map(|triangle| {
            [
                (triangle[0].min(triangle[1]), triangle[0].max(triangle[1])),
                (triangle[1].min(triangle[2]), triangle[1].max(triangle[2])),
                (triangle[2].min(triangle[0]), triangle[2].max(triangle[0])),
            ]
        })
        .collect::<BTreeSet<_>>();
    isize::try_from(mesh.positions.len()).unwrap() - isize::try_from(edges.len()).unwrap()
        + isize::try_from(mesh.triangle_count()).unwrap()
}

#[test]
fn g8_sphere_volume_and_area_error_shrink_when_resolution_doubles() {
    let (coarse_volume, coarse_area) = sphere_volume(0.16);
    let (fine_volume, fine_area) = sphere_volume(0.08);
    let exact_volume = 4.0 * std::f64::consts::PI * 0.7_f64.powi(3) / 3.0;
    let exact_area = 4.0 * std::f64::consts::PI * 0.7_f64.powi(2);
    assert!((fine_volume - exact_volume).abs() < (coarse_volume - exact_volume).abs());
    assert!((fine_area - exact_area).abs() < (coarse_area - exact_area).abs());
}

#[test]
fn g8_torus_has_analytic_volume_and_genus_one_topology() {
    let field = Field::torus(0.5, 0.2).unwrap();
    let context = ExecutionContext::new().unwrap();
    let solid = ve_geometry::field::mesh(&field, [[-1.0; 3], [1.0; 3]], 0.04, &context).unwrap();
    let expected = 2.0 * std::f64::consts::PI.powi(2) * 0.5 * 0.2_f64.powi(2);
    assert!((solid.volume() - expected).abs() < 0.015);
    assert_eq!(solid.component_count(), 1);
    assert_eq!(euler_characteristic(&solid.to_mesh().unwrap()), 0);
}

#[test]
fn g8_rust_reference_matches_cpp_interpreter_for_every_instruction() {
    let expressions = vec![
        Field::sphere([0.1, -0.2, 0.3], 0.6).unwrap(),
        Field::ellipsoid([0.6, 0.4, 0.2]).unwrap(),
        Field::rounded_box([0.5, 0.4, 0.3], 0.1).unwrap(),
        Field::torus(0.5, 0.2).unwrap(),
        Field::capsule(0.2, 0.5).unwrap(),
        Field::cylinder(0.4, 0.5).unwrap(),
        Field::plane([1.0, 2.0, -3.0], 0.2).unwrap(),
        Field::tube(
            vec![[0.0, 0.0, 0.0], [0.4, 0.3, -0.2], [0.8, 0.2, 0.1]],
            0.12,
        )
        .unwrap(),
        Field::sphere([0.0; 3], 0.2)
            .unwrap()
            .repeated([1.0; 3], [1, 0, 0])
            .unwrap(),
        Field::sphere([-0.3, 0.0, 0.0], 0.5)
            .unwrap()
            .union(Field::sphere([0.3, 0.0, 0.0], 0.5).unwrap()),
        Field::sphere([-0.3, 0.0, 0.0], 0.5)
            .unwrap()
            .smooth_union(Field::sphere([0.3, 0.0, 0.0], 0.5).unwrap(), 0.2)
            .unwrap(),
        Field::sphere([0.1, 0.2, 0.3], 0.4)
            .unwrap()
            .moved([0.2, -0.1, 0.3])
            .unwrap()
            .scaled(1.4)
            .unwrap()
            .mirrored([true, false, true])
            .offset(0.1)
            .unwrap()
            .shell(0.07)
            .unwrap(),
    ];
    for field in expressions {
        let tape = ve_geometry::field::compile(&field).unwrap();
        for x in -3..=3 {
            for y in -2..=2 {
                let point = [f64::from(x) * 0.17, f64::from(y) * 0.13, 0.29];
                assert_eq!(
                    ve_geometry::field::evaluate(&field, point).to_bits(),
                    evaluate_tape(&tape, point).unwrap().to_bits(),
                    "sample mismatch at {point:?} with tape {tape:?}"
                );
            }
        }
    }
}

#[test]
fn g8_two_point_tube_matches_capsule_volume_and_rejects_degenerate_segments() {
    let radius = 0.2;
    let length = 1.0;
    let tube = Field::tube(vec![[-0.5, 0.0, 0.0], [0.5, 0.0, 0.0]], radius).unwrap();
    assert_eq!(tube.guarantee(), DistanceGuarantee::Exact);
    let context = ExecutionContext::new().unwrap();
    let solid = ve_geometry::field::mesh(&tube, [[-1.0; 3], [1.0; 3]], 0.04, &context).unwrap();
    let exact = std::f64::consts::PI * radius * radius * length
        + 4.0 * std::f64::consts::PI * radius.powi(3) / 3.0;
    assert!((solid.volume() - exact).abs() < 0.02);
    assert!(Field::tube(vec![[0.0; 3], [0.0; 3]], radius).is_err());
    assert_eq!(
        Field::tube(vec![[0.0; 3], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]], radius)
            .unwrap()
            .guarantee(),
        DistanceGuarantee::LowerBound
    );
    let too_many = (0_u32..=128)
        .map(|index| [f64::from(index), 0.0, 0.0])
        .collect();
    assert!(Field::tube(too_many, radius).is_err());
}

#[test]
fn g8_bounded_repeat_has_expected_spacing_and_enforces_copy_count() {
    let repeated = Field::sphere([0.0; 3], 0.2)
        .unwrap()
        .repeated([1.0; 3], [1, 0, 0])
        .unwrap();
    assert_eq!(repeated.guarantee(), DistanceGuarantee::Exact);
    assert!(
        Field::sphere([0.0; 3], 0.2)
            .unwrap()
            .repeated([1.0; 3], [17, 0, 0])
            .is_err()
    );
    let actual = ve_geometry::field::evaluate(&repeated, [0.9, 0.05, 0.0]);
    let expected = 0.2 - ((0.1_f64 * 0.1) + (0.05_f64 * 0.05)).sqrt();
    assert!((actual - expected).abs() < 1.0e-15);
}

#[test]
fn g8_ellipsoid_reports_exact_or_conservative_distance_guarantee() {
    let sphere_like = Field::ellipsoid([0.4; 3]).unwrap();
    assert_eq!(sphere_like.guarantee(), DistanceGuarantee::Exact);
    let stretched = Field::ellipsoid([0.6, 0.4, 0.2]).unwrap();
    assert_eq!(stretched.guarantee(), DistanceGuarantee::LowerBound);
    let point = [0.2, 0.1, 0.05];
    let tape = ve_geometry::field::compile(&stretched).unwrap();
    assert_eq!(
        ve_geometry::field::evaluate(&stretched, point).to_bits(),
        evaluate_tape(&tape, point).unwrap().to_bits()
    );
}

#[test]
fn g8_bounds_empty_clipped_budget_and_cancellation_are_typed() {
    let sphere = Field::sphere([0.0; 3], 0.8).unwrap();
    let context = ExecutionContext::new().unwrap();
    let under_sampled =
        ve_geometry::field::mesh(&sphere, [[-1.0; 3], [1.0; 3]], 3.0, &context).unwrap_err();
    assert!(matches!(
        under_sampled,
        ve_geometry::GeometryError::FieldKernel(FieldError::Kernel(KernelError::InvalidArgument(
            _
        )))
    ));
    let clipped =
        ve_geometry::field::mesh(&sphere, [[-0.5; 3], [0.5; 3]], 0.04, &context).unwrap_err();
    assert!(matches!(
        clipped,
        ve_geometry::GeometryError::FieldKernel(FieldError::Clipped { .. })
    ));

    let distant = Field::sphere([0.0; 3], 0.1).unwrap();
    let empty =
        ve_geometry::field::mesh(&distant, [[1.0; 3], [2.0; 3]], 0.08, &context).unwrap_err();
    assert!(matches!(
        empty,
        ve_geometry::GeometryError::FieldKernel(FieldError::Empty)
    ));

    let large =
        ve_geometry::field::mesh(&sphere, [[-1.0; 3], [1.0; 3]], 0.0001, &context).unwrap_err();
    assert!(matches!(
        large,
        ve_geometry::GeometryError::FieldKernel(FieldError::Budget)
    ));

    context.cancel();
    let cancelled =
        ve_geometry::field::mesh(&sphere, [[-1.0; 3], [1.0; 3]], 0.04, &context).unwrap_err();
    assert!(matches!(
        cancelled,
        ve_geometry::GeometryError::FieldKernel(FieldError::Kernel(
            ve_kernel::KernelError::Cancelled
        ))
    ));
}

#[test]
fn g8_guarantees_are_propagated_and_bad_parameters_are_refused() {
    let sphere = Field::sphere([0.0; 3], 1.0).unwrap();
    assert_eq!(sphere.guarantee(), DistanceGuarantee::Exact);
    assert_eq!(
        sphere
            .clone()
            .union(Field::sphere([0.5, 0.0, 0.0], 1.0).unwrap())
            .guarantee(),
        DistanceGuarantee::LowerBound
    );
    assert_eq!(
        sphere
            .smooth_union(Field::sphere([0.5, 0.0, 0.0], 1.0).unwrap(), 0.2)
            .unwrap()
            .union(Field::sphere([2.0, 0.0, 0.0], 1.0).unwrap())
            .guarantee(),
        DistanceGuarantee::None
    );
    assert!(Field::torus(0.1, 0.2).is_err());
    assert!(Field::sphere([0.0; 3], f64::INFINITY).is_err());
}
