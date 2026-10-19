//! Report behaviour: a cube, a torus, holes, flips, components and determinism.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the test grids index and scale with small counts"
)]

use ve_measure::{MeasureError, Mesh, ProblemKind, Report};

fn cube(half: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let h = half;
    let positions = vec![
        [-h, -h, -h],
        [h, -h, -h],
        [h, h, -h],
        [-h, h, -h],
        [-h, -h, h],
        [h, -h, h],
        [h, h, h],
        [-h, h, h],
    ];
    let indices = vec![
        0, 3, 2, 0, 2, 1, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7, 3,
        1, 2, 6, 1, 6, 5,
    ];
    (positions, indices)
}

/// A torus of major radius 1 and minor radius 0.35, as a closed quad grid.
fn torus() -> (Vec<[f32; 3]>, Vec<u32>) {
    let (major, minor) = (1.0f32, 0.35f32);
    let (nu, nv) = (24usize, 16usize);
    let mut positions = Vec::with_capacity(nu * nv);
    for i in 0..nu {
        let u = i as f32 / nu as f32 * std::f32::consts::TAU;
        for j in 0..nv {
            let v = j as f32 / nv as f32 * std::f32::consts::TAU;
            let ring = major + minor * v.cos();
            positions.push([ring * u.cos(), minor * v.sin(), ring * u.sin()]);
        }
    }
    let mut indices = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            let a = (i * nv + j) as u32;
            let b = ((i + 1) % nu * nv + j) as u32;
            let c = ((i + 1) % nu * nv + (j + 1) % nv) as u32;
            let d = (i * nv + (j + 1) % nv) as u32;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    (positions, indices)
}

fn approx(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

#[test]
fn cube_area_volume_topology_and_orientation() {
    let (positions, indices) = cube(0.5);
    let report = Report::of(&Mesh::new(&positions, &indices)).expect("cube is valid");
    assert_eq!(report.triangle_count, 12);
    assert_eq!(report.vertex_count, 8);
    assert!(
        approx(report.surface_area, 6.0, 1e-4),
        "{:?}",
        report.surface_area
    );
    assert!(approx(report.volume, 1.0, 1e-5), "{:?}", report.volume);
    assert!(report.watertight);
    assert!(!report.inside_out);
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(report.euler_characteristic, 2);
    assert_eq!(report.genus, Some(0));
    assert_eq!(report.components.len(), 1);
    assert!(approx(report.components[0].volume, 1.0, 1e-5));
    let size = report.bounding_box.size();
    assert!(approx(size[0], 1.0, 1e-5) && approx(size[1], 1.0, 1e-5));
    assert!(approx(report.centre_of_mass[0], 0.0, 1e-5));
}

#[test]
fn torus_has_genus_one() {
    let (positions, indices) = torus();
    let report = Report::of(&Mesh::new(&positions, &indices)).expect("torus is valid");
    assert!(report.watertight, "{:?}", report.problems);
    assert_eq!(report.components.len(), 1);
    assert_eq!(report.genus, Some(1));
    assert_eq!(report.euler_characteristic, 0);
    assert!(report.volume.abs() > 0.0);
}

#[test]
fn deleting_a_triangle_opens_three_edges() {
    let (positions, mut indices) = cube(0.5);
    indices.drain(0..3);
    let report = Report::of(&Mesh::new(&positions, &indices)).expect("valid");
    assert!(!report.watertight);
    let open = report
        .problems
        .iter()
        .find(|p| p.kind == ProblemKind::OpenEdge)
        .expect("open edges");
    assert_eq!(open.count, 3);
    assert!(open.example.is_some());
}

#[test]
fn flipped_cube_reports_negative_volume() {
    let (positions, indices) = cube(0.5);
    let reversed: Vec<u32> = indices.chunks(3).flat_map(|t| [t[0], t[2], t[1]]).collect();
    let report = Report::of(&Mesh::new(&positions, &reversed)).expect("valid");
    assert!(report.inside_out);
    assert!(approx(report.volume, -1.0, 1e-5));
    assert!(report.summary().contains("inside-out"));
}

#[test]
fn duplicate_and_degenerate_are_reported() {
    let (mut positions, mut indices) = cube(0.5);
    let duplicate = positions[0];
    positions.push(duplicate);
    let report = Report::of(&Mesh::new(&positions, &indices)).expect("valid");
    let dup = report
        .problems
        .iter()
        .find(|p| p.kind == ProblemKind::DuplicateVertex)
        .expect("duplicate vertices");
    assert_eq!(dup.count, 1);

    indices.push(0);
    indices.push(1);
    indices.push(1);
    let report = Report::of(&Mesh::new(&positions, &indices)).expect("valid");
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.kind == ProblemKind::DegenerateTriangle)
    );
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.kind == ProblemKind::ZeroLengthEdge)
    );
}

#[test]
fn two_separate_cubes_are_two_components() {
    let (mut positions, indices) = cube(0.5);
    let offset = positions.len() as u32;
    let shifted: Vec<[f32; 3]> = positions.iter().map(|p| [p[0] + 4.0, p[1], p[2]]).collect();
    positions.extend_from_slice(&shifted);
    let mut all = indices.clone();
    all.extend(indices.iter().map(|i| i + offset));
    let report = Report::of(&Mesh::new(&positions, &all)).expect("valid");
    assert_eq!(report.components.len(), 2);
    assert_eq!(report.genus, Some(0));
    assert!(approx(report.components[0].volume, 1.0, 1e-5));
    assert!(approx(report.components[1].volume, 1.0, 1e-5));
}

#[test]
fn empty_and_malformed_inputs_are_typed_errors() {
    let empty: Vec<[f32; 3]> = Vec::new();
    let no_indices: Vec<u32> = Vec::new();
    assert_eq!(
        Report::of(&Mesh::new(&empty, &no_indices)),
        Err(MeasureError::Empty)
    );

    let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    let bad = vec![0, 1, 9];
    assert_eq!(
        Report::of(&Mesh::new(&positions, &bad)),
        Err(MeasureError::IndexOutOfRange {
            index: 9,
            vertices: 2
        })
    );
}

#[test]
fn the_same_input_gives_an_identical_report() {
    let (positions, indices) = torus();
    let mesh = Mesh::new(&positions, &indices);
    assert_eq!(Report::of(&mesh), Report::of(&mesh));
}
