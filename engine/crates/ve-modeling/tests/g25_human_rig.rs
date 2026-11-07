//! A person that can be animated: `body.bone` makes a bone part at a named joint of the body (its head where the joint is, its tail where the next joint is), and `body.skin`
//! skins the body to the bones you list with the pack's own weights (the strongest four per vertex, a bone you did not list handing its share to the nearest listed ancestor).
//! Bone names are the pack's, lowercase with `.` and `-` as `_`: `upperarm01_l`, `finger2_1_r`. Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality, set_human_pack};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g25_rig").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

const BONES: &str = "\
body = body.human@1()
root_n = body.bone@1(body, bone=\"root\")
spine_n = body.bone@1(body, bone=\"spine01\")
arm_n = body.bone@1(body, bone=\"upperarm01_l\")
fore_n = body.bone@1(body, bone=\"lowerarm01_l\")
fore_p = anim.parent@1(fore_n, parent=\"upperarm01_l\")
output root = root_n
output spine01 = spine_n
output upperarm01_l = arm_n
output lowerarm01_l = fore_p
";

fn part<'a>(result: &'a Evaluation, name: &str) -> &'a ve_modeling::Part {
    result
        .parts
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| {
            panic!(
                "no part {name}: {:?}",
                result.parts.iter().map(|p| &p.name).collect::<Vec<_>>()
            )
        })
}

#[test]
fn a_bone_part_starts_at_its_joint_and_ends_at_the_next_joint() {
    let result = evaluate(BONES).unwrap();
    let arm = part(&result, "upperarm01_l")
        .bone
        .expect("the part is a bone");
    // The pack's left upper arm starts at x 0.16 and the next joint is 5 cm further out and 6 cm down; the person stands with feet at y = 0.
    assert!((arm.head[0] - 0.1616).abs() < 0.03, "head x {:?}", arm.head);
    assert!(
        arm.head[1] > 1.1 && arm.head[1] < 1.5,
        "head height {:?}",
        arm.head
    );
    assert!(
        arm.tail[0] > arm.head[0] && arm.tail[1] < arm.head[1],
        "the bone runs outward and down: {arm:?}"
    );
    let fore = part(&result, "lowerarm01_l").bone.unwrap();
    assert!(
        fore.head[0] > arm.head[0] + 0.1,
        "the forearm starts further out: {fore:?}"
    );
    assert_eq!(
        part(&result, "lowerarm01_l").parent.as_deref(),
        Some("upperarm01_l"),
        "bones join with anim.parent like any part"
    );
}

#[test]
fn a_bone_part_is_small_enough_to_hide_inside_the_body() {
    let result = evaluate(BONES).unwrap();
    let arm = part(&result, "upperarm01_l");
    let (lo, hi) = arm
        .positions
        .iter()
        .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        });
    assert!(
        (0..3).all(|i| hi[i] - lo[i] < 0.02),
        "bone part size {lo:?} {hi:?}"
    );
}

#[test]
fn an_unknown_bone_names_the_valid_ones() {
    let error =
        evaluate("body = body.human@1()\nb = body.bone@1(body, bone=\"elbow\")\noutput b = b\n")
            .unwrap_err();
    assert!(
        error.contains("elbow") && error.contains("upperarm01_l"),
        "{error}"
    );
}

