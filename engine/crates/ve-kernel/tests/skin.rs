//! The Skin modifier through the Rust wrapper: a skeleton graph becomes one closed, manifold, mostly-quad surface.
#![expect(clippy::unwrap_used, reason = "tests")]
use std::collections::HashMap;

use ve_kernel::{KernelError, SkinJoint, SkinOptions, skin_graph};

fn joint(x: f32, y: f32, z: f32, r: f32, root: bool) -> SkinJoint {
    SkinJoint { position: [x, y, z], radius: [r, r], root, loose: false }
}

fn euler_and_manifold(faces: &[u32], corners: &[u32], vertices: usize) -> (i64, bool) {
    let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
    let mut at = 0;
    for size in faces {
        for k in 0..*size as usize {
            let a = corners[at + k];
            let b = corners[at + (k + 1) % *size as usize];
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
        at += *size as usize;
    }
    let manifold = edges.values().all(|c| *c == 2);
    (i64::try_from(vertices).unwrap() - i64::try_from(edges.len()).unwrap() + i64::try_from(faces.len()).unwrap(), manifold)
}

#[test]
fn a_y_shaped_skeleton_becomes_one_closed_quad_surface() {
    let joints = [
        joint(0.0, 0.0, 0.0, 0.2, true),
        joint(1.0, 0.0, 0.0, 0.1, false),
        joint(-0.5, 0.87, 0.0, 0.1, false),
        joint(-0.5, -0.87, 0.0, 0.1, false),
    ];
    let skin = skin_graph(&joints, &[[0, 1], [0, 2], [0, 3]], &SkinOptions::default()).unwrap();
    let (euler, manifold) = euler_and_manifold(&skin.face_sizes, &skin.corners, skin.positions.len());
    assert!(manifold, "every edge joins two faces");
    assert_eq!(euler, 2, "one closed sphere-like surface");
    let quads = skin.face_sizes.iter().filter(|s| **s == 4).count();
    assert!(quads * 10 >= skin.face_sizes.len() * 8, "mostly quads: {quads} of {}", skin.face_sizes.len());
    assert!(skin.vertex_origin.iter().all(|o| *o < 4 || *o == u32::MAX), "every vertex names its joint");
}

#[test]
fn a_piece_without_a_root_joint_is_refused() {
    let joints = [joint(0.0, 0.0, 0.0, 0.1, false), joint(0.0, 1.0, 0.0, 0.1, false)];
    assert!(matches!(skin_graph(&joints, &[[0, 1]], &SkinOptions::default()), Err(KernelError::InvalidArgument(_))));
}

#[test]
fn bad_bones_and_radii_are_refused() {
    let joints = [joint(0.0, 0.0, 0.0, 0.1, true), joint(0.0, 1.0, 0.0, 0.1, false)];
    assert!(matches!(skin_graph(&joints, &[[0, 7]], &SkinOptions::default()), Err(KernelError::OutOfBounds)));
    assert!(matches!(skin_graph(&joints, &[[1, 1]], &SkinOptions::default()), Err(KernelError::InvalidArgument(_))));
    let mut negative = joints;
    negative[0].radius = [-1.0, 0.1];
    assert!(matches!(skin_graph(&negative, &[[0, 1]], &SkinOptions::default()), Err(KernelError::InvalidArgument(_))));
}
