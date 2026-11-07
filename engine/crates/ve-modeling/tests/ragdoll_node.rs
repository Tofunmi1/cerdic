//! `phys.ragdoll`: a skeleton of bone parts that turns into a ragdoll on command (plan W5). The definition is resolved against the bone parts
//! after evaluation; a skeleton that cannot work is dropped with a reason.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_ragdoll").unwrap(),
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

/// A pelvis, a spine bone above it and a thigh below, each a small box with a bone, the later ones parented to the earlier ones.
fn skeleton(extra: &str, bones: &str) -> String {
    format!(
        "p = solid.box@1(size=[0.1, 0.1, 0.1])\npelvis_b = rig.bone@1(p, head=[0, 1, 0], tail=[0, 1.3, 0])\noutput pelvis = pelvis_b\n\
         s = solid.box@1(size=[0.1, 0.1, 0.1])\nspine_b = rig.bone@1(s, head=[0, 1.3, 0], tail=[0, 1.7, 0])\nspine_c = anim.parent@1(spine_b, parent=\"pelvis\")\noutput spine = spine_c\n\
         t = solid.box@1(size=[0.1, 0.1, 0.1])\nthigh_b = rig.bone@1(t, head=[0.1, 1, 0], tail=[0.1, 0.55, 0])\nthigh_c = anim.parent@1(thigh_b, parent=\"pelvis\")\noutput thigh = thigh_c\n\
         k = solid.box@1(size=[0.1, 0.1, 0.1])\nshin_b = rig.bone@1(k, head=[0.1, 0.55, 0], tail=[0.1, 0.1, 0])\nshin_c = anim.parent@1(shin_b, parent=\"thigh\")\noutput shin = shin_c\n\
         m = solid.sphere@1(radius=0.05)\nr = phys.ragdoll@1(m, name=\"hero\", bones=\"{bones}\"{extra})\noutput marker = r\n"
    )
}

fn marker(evaluation: &Evaluation) -> &ve_modeling::Part {
    evaluation
        .parts
        .iter()
        .find(|p| p.name == "marker")
        .unwrap()
}

#[test]
fn a_skeleton_is_resolved_with_segments_parents_and_the_chosen_limits() {
    let evaluation = evaluate(&skeleton(
        ", swing=70, twist=25, hinges=\"shin\", hinge_limits=[0, 130], density=500",
        "pelvis; spine; thigh; shin",
    ))
    .unwrap();
    let ragdoll = marker(&evaluation)
        .ragdoll
        .as_ref()
        .expect("the ragdoll is resolved");
    assert_eq!(ragdoll.name, "hero");
    assert_eq!(ragdoll.bones, ["pelvis", "spine", "thigh", "shin"]);
    assert_eq!(ragdoll.segments.len(), 4);
    assert_eq!(ragdoll.segments[1].head, [0.0, 1.3, 0.0]);
    assert_eq!(ragdoll.parents, [None, Some(0), Some(0), Some(2)]);
    assert!(
        (ragdoll.swing - 70.0_f32.to_radians()).abs() < 1e-5
            && (ragdoll.twist - 25.0_f32.to_radians()).abs() < 1e-5
    );
    assert_eq!(ragdoll.hinges, [false, false, false, true]);
    assert!((ragdoll.hinge_limits[1] - 130.0_f32.to_radians()).abs() < 1e-5);
    assert_eq!(ragdoll.density, 500.0);
    assert!(
        evaluation
            .warnings
            .iter()
            .all(|w| !w.code.starts_with("phys.ragdoll")),
        "{:?}",
        evaluation.warnings
    );
    // Without a ragdoll nothing is carried.
    let plain = evaluate("s = solid.sphere@1(radius=0.1)\noutput x = s\n").unwrap();
    assert!(plain.parts[0].ragdoll.is_none());
}

#[test]
fn skeletons_that_cannot_work_are_dropped_with_a_reason() {
    for (program, code) in [
        (skeleton("", "pelvis; ghost"), "phys.ragdoll_bone_missing"),
        // A bone listed before its parent.
        (skeleton("", "spine; pelvis; thigh"), "phys.ragdoll_order"),
        // The same bone twice is refused by the node itself, but a bone with no rig.bone is a missing bone.
        (skeleton("", "pelvis; marker"), "phys.ragdoll_bone_missing"),
    ] {
        let evaluation = evaluate(&program).unwrap();
        assert!(
            marker(&evaluation).ragdoll.is_none(),
            "{code}: the ragdoll is dropped"
        );
        assert!(
            evaluation.warnings.iter().any(|w| w.code == code),
            "{code}: {:?}",
            evaluation.warnings
        );
    }
}

#[test]
fn the_node_checks_its_arguments() {
    let refuse = |extra: &str, bones: &str| evaluate(&skeleton(extra, bones)).unwrap_err();
    assert!(refuse("", "").contains("bones"), "no bones");
    assert!(
        refuse("", "pelvis; pelvis").contains("twice"),
        "a bone twice"
    );
    let many = (0..40)
        .map(|i| format!("b{i}"))
        .collect::<Vec<_>>()
        .join(";");
    assert!(refuse("", &many).contains("at most"), "too many bones");
    assert!(
        refuse(", name=\"\"", "pelvis; spine").contains("name"),
        "a ragdoll needs a name"
    );
    // A name used twice in one scene: the second is dropped with a warning (names are how a game finds a ragdoll).
    let two = format!(
        "{}m2 = solid.sphere@1(radius=0.05)\nr2 = phys.ragdoll@1(m2, name=\"hero\", bones=\"pelvis; spine\")\noutput marker2 = r2\n",
        skeleton("", "pelvis; spine")
    );
    let evaluation = evaluate(&two).unwrap();
    assert!(
        evaluation
            .warnings
            .iter()
            .any(|w| w.code == "phys.ragdoll_name"),
        "{:?}",
        evaluation.warnings
    );
    assert_eq!(
        evaluation
            .parts
            .iter()
            .filter(|p| p.ragdoll.is_some())
            .count(),
        1
    );
}
