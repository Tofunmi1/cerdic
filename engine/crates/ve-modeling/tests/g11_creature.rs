//! The creature maker: sliders or a hand-written skeleton tree become one skinned, animated body.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helper unwraps fixtures that are known to be valid and reports failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::creature::{Plan, Sliders, template};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Evaluation {
    let batch = parse_text(&builtin_registry(), text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_creature").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|e| panic!("{e:?}"))
}

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for p in positions {
        for axis in 0..3 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    }
    (low, high)
}

fn make(plan: Plan, sliders: &Sliders) -> (Evaluation, usize) {
    let creature = template(plan, sliders).unwrap();
    let bones: usize = creature
        .bones
        .iter()
        .map(|b| if b.pair { 2 } else { 1 })
        .sum();
    let text = creature.to_text().unwrap();
    (evaluate(&text), bones)
}

#[test]
fn every_plan_makes_a_skinned_rigged_body_of_the_asked_height() {
    for (plan, size) in [
        (Plan::Biped, 1.7),
        (Plan::Quadruped, 2.4),
        (Plan::Serpent, 1.2),
    ] {
        let sliders = Sliders {
            size,
            ..Sliders::default()
        };
        let (evaluation, bones) = make(plan, &sliders);
        let body = evaluation
            .parts
            .iter()
            .find(|p| p.skin.is_some())
            .unwrap_or_else(|| panic!("{plan:?}: no skinned part"));
        assert!(
            body.volume > 0.0 && !body.indices.is_empty(),
            "{plan:?}: empty body"
        );
        let (low, high) = bounds(&body.positions);
        // An upright body is `size` tall; a serpent lying on the ground is `size` long.
        let measured = if plan == Plan::Serpent {
            (0..3).map(|a| high[a] - low[a]).fold(0.0_f32, f32::max)
        } else {
            high[1]
        };
        assert!(
            low[1] > -0.1 * size as f32 && (measured - size as f32).abs() < 0.12 * size as f32,
            "{plan:?}: {low:?} {high:?} for {size}"
        );
        let rig = evaluation.parts.iter().filter(|p| p.bone.is_some()).count();
        assert_eq!(rig, bones, "{plan:?}: one rig bone per skeleton bone");
        assert!(
            evaluation.parts.iter().any(|p| !p.animation.is_empty()),
            "{plan:?}: the bones move"
        );
    }
}

#[test]
fn a_mirrored_body_is_symmetric_left_to_right() {
    let (evaluation, _) = make(Plan::Biped, &Sliders::default());
    let body = evaluation.parts.iter().find(|p| p.skin.is_some()).unwrap();
    let (low, high) = bounds(&body.positions);
    assert!(
        (low[0] + high[0]).abs() < 0.03,
        "x range {low:?} {high:?} is not centred"
    );
}

#[test]
fn sliders_change_the_shape() {
    let tall = make(
        Plan::Biped,
        &Sliders {
            legs: 1.6,
            ..Sliders::default()
        },
    )
    .0;
    let base = make(Plan::Biped, &Sliders::default()).0;
    let width = |e: &Evaluation| {
        let (low, high) = bounds(&e.parts.iter().find(|p| p.skin.is_some()).unwrap().positions);
        high[0] - low[0]
    };
    // Same overall height, longer legs: the torso takes a smaller share, so the body is narrower.
    assert!(
        width(&tall) < width(&base),
        "{} against {}",
        width(&tall),
        width(&base)
    );
    assert!(
        make(
            Plan::Quadruped,
            &Sliders {
                wings: true,
                horns: true,
                ..Sliders::default()
            }
        )
        .0
        .parts
        .iter()
        .filter(|p| p.bone.is_some())
        .count()
            > 14
    );
}

#[test]
fn clips_are_named_and_a_walk_swings_the_legs() {
    let (evaluation, _) = make(Plan::Biped, &Sliders::default());
    let thigh = evaluation
        .parts
        .iter()
        .find(|p| p.name == "biped_thigh_l")
        .unwrap();
    let clips: std::collections::BTreeSet<&str> =
        thigh.animation.iter().map(|t| t.clip.as_str()).collect();
    assert!(clips.contains("") && clips.contains("run"), "{clips:?}");
    let arm = evaluation
        .parts
        .iter()
        .find(|p| p.name == "biped_upper_arm_l")
        .unwrap();
    assert!(
        arm.animation.iter().any(|t| t.clip == "idle"),
        "arms breathe at rest"
    );
}

