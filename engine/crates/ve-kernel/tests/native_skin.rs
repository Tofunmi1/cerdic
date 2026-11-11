//! Numeric checks for the native chain Skin implementation.
use std::collections::HashMap;
use ve_kernel::native::skin;
use ve_kernel::{KernelError, SkinJoint, SkinOptions};

fn joint(position: [f32; 3], radius: [f32; 2], root: bool) -> SkinJoint {
    SkinJoint {
        position,
        radius,
        root,
        loose: false,
    }
}

fn volume(mesh: &ve_kernel::BeveledMesh) -> f64 {
    let mut at = 0;
    let mut total = 0.0;
    for size in &mesh.face_sizes {
        let face = &mesh.corners[at..at + *size as usize];
        let a = mesh.positions[face[0] as usize];
        for i in 1..face.len() - 1 {
            let b = mesh.positions[face[i] as usize];
            let c = mesh.positions[face[i + 1] as usize];
            let cross = [
                f64::from(b[1]) * f64::from(c[2]) - f64::from(b[2]) * f64::from(c[1]),
                f64::from(b[2]) * f64::from(c[0]) - f64::from(b[0]) * f64::from(c[2]),
                f64::from(b[0]) * f64::from(c[1]) - f64::from(b[1]) * f64::from(c[0]),
            ];
            total += (f64::from(a[0]) * cross[0]
                + f64::from(a[1]) * cross[1]
                + f64::from(a[2]) * cross[2])
                / 6.0;
        }
        at += *size as usize;
    }
    total
}

fn closed(mesh: &ve_kernel::BeveledMesh) {
    let mut edges = HashMap::<(u32, u32), u32>::new();
    let mut at = 0;
    for size in &mesh.face_sizes {
        for i in 0..*size as usize {
            let a = mesh.corners[at + i];
            let b = mesh.corners[at + (i + 1) % *size as usize];
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
        at += *size as usize;
    }
    assert!(edges.values().all(|count| *count == 2));
}

#[test]
fn straight_bone_has_box_volume_counts_and_provenance() {
    let joints = [
        joint([0.0, 0.0, 0.0], [0.5, 0.25], true),
        joint([4.0, 0.0, 0.0], [0.5, 0.25], false),
    ];
    let options = SkinOptions {
        subdivide: false,
        ..SkinOptions::default()
    };
    let mesh = skin::skin_graph(&joints, &[[0, 1]], &options)
        .unwrap()
        .unwrap();
    assert_eq!(mesh.positions.len(), 8);
    assert_eq!(mesh.face_sizes.len(), 6);
    assert!((volume(&mesh) - 2.0).abs() < 1.0e-5);
    assert!(volume(&mesh) > 0.0);
    assert_eq!(mesh.vertex_origin, vec![0, 0, 0, 0, 1, 1, 1, 1]);
    assert!(mesh.face_parent.iter().all(|p| *p == u32::MAX));
    closed(&mesh);
}

#[test]
fn curved_five_joint_chain_is_closed_and_bounded() {
    let joints = [
        joint([0.0, 0.0, 0.0], [0.2, 0.15], true),
        joint([1.0, 0.0, 0.0], [0.2, 0.15], false),
        joint([2.0, 0.5, 0.0], [0.2, 0.15], false),
        joint([3.0, 0.5, 0.5], [0.2, 0.15], false),
        joint([4.0, 1.0, 0.5], [0.2, 0.15], false),
    ];
    let options = SkinOptions {
        subdivide: false,
        ..SkinOptions::default()
    };
    let mesh = skin::skin_graph(&joints, &[[0, 1], [1, 2], [2, 3], [3, 4]], &options)
        .unwrap()
        .unwrap();
    assert_eq!(mesh.positions.len(), 20);
    assert_eq!(mesh.face_sizes.len(), 18);
    closed(&mesh);
    let v = volume(&mesh);
    assert!(v > 0.0);
    let max_x = mesh
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_x = mesh
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min);
    assert!((-0.25..=0.0).contains(&min_x));
    assert!((4.0..=4.25).contains(&max_x));
}

