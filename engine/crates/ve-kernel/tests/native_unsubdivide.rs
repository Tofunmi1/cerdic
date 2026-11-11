//! Native unsubdivide: the inverse of one level of quad subdivision, in any orientation.

use ve_kernel::native::unsubdivide::edit;
use ve_kernel::{MeshOp, MeshSelection, PolygonMesh, subdivide_catmark};

fn grid(n: usize, rotate: bool) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut positions = Vec::new();
    for y in 0..=n {
        for x in 0..=n {
            let p = [x as f32, y as f32, 0.0];
            positions.push(if rotate { [p[2], p[0] * 0.8 + p[1] * 0.6, p[1] * 0.8 - p[0] * 0.6] } else { p });
        }
    }
    let mut corners = Vec::new();
    for y in 0..n {
        for x in 0..n {
            let a = (y * (n + 1) + x) as u32;
            corners.extend([a, a + 1, a + n as u32 + 2, a + n as u32 + 1]);
        }
    }
    (positions, corners)
}

#[test]
fn a_nine_by_nine_grid_goes_to_five_by_five_in_any_orientation() {
    for rotate in [false, true] {
        let (positions, corners) = grid(8, rotate);
        let mesh = PolygonMesh { positions: &positions, face_sizes: &vec![4; 64], corners: &corners };
        let out = edit(&mesh, &MeshSelection::default(), &MeshOp::Unsubdivide { iterations: 1 })
            .expect("handled")
            .expect("ok");
        assert_eq!(out.positions.len(), 25, "rotate {rotate}");
        assert_eq!(out.face_sizes.len(), 16, "rotate {rotate}");
        // The corners of the grid survive.
        for corner in [positions[0], positions[8], positions[72], positions[80]] {
            assert!(out.positions.contains(&corner), "{corner:?} lost (rotate {rotate})");
        }
        // Every coarse quad keeps the fine grid's facing.
        let mut at = 0;
        for &size in &out.face_sizes {
            let f = &out.corners[at..at + size as usize];
            let p = |i: u32| out.positions[i as usize].map(f64::from);
            let (a, b, c) = (p(f[0]), p(f[1]), p(f[2]));
            let n = [(b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]), (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]), (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])];
            // The rotation maps the grid's +z normal to +x.
            let fine = if rotate { [1.0, 0.0, 0.0_f64] } else { [0.0, 0.0, 1.0] };
            assert!(n[0] * fine[0] + n[1] * fine[1] + n[2] * fine[2] > 0.0, "a coarse face is flipped (rotate {rotate})");
            at += size as usize;
        }
    }
}

#[test]
fn two_levels_of_subdivision_of_a_cube_come_back_to_the_cube() {
    let positions: [[f32; 3]; 8] = [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]];
    let corners: [u32; 24] = [0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7];
    let context = ve_kernel::ExecutionContext::new().expect("context");
    let fine = subdivide_catmark(&positions, &[4; 6], &corners, 2, 1, 0, &context).expect("two levels");
    assert_eq!(fine.face_sizes.len(), 96);
    let mesh = PolygonMesh { positions: &fine.positions, face_sizes: &fine.face_sizes, corners: &fine.corners };
    let cage = edit(&mesh, &MeshSelection::default(), &MeshOp::Unsubdivide { iterations: 2 })
        .expect("handled")
        .expect("ok");
    assert_eq!(cage.face_sizes.len(), 6, "back to six faces");
    assert_eq!(cage.positions.len(), 8, "back to eight corners");
    assert!(cage.face_sizes.iter().all(|&n| n == 4));
    // One level only: 24 faces.
    let one = edit(&mesh, &MeshSelection::default(), &MeshOp::Unsubdivide { iterations: 1 })
        .expect("handled")
        .expect("ok");
    assert_eq!(one.face_sizes.len(), 24);
    // Triangles decline.
    let tri = PolygonMesh { positions: &positions, face_sizes: &[3, 3], corners: &[0, 1, 2, 0, 2, 3] };
    assert!(edit(&tri, &MeshSelection::default(), &MeshOp::Unsubdivide { iterations: 1 }).is_none());
}

#[test]
fn each_level_of_a_subdivided_cube_steps_back_one_level() {
    let positions: [[f32; 3]; 8] = [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]];
    let corners: [u32; 24] = [0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7];
    let context = ve_kernel::ExecutionContext::new().expect("context");
    for level in 1..=2 {
        let fine = subdivide_catmark(&positions, &[4; 6], &corners, level, 1, 0, &context).expect("levels");
        let mesh = PolygonMesh { positions: &fine.positions, face_sizes: &fine.face_sizes, corners: &fine.corners };
        let one = edit(&mesh, &MeshSelection::default(), &MeshOp::Unsubdivide { iterations: 1 }).expect("handled").expect("ok");
        // One level back: 6 faces from level 1, 24 faces from level 2; always quads.
        assert_eq!(one.face_sizes.len(), if level == 1 { 6 } else { 24 }, "level {level}");
        assert!(one.face_sizes.iter().all(|&n| n == 4), "level {level}");
    }
}
