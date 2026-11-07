//! `anim.retarget`: clips copied from one bone part to another, scaled and mirrored. Written before the node was tuned; do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_retarget").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A short character's left arm bone with two clips, and a tall character's right arm bone with none.
const RIGS: &str = "box = solid.box@1(size=[0.05, 0.3, 0.05])\n\
src_bone = rig.bone@1(box, head=[0.2, 1.3, 0], tail=[0.2, 1.0, 0])\n\
src_wave = anim.track@1(src_bone, property=\"rotation\", keys=\"0:0,10,20; 1:30,40,50\", pivot_at=\"head\", clip=\"wave\")\n\
src = anim.track@1(src_wave, property=\"position\", keys=\"0:0,0,0; 1:0.1,0.2,0.3\", clip=\"bob\")\n\
output src = src\n\
tall = rig.bone@1(box, head=[-0.3, 1.95, 0], tail=[-0.3, 1.5, 0])\n";

fn tracks(result: &Evaluation, name: &str) -> Vec<ve_modeling::anim::Track> {
    result
        .parts
        .iter()
        .find(|p| p.name == name)
        .unwrap()
        .animation
        .clone()
}

#[test]
fn a_clip_moves_to_the_target_s_joint_and_scales_with_the_target() {
    let result = evaluate(&format!(
        "{RIGS}out = anim.retarget@1(tall, src, scale=1.5)\noutput tall = out\n"
    ))
    .unwrap_or_else(|e| panic!("{e:?}"));
    let copied = tracks(&result, "tall");
    assert_eq!(copied.len(), 2, "both clips arrive");
    let wave = copied.iter().find(|t| t.clip == "wave").unwrap();
    assert_eq!(
        wave.pivot,
        [-0.3, 1.95, 0.0],
        "a rotation about the source joint is now about the target joint"
    );
    assert_eq!(
        wave.keys[1].value,
        [30.0, 40.0, 50.0],
        "angles are not scaled"
    );
    let bob = copied.iter().find(|t| t.clip == "bob").unwrap();
    assert!(
        (bob.keys[1].value[1] - 0.3).abs() < 1e-6 && (bob.keys[1].value[2] - 0.45).abs() < 1e-6,
        "positions scale: {:?}",
        bob.keys[1].value
    );
    assert_eq!(
        tracks(&result, "src").len(),
        2,
        "the source keeps its tracks"
    );
}

#[test]
fn mirror_turns_a_left_clip_into_a_right_one() {
    let result = evaluate(&format!(
        "{RIGS}out = anim.retarget@1(tall, src, mirror=true)\noutput tall = out\n"
    ))
    .unwrap();
    let copied = tracks(&result, "tall");
    let wave = copied.iter().find(|t| t.clip == "wave").unwrap();
    assert_eq!(
        wave.keys[1].value,
        [30.0, -40.0, -50.0],
        "rotations about y and z turn the other way"
    );
    let bob = copied.iter().find(|t| t.clip == "bob").unwrap();
    assert!(
        (bob.keys[1].value[0] + 0.1).abs() < 1e-6 && (bob.keys[1].value[1] - 0.2).abs() < 1e-6,
        "x flips: {:?}",
        bob.keys[1].value
    );
}

#[test]
fn clips_limits_the_copy_to_the_named_clips() {
    let result = evaluate(&format!(
        "{RIGS}out = anim.retarget@1(tall, src, clips=\"wave\")\noutput tall = out\n"
    ))
    .unwrap();
    let copied = tracks(&result, "tall");
    assert_eq!(copied.len(), 1);
    assert_eq!(copied[0].clip, "wave");
}

#[test]
fn a_source_with_no_matching_tracks_is_refused_with_the_fix() {
    let error = evaluate(&format!(
        "{RIGS}out = anim.retarget@1(tall, src, clips=\"run\")\noutput tall = out\n"
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "anim.retarget_empty");
}

#[test]
fn the_retargeted_part_keeps_its_own_tracks_and_obeys_the_track_limit() {
    let own = "own = anim.track@1(tall, property=\"position\", keys=\"0:0,0,0; 1:0,1,0\", clip=\"hop\")\nout = anim.retarget@1(own, src)\noutput tall = out\n";
    let result = evaluate(&format!("{RIGS}{own}")).unwrap();
    assert_eq!(
        tracks(&result, "tall").len(),
        3,
        "its own hop plus the two copied"
    );
}

#[test]
fn retargeting_between_two_human_rigs_of_different_height_lands_on_the_taller_joints() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    ve_modeling::set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let program = "short = body.human@1(height=0.2)\ntall = body.human@1(height=0.9)\n\
sb = body.bone@1(short, bone=\"upperarm01_l\")\n\
sw = anim.track@1(sb, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,60\", pivot_at=\"head\", clip=\"raise\")\n\
output short_arm = sw\n\
tb = body.bone@1(tall, bone=\"upperarm01_l\")\n\
tw = anim.retarget@1(tb, sw)\n\
output tall_arm = tw\n";
    let result = evaluate(program).unwrap_or_else(|e| panic!("{e:?}"));
    let head = |name: &str| {
        result
            .parts
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .bone
            .unwrap()
            .head
    };
    let (short, tall) = (head("short_arm"), head("tall_arm"));
    assert!(
        tall[1] > short[1] + 0.05,
        "the tall body's shoulder is higher: {short:?} against {tall:?}"
    );
    let pivot = tracks(&result, "tall_arm")[0].pivot;
    assert_eq!(
        pivot, tall,
        "the copied rotation turns about the tall body's own shoulder"
    );
}