#[test]
fn a_branch_is_one_closed_mostly_quad_surface_and_a_missing_root_is_an_error() {
    let branched = [
        joint([0.0, 0.0, 0.0], [0.2, 0.2], true),
        joint([1.0, 0.0, 0.0], [0.1, 0.1], false),
        joint([0.0, 1.0, 0.0], [0.1, 0.1], false),
        joint([0.0, 0.0, 1.0], [0.1, 0.1], false),
    ];
    let mesh = skin::skin_graph(
        &branched,
        &[[0, 1], [0, 2], [0, 3]],
        &SkinOptions::default(),
    )
    .expect("branches are handled")
    .expect("ok");
    closed(&mesh);
    let euler = mesh.positions.len() as i64 - (mesh.corners.len() / 2) as i64
        + mesh.face_sizes.len() as i64;
    assert_eq!(euler, 2);
    let quads = mesh.face_sizes.iter().filter(|&&s| s == 4).count();
    assert!(
        quads * 10 >= mesh.face_sizes.len() * 6,
        "mostly quads: {quads} of {}",
        mesh.face_sizes.len()
    );
    assert!(volume(&mesh) > 0.0, "outward winding");
    assert!(mesh.vertex_origin.iter().all(|&o| o < 4));
    // Three legs of length 1 with a 0.2 and 0.1 radius are in the right range: not much under three thin boxes.
    let thin = 3.0 * 1.0 * 0.2 * 0.2;
    assert!(
        volume(&mesh) > thin * 0.8 && volume(&mesh) < thin * 4.0,
        "{}",
        volume(&mesh)
    );
    let no_root = [
        joint([0.0, 0.0, 0.0], [0.1, 0.1], false),
        joint([0.0, 1.0, 0.0], [0.1, 0.1], false),
    ];
    assert_eq!(
        skin::skin_graph(&no_root, &[[0, 1]], &SkinOptions::default())
            .unwrap()
            .unwrap_err(),
        KernelError::InvalidArgument("every connected piece needs exactly one root joint")
    );
}

mod against_legacy {
    use ve_kernel::native::skin::skin_graph as native;
    use ve_kernel::{BeveledMesh, SkinJoint, SkinOptions};

    fn joint(x: f32, y: f32, z: f32, r0: f32, r1: f32, root: bool) -> SkinJoint {
        SkinJoint {
            position: [x, y, z],
            radius: [r0, r1],
            root,
            loose: false,
        }
    }

    fn volume(mesh: &BeveledMesh) -> f64 {
        let mut total = 0.0;
        let mut at = 0;
        for &size in &mesh.face_sizes {
            let c = &mesh.corners[at..at + size as usize];
            let p = |i: u32| mesh.positions[i as usize].map(f64::from);
            let a = p(c[0]);
            for pair in c[1..].windows(2) {
                let (b, d) = (p(pair[0]), p(pair[1]));
                total += (a[0] * (b[1] * d[2] - b[2] * d[1]) - a[1] * (b[0] * d[2] - b[2] * d[0])
                    + a[2] * (b[0] * d[1] - b[1] * d[0]))
                    / 6.0;
            }
            at += size as usize;
        }
        total
    }

