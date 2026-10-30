//! `anim.machine`: the animation state machine as a node. The text is `ve_animstate::StateMachine::parse`'s; the node carries the parsed
//! machine on the part it is applied to, tells the model when the machine names clips nothing defines, and leaves the shape alone. The runtime
//! (a later job) drives it. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(16);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_machine").unwrap(),
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

const PROGRAM: &str = "entry ground; state ground blend=speed idle@0 walk@1.5; state jump clip=jump once; transition any -> jump when trigger:jump fade=0.1; transition jump -> ground exit=0.9 when grounded=true";

/// Newlines cannot sit inside a string parameter, so the node reads `;` as a line break too.
fn world(program: &str, tracks: &str) -> String {
    format!(
        "body = solid.box@1(size=[1, 2, 1])\n{tracks}m = anim.machine@1({last}, program=\"{program}\")\noutput m = m\n",
        last = if tracks.is_empty() { "body" } else { "t_last" }
    )
}

/// A box that has keyframed tracks in each of the clips `walk`, `idle` and `jump`.
fn tracks() -> String {
    let mut text = String::new();
    let mut previous = "body".to_owned();
    for (i, clip) in ["walk", "idle", "jump"].iter().enumerate() {
        let name = if i == 2 {
            "t_last".to_owned()
        } else {
            format!("t{i}")
        };
        text.push_str(&format!(
            "{name} = anim.track@1({previous}, property=\"position\", keys=\"0:0,0,0; 1:0,0.1,0\", clip=\"{clip}\")\n"
        ));
        previous = name;
    }
    text
}

#[test]
fn the_machine_is_carried_on_the_part_and_the_shape_is_untouched() {
    let plain = evaluate("body = solid.box@1(size=[1, 2, 1])\noutput m = body\n").unwrap();
    let with = evaluate(&world(PROGRAM, "")).unwrap();
    assert_eq!(
        with.parts[0].positions, plain.parts[0].positions,
        "the shape is unchanged"
    );
    let machine = with.parts[0]
        .state_machine
        .as_ref()
        .expect("the part carries its machine");
    assert_eq!(machine.entry, "ground");
    assert_eq!(machine.states.len(), 2);
    assert_eq!(machine.transitions.len(), 2);
    machine.validate().unwrap();
}

#[test]
fn a_part_without_the_node_has_no_machine() {
    let ev = evaluate("body = solid.box@1(size=[1, 2, 1])\noutput m = body\n").unwrap();
    assert!(ev.parts[0].state_machine.is_none());
}

#[test]
fn a_bad_program_is_refused_with_the_line_that_is_wrong() {
    // `;` counts as a line break, so the second statement is line 2.
    let error = evaluate(&world("entry a; state a clip=x; bogus statement", "")).unwrap_err();
    assert!(error.contains("line 3"), "{error}");
    let error = evaluate(&world("entry nowhere; state a clip=x", "")).unwrap_err();
    assert!(error.contains("nowhere"), "{error}");
    assert!(evaluate(&world("", "")).is_err(), "an empty program");
}

#[test]
fn clips_nothing_defines_are_reported_so_the_model_can_fix_them() {
    // The tracks define walk, idle and jump; the machine plays idle, walk and jump: nothing missing.
    let ok = evaluate(&world(PROGRAM, &tracks())).unwrap();
    assert!(
        !ok.warnings
            .iter()
            .any(|w| w.code == "anim.machine.missing_clip"),
        "{:?}",
        ok.warnings
    );
    // A machine that plays a clip no track belongs to gets one warning that names the clip.
    let program =
        "entry a; state a clip=idle; state b clip=dance; transition a -> b when trigger:go";
    let ev = evaluate(&world(program, &tracks())).unwrap();
    let missing: Vec<_> = ev
        .warnings
        .iter()
        .filter(|w| w.code == "anim.machine.missing_clip")
        .collect();
    assert_eq!(missing.len(), 1, "{:?}", ev.warnings);
    assert!(
        missing[0].message.contains("dance"),
        "{}",
        missing[0].message
    );
    assert!(!missing[0].message.contains("idle"));
}

#[test]
fn the_default_clip_is_the_empty_name_and_matches_tracks_with_no_clip() {
    let tracks =
        "t_last = anim.track@1(body, property=\"position\", keys=\"0:0,0,0; 1:0,0.1,0\")\n";
    let ev = evaluate(&world("entry w; state w clip=\\\"\\\"", tracks)).unwrap();
    assert!(
        !ev.warnings
            .iter()
            .any(|w| w.code == "anim.machine.missing_clip"),
        "{:?}",
        ev.warnings
    );
}

#[test]
fn whole_number_values_written_as_integers_work() {
    // An exit time needs a one-shot source state, so `a` plays once.
    let program = "entry a; state a clip=x speed=1 once; state b clip=y; transition a -> b when speed>1 fade=0 exit=1";
    assert!(evaluate(&world(program, "")).is_ok());
}
