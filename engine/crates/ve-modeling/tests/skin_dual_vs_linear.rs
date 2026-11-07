//! On a real rig (the `ronin` of web/dist-bevy-gpu/worlds/unit_test.txt), a vertex that one bone owns outright must move the same under the linear
//! and the dual-quaternion blend: both apply that bone's rigid motion. Run bone by bone, so a bone whose pose the dual path mangles shows up
//! (the known bug: the dual path breaks the left toes).
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "fixture"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::rig::{deform, deform_dual};
use ve_modeling::{Evaluator, Quality};

fn pose_about(head: [f32; 3], degrees: f32, axis: usize) -> [f32; 16] {
    let (s, c) = degrees.to_radians().sin_cos();
    let mut m = [0.0_f32; 16];
    m[15] = 1.0;
    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
    m[axis * 4 + axis] = 1.0;
    m[a * 4 + a] = c;
    m[a * 4 + b] = s;
    m[b * 4 + a] = -s;
    m[b * 4 + b] = c;
    // Rotation about `head`: t = head - R head.
    let turned: Vec<f32> = (0..3)
        .map(|row| (0..3).map(|col| m[col * 4 + row] * head[col]).sum::<f32>())
        .collect();
    for row in 0..3 {
        m[12 + row] = head[row] - turned[row];
    }
    m
}

#[test]
fn a_bone_that_owns_a_vertex_moves_it_the_same_in_both_blends() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/dist-bevy-gpu/worlds/unit_test.txt"
    ))
    .unwrap();
    let commands = parse_text(&builtin_registry(), &source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_skin_dual").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    let part = result.parts.iter().find(|p| p.name == "ronin").unwrap();
    let skin = part.skin.as_ref().unwrap();
    let heads: std::collections::HashMap<String, [f32; 3]> = result
        .parts
        .iter()
        .filter_map(|p| p.bone.map(|b| (p.name.clone(), b.head)))
        .collect();
    let identity = pose_about([0.0; 3], 0.0, 0);
    let mut worst = Vec::new();
    for (index, bone) in skin.bones.iter().enumerate() {
        let mut poses = vec![identity; skin.bones.len()];
        poses[index] = pose_about(heads[bone], 25.0, 0);
        let (mut owned, mut max) = (0, 0.0_f32);
        for (vertex, p) in part.positions.iter().enumerate() {
            let (joints, weights) = (skin.joints[vertex], skin.weights[vertex]);
            let owner = (0..4).any(|k| usize::from(joints[k]) == index && weights[k] > 0.8);
            if !owner {
                continue;
            }
            owned += 1;
            let (a, _) = deform(*p, [0.0, 1.0, 0.0], joints, weights, &poses);
            let (b, _) = deform_dual(*p, [0.0, 1.0, 0.0], joints, weights, &poses);
            max = max.max((0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max));
        }
        eprintln!("{bone}: {owned} owned vertices, worst linear/dual difference {max:.6}");
        worst.push((bone.clone(), owned, max));
    }
    for (bone, owned, max) in &worst {
        // A vertex 80 percent owned still shares 20 percent with a neighbour, where the two blends legitimately differ a little.
        assert!(
            *max < 0.02,
            "{bone}: dual blend moves {owned} mostly-owned vertices differently from linear by up to {max}"
        );
    }
    for (bone, owned, max) in &worst {
        if let Some(twin) = bone.strip_suffix("_l").map(|stem| format!("{stem}_r")) {
            let (twin_owned, twin_max) = worst
                .iter()
                .find(|w| w.0 == twin)
                .map(|w| (w.1, w.2))
                .unwrap();
            assert!(
                (*owned as i64 - twin_owned as i64).abs() <= 2 && (max - twin_max).abs() < 0.005,
                "{bone} ({owned} vertices, {max}) differs from {twin} ({twin_owned}, {twin_max}) in a symmetric rig"
            );
        }
    }
}