#[test]
fn every_plan_carries_the_jump_and_fall_clips() {
    for plan in [Plan::Biped, Plan::Quadruped, Plan::Serpent] {
        let (evaluation, _) = make(plan, &Sliders::default());
        let clips: std::collections::BTreeSet<&str> = evaluation
            .parts
            .iter()
            .flat_map(|part| part.animation.iter().map(|track| track.clip.as_str()))
            .collect();
        assert!(clips.contains("jump"), "{plan:?}: no jump clip");
        assert!(clips.contains("fall"), "{plan:?}: no fall clip");
    }
}

#[test]
fn a_leg_has_a_one_shot_jump_and_a_looping_fall_that_are_not_the_walk() {
    for (plan, bone) in [
        (Plan::Biped, "biped_thigh_l"),
        (Plan::Quadruped, "quadruped_fore_upper_l"),
    ] {
        let (evaluation, _) = make(plan, &Sliders::default());
        let leg = evaluation
            .parts
            .iter()
            .find(|part| part.name == bone)
            .unwrap_or_else(|| panic!("{plan:?}: no part {bone}"));
        let clip = |name: &str| {
            leg.animation
                .iter()
                .find(|track| track.clip.as_str() == name)
        };
        let jump = clip("jump").unwrap_or_else(|| panic!("{plan:?}: no jump track"));
        assert!(!jump.looped, "{plan:?}: jump is a one-shot");
        let fall = clip("fall").unwrap_or_else(|| panic!("{plan:?}: no fall track"));
        assert!(fall.looped, "{plan:?}: fall loops");
        assert_eq!(
            fall.keys.first().map(|key| key.value),
            fall.keys.last().map(|key| key.value),
            "{plan:?}: the fall loop joins without a jump"
        );
        // Jump and fall are their own motion, not the walk clip the jump state used to borrow.
        let walk = clip("").unwrap_or_else(|| panic!("{plan:?}: no walk track"));
        assert_ne!(jump.keys, walk.keys, "{plan:?}: jump is not the walk");
        assert_ne!(fall.keys, walk.keys, "{plan:?}: fall is not the walk");
    }
}

#[test]
fn bad_skeletons_are_refused_with_the_fix() {
    let mut creature = template(Plan::Biped, &Sliders::default()).unwrap();
    creature.bones[2].parent = Some("nowhere".to_owned());
    let message = creature.to_text().unwrap_err().to_string();
    assert!(
        message.contains("nowhere") && message.contains("does not exist"),
        "{message}"
    );
    let mut creature = template(Plan::Biped, &Sliders::default()).unwrap();
    creature.bones[4].head[0] = -0.1;
    assert!(
        creature
            .to_text()
            .unwrap_err()
            .to_string()
            .contains("+x side")
    );
    assert!(
        template(
            Plan::Biped,
            &Sliders {
                head: 3.0,
                ..Sliders::default()
            }
        )
        .is_err()
    );
    // A chain of bones deeper than the engine follows is refused, not silently cut.
    let mut deep = template(Plan::Serpent, &Sliders::default()).unwrap();
    // (the chain limit was 8 until a body with fingers needed about 16; it is 20 now)
    let mut extra = deep.bones.last().unwrap().clone();
    for k in 0..22 {
        extra.parent = Some(deep.bones.last().unwrap().name.clone());
        extra.name = format!("seg_extra{k}");
        deep.bones.push(extra.clone());
    }
    assert!(
        deep.to_text()
            .unwrap_err()
            .to_string()
            .contains("below the root")
    );
}

#[test]
fn the_longest_allowed_names_still_make_node_names_the_language_accepts() {
    let mut creature = template(Plan::Biped, &Sliders::default()).unwrap();
    creature.name = "abcdefghijklm".to_owned();
    for bone in &mut creature.bones {
        if bone.name == "upper_arm" {
            "upper_arm_long".clone_into(&mut bone.name);
        }
        if bone.parent.as_deref() == Some("upper_arm") {
            bone.parent = Some("upper_arm_long".to_owned());
        }
    }
    let evaluation = evaluate(&creature.to_text().unwrap());
    assert!(
        evaluation
            .parts
            .iter()
            .any(|p| p.name == "abcdefghijklm_upper_arm_long_l")
    );
    creature.name = "abcdefghijklmn".to_owned();
    assert!(
        creature
            .to_text()
            .unwrap_err()
            .to_string()
            .contains("creature name")
    );
}

#[test]
fn a_creature_can_be_written_as_json() {
    let creature = template(Plan::Serpent, &Sliders::default()).unwrap();
    let json = serde_json::to_string(&creature).unwrap();
    let back: ve_modeling::creature::Creature = serde_json::from_str(&json).unwrap();
    assert_eq!(back.to_text().unwrap(), creature.to_text().unwrap());
}