    fn bounds(mesh: &BeveledMesh) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in &mesh.positions {
            for d in 0..3 {
                lo[d] = lo[d].min(p[d]);
                hi[d] = hi[d].max(p[d]);
            }
        }
        (lo, hi)
    }

    #[test]
    fn a_bent_limb_matches_the_legacy_volume_and_extent() {
        let joints = [
            joint(0.0, 0.0, 0.0, 0.30, 0.30, true),
            joint(0.0, 0.0, 1.0, 0.25, 0.25, false),
            joint(0.3, 0.0, 2.0, 0.20, 0.20, false),
            joint(0.9, 0.0, 2.6, 0.15, 0.15, false),
        ];
        let bones = [[0, 1], [1, 2], [2, 3]];
        for subdivide in [false, true] {
            let options = SkinOptions {
                subdivide,
                ..SkinOptions::default()
            };
            let ours = native(&joints, &bones, &options)
                .expect("handled")
                .expect("ok");
            // A square cross-section (2r by 2r, r the mean radius of each bone) along the three bones: about 0.618,
            // a little more where the bends are mitred.
            let v = volume(&ours).abs();
            assert!(v > 0.5 && v < 0.78, "subdivide {subdivide}: volume {v}");
            // The surface stops at the end joints: it spans the skeleton plus one radius at most.
            let (lo, hi) = bounds(&ours);
            assert!((lo[2] - 0.0).abs() < 0.05 && (hi[2] - 2.6).abs() < 0.2, "z span {} .. {}", lo[2], hi[2]);
            assert!(lo[0] > -0.4 && hi[0] < 1.2, "x span {} .. {}", lo[0], hi[0]);
        }
    }
}

mod junctions {
    use std::collections::BTreeMap;

    use ve_kernel::native::skin::skin_graph as native;
    use ve_kernel::{BeveledMesh, SkinJoint, SkinOptions};

    fn joint(x: f32, y: f32, z: f32, r: f32, root: bool) -> SkinJoint {
        SkinJoint { position: [x, y, z], radius: [r, r], root, loose: false }
    }

    fn closed_and_oriented(mesh: &BeveledMesh) -> bool {
        let mut directed: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        let mut at = 0;
        for &size in &mesh.face_sizes {
            let f = &mesh.corners[at..at + size as usize];
            for i in 0..f.len() {
                *directed.entry((f[i], f[(i + 1) % f.len()])).or_default() += 1;
            }
            at += size as usize;
        }
        directed.iter().all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
    }

    fn euler(mesh: &BeveledMesh) -> i64 {
        mesh.positions.len() as i64 - (mesh.corners.len() / 2) as i64 + mesh.face_sizes.len() as i64
    }

    fn volume(mesh: &BeveledMesh) -> f64 {
        let mut total = 0.0;
        let mut at = 0;
        for &size in &mesh.face_sizes {
            let c = &mesh.corners[at..at + size as usize];
            let p = |i: u32| mesh.positions[i as usize].map(f64::from);
            let a = p(c[0]);
            for pair in c[1..].windows(2) {
                let (b, d) = (p(pair[0]), p(pair[1]));
                total += (a[0] * (b[1] * d[2] - b[2] * d[1]) - a[1] * (b[0] * d[2] - b[2] * d[0]) + a[2] * (b[0] * d[1] - b[1] * d[0])) / 6.0;
            }
            at += size as usize;
        }
        total
    }

    fn quad_share(mesh: &BeveledMesh) -> f64 {
        mesh.face_sizes.iter().filter(|&&n| n == 4).count() as f64 / mesh.face_sizes.len() as f64
    }

    #[test]
    fn a_wide_y_gets_a_quad_junction() {
        let joints = [
            joint(0.0, 0.0, 0.0, 0.2, true),
            joint(1.0, 0.0, 0.0, 0.1, false),
            joint(-0.5, 0.87, 0.0, 0.1, false),
            joint(-0.5, -0.87, 0.0, 0.1, false),
        ];
        let bones = [[0, 1], [0, 2], [0, 3]];
        let mesh = native(&joints, &bones, &SkinOptions::default()).expect("handled").expect("ok");
        assert!(closed_and_oriented(&mesh), "closed and consistently wound");
        assert_eq!(euler(&mesh), 2);
        assert!(volume(&mesh) > 0.0, "outward");
        assert!(quad_share(&mesh) >= 0.8, "{}", quad_share(&mesh));
        // Three legs of length 1 with radius 0.1 (cross-section 0.04) plus the thicker hub: a little over 0.12.
        let v = volume(&mesh);
        assert!(v > 0.1 && v < 0.35, "volume {v}");
    }

