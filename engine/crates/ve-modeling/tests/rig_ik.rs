//! `rig.ik`: a target part reached by a chain of bone parts. The chain is resolved against the parts after evaluation, and
//! a chain that cannot work is dropped with a reason.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_ik").unwrap(),
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

/// An arm of two one-metre bones along +X, the forearm parented to the upper arm, and a target ball.
fn arm(forearm_head: &str, parented: bool, bones: &str) -> String {
    let parent = if parented {
        "forearm_parented = anim.parent@1(forearm_bone, parent=\"upper_arm\")\n"
    } else {
        "forearm_parented = anim.track@1(forearm_bone, property=\"position\", keys=\"0:0,0,0; 1:0,0,0\")\n"
    };
    format!(
        "u = solid.box@1(size=[0.1, 0.1, 0.1])\nupper_bone = rig.bone@1(u, head=[0, 0, 0], tail=[1, 0, 0])\noutput upper_arm = upper_bone\n\
         f = solid.box@1(size=[0.1, 0.1, 0.1])\nforearm_bone = rig.bone@1(f, head={forearm_head}, tail=[2, 0, 0])\n{parent}\
         output forearm = forearm_parented\n\
         ball = solid.sphere@1(radius=0.1)\ngoal = rig.ik@1(ball, bones=\"{bones}\", iterations=48)\noutput target = goal\n"
    )
}

#[test]
fn a_connected_parented_chain_is_resolved_with_its_segments() {
    let evaluation = evaluate(&arm("[1, 0, 0]", true, "upper_arm; forearm")).unwrap();
    let target = evaluation
        .parts
        .iter()
        .find(|p| p.name == "target")
        .unwrap();
    let chain = target.ik.as_ref().unwrap();
    assert_eq!(chain.bones, ["upper_arm", "forearm"]);
    assert_eq!(chain.iterations, 48);
    assert_eq!(chain.segments.len(), 2);
    assert_eq!(chain.segments[1].head, [1.0, 0.0, 0.0]);
    assert!(
        evaluation
            .warnings
            .iter()
            .all(|w| !w.code.starts_with("rig.ik")),
        "{:?}",
        evaluation.warnings
    );
}

#[test]
fn chains_that_cannot_work_are_dropped_with_a_reason() {
    for (program, code) in [
        (
            arm("[1, 0, 0]", false, "upper_arm; forearm"),
            "rig.ik_unparented",
        ),
        (arm("[1.5, 0, 0]", true, "upper_arm; forearm"), "rig.ik_gap"),
        (
            arm("[1, 0, 0]", true, "upper_arm; ghost"),
            "rig.ik_bone_missing",
        ),
        (
            arm("[1, 0, 0]", true, "upper_arm; target"),
            "rig.ik_bone_missing",
        ),
    ] {
        let evaluation = evaluate(&program).unwrap();
        assert!(
            evaluation
                .parts
                .iter()
                .find(|p| p.name == "target")
                .unwrap()
                .ik
                .is_none(),
            "{code}: the chain is dropped"
        );
        assert!(
            evaluation.warnings.iter().any(|w| w.code == code),
            "{code}: {:?}",
            evaluation.warnings
        );
    }
}

#[test]
fn the_two_bone_solver_carries_its_pole_soften_and_twist_and_needs_exactly_two_bones() {
    let program = |extra: &str, bones: &str| {
        arm("[1, 0, 0]", true, bones).replace(
            "iterations=48",
            &format!("iterations=48, solver=\"two_bone\"{extra}"),
        )
    };
    // Angles are written in degrees in the language and kept in radians.
    let evaluation = evaluate(&program(
        ", pole=[0, 1, 0], soften=0.3, twist=30",
        "upper_arm; forearm",
    ))
    .unwrap();
    let target = evaluation
        .parts
        .iter()
        .find(|p| p.name == "target")
        .unwrap();
    let settings = target.ik.as_ref().unwrap().two_bone.as_ref().unwrap();
    assert_eq!(settings.pole, [0.0, 1.0, 0.0]);
    assert!(
        (settings.soften - 0.3).abs() < 1e-6
            && (settings.twist - 30.0_f32.to_radians()).abs() < 1e-5,
        "{settings:?}"
    );
    // The default solver is the general one and carries no two-bone settings.
    let plain = evaluate(&arm("[1, 0, 0]", true, "upper_arm; forearm")).unwrap();
    let plain = plain.parts.iter().find(|p| p.name == "target").unwrap();
    assert!(plain.ik.as_ref().unwrap().two_bone.is_none());
    // Not two bones: refused with the fix named.
    let message = evaluate(&program("", "upper_arm")).unwrap_err();
    assert!(
        message.contains("two_bone") || message.contains("two bones"),
        "{message}"
    );
}

#[test]
fn rig_aim_makes_a_target_that_one_bone_looks_at() {
    let program = |extra: &str| {
        format!(
            "h = solid.box@1(size=[0.1, 0.1, 0.1])\nhead_bone = rig.bone@1(h, head=[0, 1.7, 0], tail=[0, 1.7, 0.2])\noutput head = head_bone\n\
             ball = solid.sphere@1(radius=0.1)\ngoal = rig.aim@1(ball, bone=\"head\"{extra})\noutput target = goal\n"
        )
    };
    let evaluation = evaluate(&program(", pole=[0, 1, 0], twist=10, weight=0.8")).unwrap();
    let target = evaluation
        .parts
        .iter()
        .find(|p| p.name == "target")
        .unwrap();
    let chain = target.ik.as_ref().unwrap();
    assert_eq!(chain.bones, ["head"]);
    assert_eq!(
        chain.segments.len(),
        1,
        "the bone is resolved against its part"
    );
    let aim = chain.aim.as_ref().unwrap();
    assert_eq!(aim.pole, [0.0, 1.0, 0.0]);
    assert!(
        (aim.twist - 10.0_f32.to_radians()).abs() < 1e-5 && (aim.weight - 0.8).abs() < 1e-6,
        "{aim:?}"
    );
    // A target that is already an ik target, or a bone that does not exist, is refused or dropped with a reason.
    let twice = format!(
        "{}r2 = rig.aim@1(goal, bone=\"head\")\noutput t2 = r2\n",
        program("")
    );
    assert!(evaluate(&twice).is_err(), "one chain per target");
    let ghost = evaluate(&program("").replace("bone=\"head\"", "bone=\"ghost\"")).unwrap();
    assert!(
        ghost
            .parts
            .iter()
            .find(|p| p.name == "target")
            .unwrap()
            .ik
            .is_none()
            && ghost
                .warnings
                .iter()
                .any(|w| w.code == "rig.ik_bone_missing"),
        "{:?}",
        ghost.warnings
    );
}

#[test]
fn the_node_checks_its_bones() {
    for bad in [
        "b = solid.box@1(size=[1, 1, 1])\nr = rig.ik@1(b, bones=\"\")\noutput o = r\n",
        "b = solid.box@1(size=[1, 1, 1])\nr = rig.ik@1(b, bones=\"a; a\")\noutput o = r\n",
    ] {
        assert!(evaluate(bad).is_err(), "{bad}");
    }
    let nine = (0..9)
        .map(|i| format!("b{i}"))
        .collect::<Vec<_>>()
        .join(";");
    assert!(
        evaluate(&format!(
            "b = solid.box@1(size=[1, 1, 1])\nr = rig.ik@1(b, bones=\"{nine}\")\noutput o = r\n"
        ))
        .is_err(),
        "at most 8 bones"
    );
}