const SKIN: &str = "\
body = body.human@1()
root_n = body.bone@1(body, bone=\"root\")
arm_n = body.bone@1(body, bone=\"upperarm01_l\")
hand_n = body.bone@1(body, bone=\"wrist_l\")
tip_n = body.bone@1(body, bone=\"finger3_3_l\")
arm_p = anim.parent@1(arm_n, parent=\"root\")
hand_p = anim.parent@1(hand_n, parent=\"upperarm01_l\")
tip_p = anim.parent@1(tip_n, parent=\"wrist_l\")
skin = body.skin@1(body, bones=\"root; upperarm01_l; wrist_l; finger3_3_l\")
output root = root_n
output upperarm01_l = arm_p
output wrist_l = hand_p
output finger3_3_l = tip_p
output person = skin
";

#[test]
fn the_body_is_skinned_with_the_packs_weights_to_the_bones_that_were_listed() {
    let result = evaluate(SKIN).unwrap();
    let person = part(&result, "person");
    let skin = person.skin.as_ref().expect("the body is skinned");
    assert_eq!(
        skin.bones,
        vec!["root", "upperarm01_l", "wrist_l", "finger3_3_l"]
    );
    assert_eq!(skin.joints.len(), person.positions.len());
    assert_eq!(skin.weights.len(), person.positions.len());
    for (k, w) in skin.weights.iter().enumerate() {
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "vertex {k} weights sum to {sum}");
    }
    // The vertex furthest down is a toe: its bones are not listed, so it follows the root, the nearest listed ancestor.
    let lowest = person
        .positions
        .iter()
        .enumerate()
        .min_by(|a, b| a.1[1].total_cmp(&b.1[1]))
        .unwrap()
        .0;
    assert_eq!(skin.joints[lowest][0], 0, "a toe follows the root");
    assert!((skin.weights[lowest][0] - 1.0).abs() < 1e-4);
    // The vertex furthest out along the left arm belongs to the hand or its fingertip, not the root.
    let outermost = person
        .positions
        .iter()
        .enumerate()
        .max_by(|a, b| a.1[0].total_cmp(&b.1[0]))
        .unwrap()
        .0;
    let (j, w) = (skin.joints[outermost], skin.weights[outermost]);
    let on_hand: f32 = (0..4).filter(|&k| j[k] >= 2).map(|k| w[k]).sum();
    assert!(
        on_hand > 0.8,
        "the fingertips follow the hand and fingertip bones: {j:?} {w:?}"
    );
}

#[test]
fn posing_a_listed_bone_moves_the_vertices_it_owns() {
    use ve_modeling::rig::deform;
    let result = evaluate(SKIN).unwrap();
    let person = part(&result, "person");
    let skin = person.skin.as_ref().unwrap();
    let outermost = person
        .positions
        .iter()
        .enumerate()
        .max_by(|a, b| a.1[0].total_cmp(&b.1[0]))
        .unwrap()
        .0;
    // Raise the arm bone 90 degrees about the shoulder (the bone head); everything else stays.
    let arm = part(&result, "upperarm01_l").bone.unwrap();
    let c = std::f32::consts::FRAC_PI_2.cos();
    let s = std::f32::consts::FRAC_PI_2.sin();
    // Rotation about Z through the head, column-major.
    let h = arm.head;
    let rot = [
        c,
        s,
        0.0,
        0.0,
        -s,
        c,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        h[0] - c * h[0] + s * h[1],
        h[1] - s * h[0] - c * h[1],
        0.0,
        1.0,
    ];
    let ident = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let poses = [ident, rot, rot, rot];
    let (moved, _) = deform(
        person.positions[outermost],
        [0.0, 1.0, 0.0],
        skin.joints[outermost],
        skin.weights[outermost],
        &poses,
    );
    let before = person.positions[outermost];
    let distance = ((moved[0] - before[0]).powi(2) + (moved[1] - before[1]).powi(2)).sqrt();
    assert!(
        distance > 0.3,
        "the fingertip swung with the arm: {before:?} to {moved:?}"
    );
}

/// A skinned person with every bone at rest must look exactly like the body did: no vertex may move.
#[test]
fn a_skinned_body_with_every_bone_at_rest_does_not_move() {
    use ve_modeling::rig::deform;
    let source = "\
body = body.human@1()
b_root = body.bone@1(body, bone=\"root\")
output root = b_root
b_spine03 = body.bone@1(body, bone=\"spine03\")
p_spine03 = anim.parent@1(b_spine03, parent=\"root\")
output spine03 = p_spine03
b_spine01 = body.bone@1(body, bone=\"spine01\")
p_spine01 = anim.parent@1(b_spine01, parent=\"spine03\")
output spine01 = p_spine01
b_head = body.bone@1(body, bone=\"head\")
p_head = anim.parent@1(b_head, parent=\"spine01\")
output head = p_head
b_arm_l = body.bone@1(body, bone=\"upperarm01_l\")
p_arm_l = anim.parent@1(b_arm_l, parent=\"spine01\")
output upperarm01_l = p_arm_l
b_fore_l = body.bone@1(body, bone=\"lowerarm01_l\")
p_fore_l = anim.parent@1(b_fore_l, parent=\"upperarm01_l\")
output lowerarm01_l = p_fore_l
b_arm_r = body.bone@1(body, bone=\"upperarm01_r\")
p_arm_r = anim.parent@1(b_arm_r, parent=\"spine01\")
output upperarm01_r = p_arm_r
b_fore_r = body.bone@1(body, bone=\"lowerarm01_r\")
p_fore_r = anim.parent@1(b_fore_r, parent=\"upperarm01_r\")
output lowerarm01_r = p_fore_r
b_leg_l = body.bone@1(body, bone=\"upperleg01_l\")
p_leg_l = anim.parent@1(b_leg_l, parent=\"root\")
output upperleg01_l = p_leg_l
b_shin_l = body.bone@1(body, bone=\"lowerleg01_l\")
p_shin_l = anim.parent@1(b_shin_l, parent=\"upperleg01_l\")
output lowerleg01_l = p_shin_l
b_foot_l = body.bone@1(body, bone=\"foot_l\")
p_foot_l = anim.parent@1(b_foot_l, parent=\"lowerleg01_l\")
output foot_l = p_foot_l
b_leg_r = body.bone@1(body, bone=\"upperleg01_r\")
p_leg_r = anim.parent@1(b_leg_r, parent=\"root\")
output upperleg01_r = p_leg_r
b_shin_r = body.bone@1(body, bone=\"lowerleg01_r\")
p_shin_r = anim.parent@1(b_shin_r, parent=\"upperleg01_r\")
output lowerleg01_r = p_shin_r
b_foot_r = body.bone@1(body, bone=\"foot_r\")
p_foot_r = anim.parent@1(b_foot_r, parent=\"lowerleg01_r\")
output foot_r = p_foot_r
skinned = body.skin@1(body, bones=\"root; spine03; spine01; head; upperarm01_l; lowerarm01_l; upperarm01_r; lowerarm01_r; upperleg01_l; lowerleg01_l; foot_l; upperleg01_r; lowerleg01_r; foot_r\")
output person = skinned
";
    let result = evaluate(source).unwrap();
    let person = part(&result, "person");
    let skin = person.skin.as_ref().unwrap();
    let ident = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let poses = vec![ident; skin.bones.len()];
    let mut worst = (0.0_f32, 0_usize);
    // The runtime draws dual-quaternion skins through the prepared poses; at rest that must not move anything either.
    let prepared = ve_modeling::rig::prepare_dual(&poses);
    for (k, rest) in person.positions.iter().enumerate() {
        let (moved, _) = ve_modeling::rig::deform_prepared(
            *rest,
            [0.0, 1.0, 0.0],
            skin.joints[k],
            skin.weights[k],
            &prepared,
        );
        let d = ((moved[0] - rest[0]).powi(2)
            + (moved[1] - rest[1]).powi(2)
            + (moved[2] - rest[2]).powi(2))
        .sqrt();
        if d > worst.0 {
            worst = (d, k);
        }
    }
    assert!(
        worst.0 < 1e-3,
        "vertex {} at {:?} moved {} with every bone at rest (joints {:?} weights {:?})",
        worst.1,
        person.positions[worst.1],
        worst.0,
        skin.joints[worst.1],
        skin.weights[worst.1]
    );
}

/// The vertices of a foot follow the leg and the foot, nothing from the arms or the head, on either side.
#[test]
fn the_vertices_of_each_foot_follow_only_the_leg_and_the_foot() {
    let source = "\
body = body.human@1()
b_root = body.bone@1(body, bone=\"root\")
output root = b_root
b_arm_l = body.bone@1(body, bone=\"upperarm01_l\")
p_arm_l = anim.parent@1(b_arm_l, parent=\"root\")
output upperarm01_l = p_arm_l
b_leg_l = body.bone@1(body, bone=\"upperleg01_l\")
p_leg_l = anim.parent@1(b_leg_l, parent=\"root\")
output upperleg01_l = p_leg_l
b_shin_l = body.bone@1(body, bone=\"lowerleg01_l\")
p_shin_l = anim.parent@1(b_shin_l, parent=\"upperleg01_l\")
output lowerleg01_l = p_shin_l
b_foot_l = body.bone@1(body, bone=\"foot_l\")
p_foot_l = anim.parent@1(b_foot_l, parent=\"lowerleg01_l\")
output foot_l = p_foot_l
b_leg_r = body.bone@1(body, bone=\"upperleg01_r\")
p_leg_r = anim.parent@1(b_leg_r, parent=\"root\")
output upperleg01_r = p_leg_r
b_shin_r = body.bone@1(body, bone=\"lowerleg01_r\")
p_shin_r = anim.parent@1(b_shin_r, parent=\"upperleg01_r\")
output lowerleg01_r = p_shin_r
b_foot_r = body.bone@1(body, bone=\"foot_r\")
p_foot_r = anim.parent@1(b_foot_r, parent=\"lowerleg01_r\")
output foot_r = p_foot_r
skinned = body.skin@1(body, bones=\"root; upperarm01_l; upperleg01_l; lowerleg01_l; foot_l; upperleg01_r; lowerleg01_r; foot_r\")
output person = skinned
";
    let result = evaluate(source).unwrap();
    let person = part(&result, "person");
    let skin = person.skin.as_ref().unwrap();
    for (k, p) in person.positions.iter().enumerate() {
        if p[1] > 0.1 {
            continue;
        }
        for slot in 0..4 {
            if skin.weights[k][slot] < 0.01 {
                continue;
            }
            let bone = &skin.bones[skin.joints[k][slot] as usize];
            let allowed: &[&str] = if p[0] > 0.0 {
                &["root", "upperleg01_l", "lowerleg01_l", "foot_l"]
            } else {
                &["root", "upperleg01_r", "lowerleg01_r", "foot_r"]
            };
            assert!(
                allowed.contains(&bone.as_str()),
                "foot vertex {k} at {p:?} follows {bone} ({:?} {:?})",
                skin.joints[k],
                skin.weights[k]
            );
        }
    }
}

/// A world with several people: `prefix` goes in front of every bone name a skin follows, so each person has bone parts of their own
/// (`ken_root`) and two people never share a part. Every node that skins one person takes the same prefix.
#[test]
fn a_prefix_names_the_bones_each_skin_follows_on_every_skinning_node() {
    let source = "\
body = body.human@1()
root_n = body.bone@1(body, bone=\"root\")
arm_n = body.bone@1(body, bone=\"upperarm01_l\")
output ken_root = root_n
output ken_upperarm01_l = arm_n
skinned = body.skin@1(body, bones=\"root; upperarm01_l\", prefix=\"ken_\")
output person = skinned
shirt = body.garment@1(body, cover=\"shirt\", bones=\"root; upperarm01_l\", prefix=\"ken_\")
output shirt = shirt
shoes = body.outfit@1(body, asset=\"shoes02\", bones=\"root; upperarm01_l\", prefix=\"ken_\")
output shoes = shoes
cape_g = solid.box@1(size=[0.4, 0.5, 0.05])
cape_at = xform.move@1(cape_g, by=[0, 1.3, -0.1])
cape = body.skin_to@1(cape_at, body, bones=\"root; upperarm01_l\", prefix=\"ken_\")
output cape = cape
";
    let result = evaluate(source).unwrap();
    for name in ["person", "shirt", "shoes", "cape"] {
        let skin = part(&result, name).skin.as_ref().expect("skinned");
        assert_eq!(skin.bones, vec!["ken_root", "ken_upperarm01_l"], "{name}");
    }
    // `blend` picks how the bones combine; the default is dual quaternions, and "linear" is the renderer's GPU skinning.
    let linear = evaluate(&source.replace(
        "bones=\"root; upperarm01_l\", prefix=\"ken_\")\noutput person",
        "bones=\"root; upperarm01_l\", prefix=\"ken_\", blend=\"linear\")\noutput person",
    ))
    .unwrap();
    assert_eq!(
        part(&linear, "person").skin.as_ref().unwrap().blend,
        ve_modeling::rig::Blend::Linear
    );
    assert_eq!(
        part(&result, "person").skin.as_ref().unwrap().blend,
        ve_modeling::rig::Blend::DualQuaternion
    );
    // Without a prefix the names are the plain ones, as before.
    let plain = evaluate(SKIN).unwrap();
    assert_eq!(
        part(&plain, "person").skin.as_ref().unwrap().bones[0],
        "root"
    );
}
