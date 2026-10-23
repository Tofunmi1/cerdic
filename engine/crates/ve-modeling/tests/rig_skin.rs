//! `rig.bone` and `rig.skin`: a tube skinned to two bones gets per-vertex weights, and bad skins are dropped with a reason.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_rig").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { .. } = outcome {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| e.message)
}

const BONES: &str = "lower = solid.box@1(size=[0.1, 0.1, 0.1])\n\
    lower_bone_part = rig.bone@1(lower, head=[0, -1, 0], tail=[0, 0, 0])\n\
    output lower_bone = lower_bone_part\n\
    upper = solid.box@1(size=[0.1, 0.1, 0.1])\n\
    upper_bone_part = rig.bone@1(upper, head=[0, 0, 0], tail=[0, 1, 0])\n\
    output upper_bone = upper_bone_part\n";

fn tube(extra: &str) -> String {
    format!(
        "{BONES}pipe = solid.cylinder@1(radius=0.2, height=2, segments=12)\n\
         skinned = rig.skin@1(pipe, bones=\"lower_bone; upper_bone\"{extra})\noutput tube = skinned\n"
    )
}

#[test]
fn a_tube_skinned_to_two_bones_follows_the_nearer_one() {
    let evaluation = evaluate(&tube("")).unwrap();
    let part = evaluation.parts.iter().find(|p| p.name == "tube").unwrap();
    let skin = part.skin.as_ref().unwrap();
    assert_eq!(skin.bones, ["lower_bone", "upper_bone"]);
    assert_eq!(skin.joints.len(), part.positions.len());
    assert_eq!(skin.weights.len(), part.positions.len());
    let mut lower = 0;
    let mut upper = 0;
    for ((position, joints), weights) in part.positions.iter().zip(&skin.joints).zip(&skin.weights)
    {
        assert!(
            (weights.iter().sum::<f32>() - 1.0).abs() < 1e-5,
            "weights sum to one"
        );
        let strongest = joints[0];
        if position[1] < -0.9 {
            assert_eq!(strongest, 0, "the bottom ring follows the lower bone");
            lower += 1;
        } else if position[1] > 0.9 {
            assert_eq!(strongest, 1, "the top ring follows the upper bone");
            upper += 1;
        }
    }
    assert!(lower > 0 && upper > 0, "both rings were checked");
    assert!(
        evaluation
            .warnings
            .iter()
            .all(|w| !w.code.starts_with("rig.")),
        "{:?}",
        evaluation.warnings
    );
    // The bones themselves stay ordinary parts that carry their segments.
    let bone = evaluation
        .parts
        .iter()
        .find(|p| p.name == "upper_bone")
        .unwrap();
    assert_eq!(bone.bone.unwrap().tail, [0.0, 1.0, 0.0]);
    assert!(bone.skin.is_none());
}

/// A tall box with its four vertical edges cut into eight pieces, skinned to the two bones: it has rings of vertices
/// all the way up, so the weights should change gradually from the lower bone to the upper one.
const SUBDIVIDED_TUBE: &str = "tall = solid.box@1(size=[0.4, 2, 0.4])\n\
    cage = poly.from_solid@1(tall)\n\
    flat = poly.merge_coplanar@1(cage)\n\
    uprights = poly.select@1(flat, element=\"edge\", by=\"box\", box_min=[-1, -0.2, -1], box_max=[1, 0.2, 1])\n\
    cut = poly.subdivide_edges@1(flat, uprights, cuts=7)\n\
    skinned = rig.skin@1(cut, bones=\"lower_bone; upper_bone\")\n\
    output tube = skinned\n";

