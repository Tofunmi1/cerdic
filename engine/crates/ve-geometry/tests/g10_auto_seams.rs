//! `auto_seams`: sharp edges are cut, and closed or tube-shaped pieces are opened so they unwrap.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_precision_loss,
    reason = "fixtures are known to be valid; failures are reported by panicking; sphere rings are tiny"
)]

use ve_geometry::{AttrData, PolyMesh, UvMethod, UvUnwrap, auto_seams, unwrap_uv, uv_quality};
use ve_kernel::ExecutionContext;

fn unwrap_options() -> UvUnwrap {
    UvUnwrap {
        method: UvMethod::Conformal,
        pack: true,
        rotate: false,
        margin: 0.02,
    }
}

fn cube() -> PolyMesh {
    PolyMesh::new(
        vec![
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ],
        vec![4; 6],
        vec![
            0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
        ],
    )
    .unwrap()
}

/// A lat-long sphere: triangle caps and quad bands. `capped` false leaves a tube instead (no caps, no poles).
fn round(slices: u32, stacks: u32, sphere: bool) -> PolyMesh {
    let mut positions = Vec::new();
    let (first, last) = if sphere { (0, stacks) } else { (1, stacks - 1) };
    if sphere {
        positions.push([0.0, 1.0, 0.0]);
    }
    let ring_start = u32::try_from(positions.len()).unwrap();
    for j in first..=last {
        if sphere && (j == 0 || j == stacks) {
            continue;
        }
        let theta = std::f32::consts::PI * j as f32 / stacks as f32;
        for i in 0..slices {
            let phi = std::f32::consts::TAU * i as f32 / slices as f32;
            let r = if sphere { theta.sin() } else { 1.0 };
            let y = if sphere {
                theta.cos()
            } else {
                j as f32 / stacks as f32
            };
            positions.push([r * phi.cos(), y, r * phi.sin()]);
        }
    }
    let rings = if sphere { stacks - 1 } else { last - first + 1 };
    let ring = |k: u32, i: u32| ring_start + k * slices + i % slices;
    let (mut sizes, mut corners) = (Vec::new(), Vec::new());
    if sphere {
        let south = u32::try_from(positions.len()).unwrap();
        positions.push([0.0, -1.0, 0.0]);
        for i in 0..slices {
            sizes.push(3);
            corners.extend([0, ring(0, i + 1), ring(0, i)]);
            sizes.push(3);
            corners.extend([south, ring(rings - 1, i), ring(rings - 1, i + 1)]);
        }
    }
    for k in 0..rings - 1 {
        for i in 0..slices {
            sizes.push(4);
            corners.extend([
                ring(k, i),
                ring(k, i + 1),
                ring(k + 1, i + 1),
                ring(k + 1, i),
            ]);
        }
    }
    PolyMesh::new(positions, sizes, corners).unwrap()
}

fn uvs_in_unit_square(mesh: &PolyMesh) {
    for name in ["u", "v"] {
        let AttrData::F32(values) = &mesh.attributes()[name].data else {
            panic!("{name} is not f32")
        };
        assert_eq!(values.len(), mesh.corners().len());
        assert!(
            values.iter().all(|v| (-1e-5..=1.0 + 1e-5).contains(v)),
            "{name} stays in the unit square"
        );
    }
}

#[test]
fn every_edge_of_a_cube_is_sharp_enough_to_cut() {
    let context = ExecutionContext::new().unwrap();
    assert_eq!(auto_seams(&cube(), 1.1, &context).unwrap().len(), 12);
    // Above 90 degrees nothing on a cube is sharp, and the closed piece is opened by a path instead.
    let gentle = auto_seams(&cube(), 2.0, &context).unwrap();
    assert!(
        !gentle.is_empty() && gentle.len() < 12,
        "a cube with no sharp edges gets a cut path: {gentle:?}"
    );
}

#[test]
fn a_smooth_closed_sphere_gets_a_cut_that_lets_it_unwrap() {
    let context = ExecutionContext::new().unwrap();
    let sphere = round(12, 8, true);
    let seams = auto_seams(&sphere, 1.1, &context).unwrap();
    assert!(!seams.is_empty());
    let result = unwrap_uv(&sphere, &seams, &unwrap_options(), &context).unwrap();
    uvs_in_unit_square(&result);
    let cut = uv_quality(&result).unwrap();
    let uncut = uv_quality(&unwrap_uv(&sphere, &[], &unwrap_options(), &context).unwrap());
    // Without seams the unwrap leaves no usable layout at all; with charts it is nearly even.
    assert!(
        uncut.is_none(),
        "an uncut closed sphere has no usable UVs: {uncut:?}"
    );
    assert_eq!(
        cut.flipped_faces, 0,
        "a cut sphere lies flat without folding"
    );
    assert!(
        cut.overlap < 0.02,
        "islands do not overlap: {}",
        cut.overlap
    );
    assert!(
        cut.max_density_ratio < 2.0,
        "no face is stretched past 2x: {}",
        cut.max_density_ratio
    );
    assert!(
        cut.min_density_ratio > 0.5,
        "no face is squashed past half: {}",
        cut.min_density_ratio
    );
}

#[test]
fn an_open_tube_is_cut_between_its_two_rims() {
    let context = ExecutionContext::new().unwrap();
    let tube = round(12, 6, false);
    let seams = auto_seams(&tube, 1.1, &context).unwrap();
    assert!(
        !seams.is_empty(),
        "the tube has two outlines and needs a cut joining them"
    );
    let result = unwrap_uv(&tube, &seams, &unwrap_options(), &context).unwrap();
    uvs_in_unit_square(&result);
    let quality = uv_quality(&result).unwrap();
    assert_eq!(quality.flipped_faces, 0);
    assert!(
        quality.max_density_ratio < 2.0 && quality.min_density_ratio > 0.5,
        "{quality:?}"
    );
}

#[test]
fn a_wide_cone_keeps_a_sphere_in_one_piece_and_the_quality_report_shows_the_cost() {
    // Charts are cones of seam_angle: at nearly 180 degrees the whole sphere is one chart, opened by a single
    // path, which is what the old behaviour did: heavy stretch near the cut, and the report says so.
    let context = ExecutionContext::new().unwrap();
    let sphere = round(12, 8, true);
    let seams = auto_seams(&sphere, 3.1, &context).unwrap();
    let result = unwrap_uv(&sphere, &seams, &unwrap_options(), &context).unwrap();
    let quality = uv_quality(&result).unwrap();
    assert!(
        quality.max_density_ratio > 4.0 || quality.min_density_ratio < 0.25,
        "{quality:?}"
    );
}

#[test]
fn a_flat_open_grid_needs_no_seams() {
    let context = ExecutionContext::new().unwrap();
    let grid = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
        ],
        vec![4, 4],
        vec![0, 3, 4, 1, 1, 4, 5, 2],
    )
    .unwrap();
    assert!(auto_seams(&grid, 1.1, &context).unwrap().is_empty());
}

#[test]
fn seams_are_deterministic() {
    let context = ExecutionContext::new().unwrap();
    let sphere = round(12, 8, true);
    assert_eq!(
        auto_seams(&sphere, 1.1, &context).unwrap(),
        auto_seams(&sphere, 1.1, &context).unwrap()
    );
}
