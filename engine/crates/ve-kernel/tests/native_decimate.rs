//! Decimation: plain, with every face keeping the face it came from, and symmetric about an axis.

use std::collections::BTreeMap;

use ve_kernel::{EditedMesh, MeshOp, MeshSelection, PolygonMesh, edit_mesh};

/// A triangulated UV sphere of radius 1 whose vertices are mirror images across x = 0 (the segments start at angle 0 and their count is even), moved by `shift` along x.
fn sphere(segments: usize, rings: usize, shift: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut positions = vec![[shift, 1.0, 0.0], [shift, -1.0, 0.0]];
    for j in 1..rings {
        let theta = std::f32::consts::PI * j as f32 / rings as f32;
        for k in 0..segments {
            let phi = std::f32::consts::TAU * k as f32 / segments as f32;
            positions.push([shift + theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin()]);
        }
    }
    let ring = |j: usize, k: usize| (2 + (j - 1) * segments + k % segments) as u32;
    let mut corners = Vec::new();
    for k in 0..segments {
        corners.extend([0, ring(1, k + 1), ring(1, k)]);
        corners.extend([1, ring(rings - 1, k), ring(rings - 1, k + 1)]);
    }
    for j in 1..rings - 1 {
        for k in 0..segments {
            let (a, b, c, d) = (ring(j, k), ring(j, k + 1), ring(j + 1, k + 1), ring(j + 1, k));
            corners.extend([a, b, c, a, c, d]);
        }
    }
    (positions, corners)
}

fn decimate(positions: &[[f32; 3]], corners: &[u32], ratio: f64, symmetry: Option<u8>) -> EditedMesh {
    let sizes = vec![3_u32; corners.len() / 3];
    let mesh = PolygonMesh { positions, face_sizes: &sizes, corners };
    edit_mesh(&mesh, &MeshSelection::default(), &MeshOp::Decimate { ratio, symmetry, triangulate: true }).expect("decimation runs")
}

fn centre(result: &EditedMesh, face: usize) -> [f32; 3] {
    let start: usize = result.face_sizes[..face].iter().map(|&n| n as usize).sum();
    let corners = &result.corners[start..start + result.face_sizes[face] as usize];
    let mut sum = [0.0; 3];
    for &v in corners {
        for (slot, value) in sum.iter_mut().zip(result.positions[v as usize]) {
            *slot += value / corners.len() as f32;
        }
    }
    sum
}

#[test]
fn plain_decimation_keeps_the_face_each_triangle_came_from() {
    let (positions, corners) = sphere(24, 12, 0.0);
    let input_faces = corners.len() / 3;
    let result = decimate(&positions, &corners, 0.5, None);
    assert!(result.face_sizes.len() < input_faces * 7 / 10, "{} of {input_faces}", result.face_sizes.len());
    assert!(result.vertex_origin.iter().all(|&o| (o as usize) < positions.len()));
    assert!(result.face_parent.iter().all(|&p| (p as usize) < input_faces), "every face names a parent");
    // A parent is the triangle a survivor grew out of, so it sits close to it (the sphere is 2 across).
    for face in 0..result.face_sizes.len() {
        let parent = result.face_parent[face] as usize;
        let from: Vec<[f32; 3]> = corners[parent * 3..parent * 3 + 3].iter().map(|&v| positions[v as usize]).collect();
        let at = centre(&result, face);
        let near = from.iter().map(|p| (0..3).map(|i| (p[i] - at[i]).powi(2)).sum::<f32>().sqrt()).fold(f32::MAX, f32::min);
        assert!(near < 0.8, "face {face} is {near} from its parent");
    }
}

fn assert_symmetric_and_closed(result: &EditedMesh) {
    let mirror = |p: [f32; 3]| [-p[0], p[1], p[2]];
    for p in &result.positions {
        let want = mirror(*p);
        let found = result.positions.iter().any(|q| (0..3).all(|i| (q[i] - want[i]).abs() < 1e-3));
        assert!(found, "{p:?} has no mirror image");
    }
    let mut uses: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut at = 0_usize;
    for &size in &result.face_sizes {
        let face = &result.corners[at..at + size as usize];
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
        at += size as usize;
    }
    let open: Vec<_> = uses.iter().filter(|&(_, &n)| n != 2).map(|(&(a, b), &n)| (n, result.positions[a as usize], result.positions[b as usize])).collect();
    assert!(open.is_empty(), "{} edges do not join exactly two faces, for example {:?}", open.len(), &open[..open.len().min(4)]);
}

#[test]
fn symmetric_decimation_is_symmetric_and_stays_closed() {
    let (positions, corners) = sphere(24, 12, 0.0);
    let input_faces = corners.len() / 3;
    let result = decimate(&positions, &corners, 0.5, Some(0));
    assert!(result.face_sizes.len() > input_faces / 5 && result.face_sizes.len() < input_faces * 7 / 10, "{} of {input_faces}", result.face_sizes.len());
    assert_symmetric_and_closed(&result);
    assert!(result.vertex_origin.iter().all(|&o| (o as usize) < positions.len()));
    assert!(result.face_parent.iter().all(|&p| (p as usize) < input_faces));
}

#[test]
fn a_mesh_that_is_not_symmetric_still_comes_out_symmetric() {
    // The sphere straddles the plane unevenly: the cut and mirror make it symmetric, so it must not be refused.
    let (positions, corners) = sphere(24, 12, 0.3);
    let result = decimate(&positions, &corners, 0.5, Some(0));
    assert!(!result.face_sizes.is_empty());
    assert_symmetric_and_closed(&result);
}