#[test]
fn a_subdivided_tube_blends_from_one_bone_to_the_other_along_its_length() {
    let evaluation = evaluate(&format!("{BONES}{SUBDIVIDED_TUBE}")).unwrap();
    let part = evaluation.parts.iter().find(|p| p.name == "tube").unwrap();
    let skin = part.skin.as_ref().unwrap();
    // The share of the upper bone, by height: zero at the bottom, one at the top, rising steadily.
    let mut share: Vec<(f32, f32)> = part
        .positions
        .iter()
        .zip(&skin.joints)
        .zip(&skin.weights)
        .map(|((p, j), w)| {
            (
                p[1],
                (0..4).filter(|k| j[*k] == 1).map(|k| w[k]).sum::<f32>(),
            )
        })
        .collect();
    share.sort_by(|a, b| a.0.total_cmp(&b.0));
    let heights: std::collections::BTreeSet<i32> = share
        .iter()
        .map(|(y, _)| (y * 1000.0).round() as i32)
        .collect();
    assert!(
        heights.len() >= 8,
        "the cuts added rings: {} heights",
        heights.len()
    );
    assert!(
        share[0].1 < 0.05 && share[share.len() - 1].1 > 0.95,
        "the ends follow their own bones"
    );
    assert!(
        share.iter().any(|(_, s)| (0.2..0.8).contains(s)),
        "some rings are a real blend of both bones"
    );
    for pair in share.windows(2) {
        assert!(
            pair[1].1 + 0.05 >= pair[0].1,
            "the upper bone's share never falls as you go up: {pair:?}"
        );
    }
}

#[test]
fn influences_one_gives_rigid_pieces() {
    let evaluation = evaluate(&tube(", influences=1")).unwrap();
    let skin = evaluation
        .parts
        .iter()
        .find(|p| p.name == "tube")
        .unwrap()
        .skin
        .clone()
        .unwrap();
    assert!(
        skin.weights
            .iter()
            .all(|w| (w[0] - 1.0).abs() < 1e-6 && w[1] == 0.0),
        "every vertex follows exactly one bone"
    );
}

#[test]
fn weights_are_the_same_every_time() {
    let a = evaluate(&tube("")).unwrap();
    let b = evaluate(&tube("")).unwrap();
    let skin = |e: &Evaluation| {
        e.parts
            .iter()
            .find(|p| p.name == "tube")
            .unwrap()
            .skin
            .clone()
            .unwrap()
    };
    assert_eq!(skin(&a).joints, skin(&b).joints);
    assert_eq!(skin(&a).weights, skin(&b).weights);
}

#[test]
fn painted_stamps_move_weight_to_a_bone_only_near_the_brush() {
    let plain = evaluate(&tube("")).unwrap();
    let painted = evaluate(&tube(", paint=\"lower_bone 0 1 0 0.5 1\"")).unwrap();
    let skin = |e: &Evaluation| {
        let part = e.parts.iter().find(|p| p.name == "tube").unwrap();
        (part.positions.clone(), part.skin.clone().unwrap())
    };
    let ((positions, before), (_, after)) = (skin(&plain), skin(&painted));
    let share = |s: &ve_modeling::rig::Skinning, i: usize, bone: u16| {
        (0..4)
            .filter(|k| s.joints[i][*k] == bone)
            .map(|k| s.weights[i][k])
            .sum::<f32>()
    };
    let (mut near, mut far) = (0, 0);
    for (i, p) in positions.iter().enumerate() {
        assert!((after.weights[i].iter().sum::<f32>() - 1.0).abs() < 1e-5);
        let d = (p[0] * p[0] + (p[1] - 1.0).powi(2) + p[2] * p[2]).sqrt();
        if d < 0.3 {
            assert!(
                share(&after, i, 0) > share(&before, i, 0),
                "under the brush the lower bone gains: {p:?}"
            );
            near += 1;
        } else if d > 0.5 {
            assert_eq!(before.joints[i], after.joints[i]);
            assert_eq!(
                before.weights[i], after.weights[i],
                "beyond the radius nothing changes: {p:?}"
            );
            far += 1;
        }
    }
    assert!(near > 0 && far > 0, "both regions were checked");
    let ghost = evaluate(&tube(", paint=\"ghost 0 0 0 1 1\"")).unwrap();
    assert!(
        ghost
            .warnings
            .iter()
            .any(|w| w.code == "rig.paint_bone_missing"),
        "{:?}",
        ghost.warnings
    );
    assert!(
        evaluate(&tube(", paint=\"lower_bone 0 0 0 0 1\"")).is_err(),
        "a zero radius is refused"
    );
}