    #[test]
    fn a_four_legged_skeleton_is_closed_with_two_junctions() {
        // A spine along x with two branch joints, each with a left and right leg going down.
        let joints = [
            joint(0.0, 0.0, 0.0, 0.2, true),
            joint(1.0, 0.0, 0.0, 0.2, false),
            joint(2.0, 0.0, 0.0, 0.15, false),
            joint(0.0, -0.8, 1.6, 0.1, false),
            joint(0.0, -0.8, -1.6, 0.1, false),
            joint(2.0, -0.8, 1.6, 0.1, false),
            joint(2.0, -0.8, -1.6, 0.1, false),
            joint(-0.8, 0.2, 0.0, 0.1, false),
        ];
        // Branch joints 0 (spine end, two legs and a neck) and 2 (two legs and the spine).
        let bones = [[0, 1], [1, 2], [0, 3], [0, 4], [0, 7], [2, 5], [2, 6]];
        let mesh = native(&joints, &bones, &SkinOptions::default());
        let Some(Ok(mesh)) = mesh else {
            panic!("the native path should take this skeleton, got {}", if matches!(mesh, None) { "a decline" } else { "an error" });
        };
        assert!(closed_and_oriented(&mesh));
        assert_eq!(euler(&mesh), 2);
        assert!(volume(&mesh) > 0.0);
        assert!(mesh.vertex_origin.iter().all(|&o| (o as usize) < joints.len() || o == u32::MAX));
    }

    #[test]
    fn a_narrow_fork_is_closed_or_declines() {
        let joints = [
            joint(0.0, 0.0, 0.0, 0.15, true),
            joint(1.0, 0.1, 0.0, 0.1, false),
            joint(1.0, -0.1, 0.0, 0.1, false),
        ];
        match native(&joints, &[[0, 1], [0, 2]], &SkinOptions::default()) {
            None => {}
            Some(result) => {
                let mesh = result.expect("ok");
                assert!(closed_and_oriented(&mesh));
                assert_eq!(euler(&mesh), 2);
            }
        }
    }
}

mod tips {
    use ve_kernel::native::skin::skin_graph as native;
    use ve_kernel::{SkinJoint, SkinOptions};

    fn joint(x: f32, r: f32, root: bool) -> SkinJoint {
        SkinJoint { position: [x, 0.0, 0.0], radius: [r, r], root, loose: false }
    }

    #[test]
    fn a_limb_can_taper_to_a_point() {
        let joints = [joint(0.0, 0.3, true), joint(1.0, 0.2, false), joint(2.0, 0.0, false)];
        let options = SkinOptions { subdivide: false, ..SkinOptions::default() };
        let mesh = native(&joints, &[[0, 1], [1, 2]], &options).expect("handled").expect("ok");
        // Two rings of four and one apex; four quads, four triangles, four more quads and the start cap.
        assert_eq!(mesh.positions.len(), 9);
        assert_eq!(mesh.face_sizes.iter().filter(|&&n| n == 3).count(), 4);
        let euler = mesh.positions.len() as i64 - (mesh.corners.len() / 2) as i64 + mesh.face_sizes.len() as i64;
        assert_eq!(euler, 2);
        // The tip is the end joint itself.
        let far = mesh.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        assert!((far - 2.0).abs() < 1e-6, "tip at {far}");
    }

    #[test]
    fn a_pinch_in_the_middle_declines() {
        let joints = [joint(0.0, 0.3, true), joint(1.0, 0.0, false), joint(2.0, 0.3, false)];
        assert!(native(&joints, &[[0, 1], [1, 2]], &SkinOptions::default()).is_none());
    }
}
