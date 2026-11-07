//! `pivot_at` on `anim.track` and `anim.wave`: a part that is a bone turns about its own joint (`head`) or its far end (`tail`) without the joint being
//! typed. `none` keeps the typed `pivot`; a part that is not a bone is refused with the fix. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
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
        transaction: TransactionId::parse("t_pivot_at").unwrap(),
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

fn part<'a>(result: &'a Evaluation, name: &str) -> &'a ve_modeling::Part {
    result.parts.iter().find(|p| p.name == name).unwrap()
}

const ARM: &str = "\
body = body.human@1()
b = body.bone@1(body, bone=\"upperarm01_l\")
";

#[test]
fn head_and_tail_take_the_joint_and_the_far_end_of_the_bone() {
    let source = format!(
        "{ARM}t_head = anim.track@1(b, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,30\", pivot_at=\"head\")\nt_tail = anim.track@1(b, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,30\", pivot_at=\"tail\")\noutput arm_head = t_head\noutput arm_tail = t_tail\n"
    );
    let result = evaluate(&source).unwrap();
    let head = part(&result, "arm_head");
    let segment = head.bone.unwrap();
    assert_eq!(head.animation[0].pivot, segment.head);
    assert_eq!(part(&result, "arm_tail").animation[0].pivot, segment.tail);
    assert_ne!(segment.head, segment.tail);
}

#[test]
fn the_typed_pivot_is_used_when_pivot_at_is_none_or_left_out() {
    let source = format!(
        "{ARM}t = anim.track@1(b, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,30\", pivot=[0.5, 1.5, 0.25])\noutput arm = t\n"
    );
    let result = evaluate(&source).unwrap();
    assert_eq!(part(&result, "arm").animation[0].pivot, [0.5, 1.5, 0.25]);
}

#[test]
fn pivot_at_wins_over_a_typed_pivot() {
    let source = format!(
        "{ARM}t = anim.track@1(b, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,30\", pivot=[9, 9, 9], pivot_at=\"head\")\noutput arm = t\n"
    );
    let result = evaluate(&source).unwrap();
    let arm = part(&result, "arm");
    assert_eq!(arm.animation[0].pivot, arm.bone.unwrap().head);
}

#[test]
fn a_wave_takes_the_joint_too() {
    let source = format!(
        "{ARM}w = anim.wave@1(b, property=\"rotation\", amplitude=[0, 0, 10], period=2, pivot_at=\"head\")\noutput arm = w\n"
    );
    let result = evaluate(&source).unwrap();
    let arm = part(&result, "arm");
    assert_eq!(arm.animation[0].pivot, arm.bone.unwrap().head);
}

#[test]
fn a_part_that_is_not_a_bone_is_refused_with_the_fix() {
    let error = evaluate(
        "b = solid.box@1(size=[1, 1, 1])\nt = anim.track@1(b, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,30\", pivot_at=\"head\")\noutput o = t\n",
    )
    .unwrap_err();
    assert!(
        error.contains("anim.pivot_needs_bone") || error.contains("needs the part to be a bone"),
        "{error}"
    );
}

#[test]
fn a_bone_chain_keeps_each_joint_through_anim_parent() {
    let source = "\
body = body.human@1()
a = body.bone@1(body, bone=\"upperarm01_l\")
f = body.bone@1(body, bone=\"lowerarm01_l\")
fp = anim.parent@1(f, parent=\"arm\")
ta = anim.track@1(a, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,30\", pivot_at=\"head\")
tf = anim.track@1(fp, property=\"rotation\", keys=\"0:0,0,0; 1:-20,0,0\", pivot_at=\"head\")
output arm = ta
output fore = tf
";
    let result = evaluate(source).unwrap();
    let arm = part(&result, "arm");
    let fore = part(&result, "fore");
    assert_eq!(arm.animation[0].pivot, arm.bone.unwrap().head);
    assert_eq!(fore.animation[0].pivot, fore.bone.unwrap().head);
    assert_ne!(arm.animation[0].pivot, fore.animation[0].pivot);
}