#[test]
fn the_blend_mode_is_kept_on_the_skin() {
    let skin = |extra: &str| {
        evaluate(&tube(extra))
            .unwrap()
            .parts
            .iter()
            .find(|p| p.name == "tube")
            .unwrap()
            .skin
            .clone()
            .unwrap()
    };
    assert_eq!(skin("").blend, ve_modeling::rig::Blend::Linear);
    assert_eq!(
        skin(", blend=\"dual_quaternion\"").blend,
        ve_modeling::rig::Blend::DualQuaternion
    );
}

#[test]
fn bad_skins_are_dropped_with_a_reason() {
    let missing = format!(
        "{BONES}pipe = solid.cylinder@1(radius=0.2, height=2, segments=12)\n\
         skinned = rig.skin@1(pipe, bones=\"lower_bone; ghost\")\noutput tube = skinned\n"
    );
    let evaluation = evaluate(&missing).unwrap();
    let skin = evaluation
        .parts
        .iter()
        .find(|p| p.name == "tube")
        .unwrap()
        .skin
        .clone()
        .unwrap();
    assert_eq!(skin.bones, ["lower_bone"], "the missing bone is ignored");
    assert!(
        evaluation
            .warnings
            .iter()
            .any(|w| w.code == "rig.bone_missing")
    );

    let nothing = format!(
        "{BONES}pipe = solid.cylinder@1(radius=0.2, height=2, segments=12)\n\
         skinned = rig.skin@1(pipe, bones=\"ghost\")\noutput tube = skinned\n"
    );
    let evaluation = evaluate(&nothing).unwrap();
    assert!(
        evaluation
            .parts
            .iter()
            .find(|p| p.name == "tube")
            .unwrap()
            .skin
            .is_none()
    );
    assert!(
        evaluation
            .warnings
            .iter()
            .any(|w| w.code == "rig.skin_empty")
    );

    let moving = format!(
        "{BONES}pipe = solid.cylinder@1(radius=0.2, height=2, segments=12)\n\
         skinned = rig.skin@1(pipe, bones=\"lower_bone\")\n\
         slide = anim.track@1(skinned, property=\"position\", keys=\"0:0,0,0; 1:1,0,0\")\noutput tube = slide\n"
    );
    let evaluation = evaluate(&moving).unwrap();
    assert!(
        evaluation
            .parts
            .iter()
            .find(|p| p.name == "tube")
            .unwrap()
            .skin
            .is_none()
    );
    assert!(
        evaluation
            .warnings
            .iter()
            .any(|w| w.code == "rig.skin_not_rigid")
    );
}

#[test]
fn skin_and_bone_text_is_validated() {
    for bad in [
        "b = solid.box@1(size=[1, 1, 1])\nr = rig.bone@1(b, head=[0, 0, 0], tail=[0, 0, 0])\noutput o = r\n",
        "b = solid.box@1(size=[1, 1, 1])\nr = rig.skin@1(b, bones=\"\")\noutput o = r\n",
        "b = solid.box@1(size=[1, 1, 1])\nr = rig.skin@1(b, bones=\"a; a\")\noutput o = r\n",
    ] {
        assert!(evaluate(bad).is_err(), "{bad}");
    }
    // The limit was 32 until a body with fingers needed about 55 bones; it is 64 now.
    let many = (0..65)
        .map(|i| format!("b{i}"))
        .collect::<Vec<_>>()
        .join(";");
    let program = format!(
        "b = solid.box@1(size=[1, 1, 1])\nr = rig.skin@1(b, bones=\"{many}\")\noutput o = r\n"
    );
    assert!(evaluate(&program).is_err(), "at most 64 bones");
}
