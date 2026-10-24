//! Additional regression coverage for hierarchical FSM and skill semantics.
#![expect(
    clippy::expect_used,
    reason = "tests use expect for concise fixture failures"
)]
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use ve_npc::{Blackboard, Ctx, Fsm, Registry, Rng, SkillStack, Status, Value};

type Log = Rc<RefCell<Vec<String>>>;
fn context() -> Ctx {
    Ctx::new(Blackboard::new(), Rng::new(7))
}
fn actions(reg: &mut Registry, log: &Log, names: &[&str]) {
    for name in names {
        let (out, label) = (Rc::clone(log), (*name).to_owned());
        reg.register_action(name, move |_| {
            out.borrow_mut().push(label.clone());
            Status::Running
        });
    }
}
fn tick(fsm: &mut Fsm, reg: &mut Registry, ctx: &mut Ctx, now: f64) {
    ctx.now = now;
    fsm.tick(reg, ctx).expect("tick");
}

#[test]
fn deep_parent_transition_exits_chain_and_updates_run_outer_first() {
    let json = r#"{"initial":"a","states":{"a":{"on_update":"a_u","on_exit":"a_x","initial":"b","states":{"b":{"on_update":"b_u","on_exit":"b_x","initial":"c","states":{"c":{"on_update":"c_u","on_exit":"c_x"}},"transitions":[]}},"transitions":[{"to":"z","when":{"event":"leave"}}]},"z":{"on_enter":"z_i"}}}"#;
    let log: Log = Rc::default();
    let mut reg = Registry::new();
    actions(
        &mut reg,
        &log,
        &["a_u", "b_u", "c_u", "a_x", "b_x", "c_x", "z_i"],
    );
    let mut f = Fsm::from_json(json).expect("parse");
    let mut c = context();
    f.start(&mut reg, &mut c).expect("start");
    log.borrow_mut().clear();
    tick(&mut f, &mut reg, &mut c, 1.);
    assert_eq!(*log.borrow(), ["a_u", "b_u", "c_u"]);
    log.borrow_mut().clear();
    f.send_event("leave");
    tick(&mut f, &mut reg, &mut c, 2.);
    assert_eq!(*log.borrow(), ["c_x", "b_x", "a_x", "z_i"]);
    assert_eq!(f.state_path(), ["z"]);
}

#[test]
fn self_transition_reenters_and_restarts_after_timer() {
    let json = r#"{"initial":"a","states":{"a":{"on_enter":"in","on_exit":"out","transitions":[{"to":"a","when":{"after":2}}]}}}"#;
    let log: Log = Rc::default();
    let mut reg = Registry::new();
    actions(&mut reg, &log, &["in", "out"]);
    let mut f = Fsm::from_json(json).expect("parse");
    let mut c = context();
    c.now = 1.;
    f.start(&mut reg, &mut c).expect("start");
    c.now = 2.;
    assert!(!f.tick(&mut reg, &mut c).expect("tick").changed);
    c.now = 3.;
    assert!(f.tick(&mut reg, &mut c).expect("tick").changed);
    assert_eq!(*log.borrow(), ["in", "out", "in"]);
}

#[test]
fn child_timer_and_linked_sibling_timers_are_independent() {
    let lib=BTreeMap::from([("m".to_owned(),r#"{"initial":"wait","states":{"wait":{"transitions":[{"to":"done","when":{"after":2}}]},"done":{}}}"#.to_owned())]);
    let json = r#"{"initial":"box","states":{"box":{"initial":"kid","states":{"kid":{"transitions":[{"to":"end","when":{"after":3}}]},"end":{}}},"one":{"link":"m"},"two":{"link":"m"}}}"#;
    let mut f = Fsm::from_json_with_library(json, &lib).expect("parse");
    let mut r = Registry::new();
    let mut c = context();
    f.start(&mut r, &mut c).expect("start");
    tick(&mut f, &mut r, &mut c, 2.);
    assert_eq!(f.state_path(), ["box", "kid"]);
    let json = r#"{"initial":"one","states":{"one":{"link":"m","transitions":[{"to":"two","when":{"event":"go"}}]},"two":{"link":"m"}}}"#;
    let mut f = Fsm::from_json_with_library(json, &lib).expect("parse");
    f.start(&mut r, &mut c).expect("start");
    tick(&mut f, &mut r, &mut c, 10.);
    f.send_event("go");
    tick(&mut f, &mut r, &mut c, 10.);
    assert_eq!(f.state_path(), ["two", "wait"]);
    tick(&mut f, &mut r, &mut c, 11.);
    assert_eq!(f.state_path(), ["two", "wait"]);
}

#[test]
fn event_before_start_is_discarded_and_check_supports_not_equal_and_less_equal() {
    let json = r#"{"initial":"a","states":{"a":{"transitions":[{"to":"b","when":{"check":{"key":"n","op":"!=","value":4}}}]},"b":{"transitions":[{"to":"c","when":{"check":{"key":"n","op":"<=","value":3}}}]},"c":{}}}"#;
    let mut f = Fsm::from_json(json).expect("parse");
    f.send_event("early");
    let mut r = Registry::new();
    let mut c = context();
    c.blackboard.set("n", Value::Number(3.), 0.);
    f.start(&mut r, &mut c).expect("start");
    tick(&mut f, &mut r, &mut c, 1.);
    assert_eq!(f.state_path(), ["b"]);
    tick(&mut f, &mut r, &mut c, 2.);
    assert_eq!(f.state_path(), ["c"]);
}

#[test]
fn six_equal_priority_skills_keep_first_and_empty_validation_succeeds() {
    let mut s = SkillStack::new();
    let mut r = Registry::new();
    for n in 0..6 {
        let name = format!("a{n}");
        r.register_action(&name, |_| Status::Running);
        s.add(
            &format!("s{n}"),
            2,
            None,
            &format!(r#"{{"type":"action","name":"{name}"}}"#),
        )
        .expect("add");
    }
    assert_eq!(
        s.tick(&mut r, &mut context())
            .expect("tick")
            .active
            .as_deref(),
        Some("s0")
    );
    assert!(SkillStack::new().validate(&Registry::new()).is_ok());
}

#[test]
fn seeded_parser_fuzz_never_panics() {
    let mut seed = 0x9e37_79b9_u32;
    for len in 0..5000 {
        let size = len % 97;
        let mut bytes = Vec::with_capacity(size);
        for _ in 0..size {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            bytes.push(seed.to_le_bytes()[0]);
        }
        let text = String::from_utf8_lossy(&bytes);
        let _ = Fsm::from_json(&text);
    }
}
