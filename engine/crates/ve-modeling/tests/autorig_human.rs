//! The human autorig macro (`kits/autorig_human.txt`): a rigged, skinned character from a few lines. Written before the macro was tuned; do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const PRELUDE: &str = include_str!("../kits/autorig_human.txt");

fn pack() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    ve_modeling::set_human_pack(&std::fs::read(path).unwrap()).unwrap();
}

fn evaluate(program: &str) -> Result<Evaluation, (String, String)> {
    pack();
    let source = format!("{PRELUDE}\n{program}\n");
    let commands = parse_text(&builtin_registry(), &source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_autorig").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

const SIMPLE: &str = "body = body.human@1()\nhuman_rig(\"body\", 0, \"\")\nhuman_skin(\"body\", \"person\", 0, 0.86, 0.66, 0.52)";
const WITH_HANDS: &str = "body = body.human@1()\nhuman_rig(\"body\", 1, \"\")\nhuman_skin(\"body\", \"person\", 1, 0.86, 0.66, 0.52)";

#[test]
fn three_lines_make_a_skinned_character_with_every_bone_a_part_of_its_own() {
    let result = evaluate(SIMPLE).unwrap_or_else(|e| panic!("{e:?}"));
    for bone in [
        "root",
        "spine01",
        "spine05",
        "neck03",
        "head",
        "clavicle_l",
        "upperarm01_r",
        "wrist_l",
        "foot_r",
        "toe1_1_l",
        "pelvis_r",
        "lowerleg02_l",
    ] {
        assert!(
            result
                .parts
                .iter()
                .any(|p| p.name == bone && p.bone.is_some()),
            "bone part {bone} is missing"
        );
    }
    let person = result.parts.iter().find(|p| p.name == "person").unwrap();
    let skin = person.skin.as_ref().unwrap();
    assert_eq!(skin.bones.len(), 34, "the listed bones");
    assert!(person.positions.len() > 10_000, "the body's surface");
    assert_eq!(
        result.parts.iter().filter(|p| p.bone.is_some()).count(),
        1 + 5 + 3 + 1 + 2 * 7 + 2 * 7,
        "root, spine, neck, head, two arms, two legs"
    );
    assert!(
        result
            .warnings
            .iter()
            .all(|w| !w.code.starts_with("rig.") && !w.code.starts_with("hierarchy.")),
        "{:?}",
        result.warnings
    );
}

#[test]
fn the_bones_hang_in_the_pack_s_hierarchy() {
    let result = evaluate(SIMPLE).unwrap();
    let parent = |name: &str| {
        result
            .parts
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .parent
            .clone()
    };
    assert_eq!(parent("spine05").as_deref(), Some("root"));
    assert_eq!(parent("neck01").as_deref(), Some("spine01"));
    assert_eq!(parent("clavicle_r").as_deref(), Some("spine01"));
    assert_eq!(parent("lowerarm01_l").as_deref(), Some("upperarm02_l"));
    assert_eq!(parent("foot_l").as_deref(), Some("lowerleg02_l"));
    assert_eq!(parent("pelvis_l").as_deref(), Some("root"));
}

#[test]
fn with_hands_the_fingers_are_bones_and_skinned_within_the_limit() {
    let result = evaluate(WITH_HANDS).unwrap_or_else(|e| panic!("{e:?}"));
    for bone in [
        "finger1_1_l",
        "finger5_3_r",
        "metacarpal2_l",
        "metacarpal4_r",
    ] {
        assert!(
            result
                .parts
                .iter()
                .any(|p| p.name == bone && p.bone.is_some()),
            "bone part {bone} is missing"
        );
    }
    let skin = result
        .parts
        .iter()
        .find(|p| p.name == "person")
        .unwrap()
        .skin
        .as_ref()
        .unwrap();
    assert_eq!(skin.bones.len(), 64);
    assert_eq!(
        result
            .parts
            .iter()
            .find(|p| p.name == "finger3_2_l")
            .unwrap()
            .parent
            .as_deref(),
        Some("finger3_1_l")
    );
    assert_eq!(
        result
            .parts
            .iter()
            .find(|p| p.name == "finger3_1_l")
            .unwrap()
            .parent
            .as_deref(),
        Some("metacarpal2_l")
    );
}

#[test]
fn a_bone_can_be_animated_by_writing_its_own_lines_between_the_macro_calls() {
    // The macro owns the output names, so an animated bone is written by hand instead of by the macro: here the head nods, and the rest is the macro's.
    let program = "body = body.human@1()\nhuman_rig(\"body\", 0, \"head\")\nhb = body.bone@1(body, bone=\"head\")\nhp = anim.parent@1(hb, parent=\"neck03\")\nnod = anim.track@1(hp, property=\"rotation\", keys=\"0:0,0,0; 1:20,0,0\", pivot_at=\"head\")\noutput head = nod\nhuman_skin(\"body\", \"person\", 0, 0.86, 0.66, 0.52)";
    let result = evaluate(program).unwrap_or_else(|e| panic!("{e:?}"));
    let head = result.parts.iter().find(|p| p.name == "head").unwrap();
    assert_eq!(head.animation.len(), 1, "the head carries the track");
    assert!(
        result
            .parts
            .iter()
            .any(|p| p.name == "spine01" && p.animation.is_empty()),
        "the rest of the rig is the macro's"
    );
}

#[test]
fn human_ik_gives_each_limb_a_target_it_reaches_for_at_the_wrist_and_ankle() {
    let program = format!("{SIMPLE}\nhuman_ik(\"l\")\nhuman_ik(\"r\")");
    let result = evaluate(&program).unwrap_or_else(|e| panic!("{e:?}"));
    for (target, bones, near_bone) in [
        ("reach_hand_l", 4, "wrist_l"),
        ("reach_foot_r", 4, "foot_r"),
        ("reach_hand_r", 4, "wrist_r"),
        ("reach_foot_l", 4, "foot_l"),
    ] {
        let part = result
            .parts
            .iter()
            .find(|p| p.name == target)
            .unwrap_or_else(|| panic!("{target} is missing"));
        let chain = part
            .ik
            .as_ref()
            .unwrap_or_else(|| panic!("{target} has no chain; warnings: {:?}", result.warnings));
        assert_eq!(
            chain.segments.len(),
            bones,
            "{target} reaches with {bones} bones"
        );
        // The target starts at the joint it stands for.
        let head = result
            .parts
            .iter()
            .find(|p| p.name == near_bone)
            .unwrap()
            .bone
            .unwrap()
            .head;
        let at = part.positions.iter().fold([0.0_f32; 3], |sum, p| {
            [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]]
        });
        let n = part.positions.len() as f32;
        let centre = [at[0] / n, at[1] / n, at[2] / n];
        let off = ((centre[0] - head[0]).powi(2)
            + (centre[1] - head[1]).powi(2)
            + (centre[2] - head[2]).powi(2))
        .sqrt();
        assert!(off < 0.02, "{target} sits {off} m from {near_bone}");
    }
    assert!(
        result.warnings.iter().all(|w| !w.code.starts_with("rig.")),
        "{:?}",
        result.warnings
    );
}

#[test]
fn the_rig_macro_is_deterministic() {
    let a = evaluate(SIMPLE).unwrap();
    let b = evaluate(SIMPLE).unwrap();
    let skin = |r: &Evaluation| {
        r.parts
            .iter()
            .find(|p| p.name == "person")
            .unwrap()
            .skin
            .clone()
            .unwrap()
    };
    assert_eq!(skin(&a).joints, skin(&b).joints);
    assert_eq!(skin(&a).weights, skin(&b).weights);
}
