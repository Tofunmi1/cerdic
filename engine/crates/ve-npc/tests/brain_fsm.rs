//! Contract tests for the middle layer of an NPC's mind (ADR 0063, step M3b): a hierarchical state machine described as data (with linked
//! reusable machines, timers and events) and a skill stack (prioritised, interruptible skills over reusable behaviour trees).
#![expect(
    clippy::expect_used,
    reason = "tests assert by failing and name small fixtures"
)]

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use ve_npc::{Blackboard, Ctx, Fsm, Registry, Rng, SkillStack, Status, Value};

type Log = Rc<RefCell<Vec<String>>>;

fn ctx() -> Ctx {
    Ctx::new(Blackboard::new(), Rng::new(1))
}

/// Registers an action that appends `name` to the shared log and succeeds.
fn logged(reg: &mut Registry, log: &Log, name: &str) {
    let (log, label) = (Rc::clone(log), name.to_owned());
    reg.register_action(name, move |_| {
        log.borrow_mut().push(label.clone());
        Status::Success
    });
}

fn registry(log: &Log) -> Registry {
    let mut reg = Registry::new();
    for name in [
        "idle_in",
        "idle_out",
        "idle_tick",
        "alert_in",
        "alert_out",
        "alert_tick",
        "watch_in",
        "watch_out",
        "chase_in",
        "chase_out",
        "sleep_in",
    ] {
        logged(&mut reg, log, name);
    }
    reg.register_condition("player_near", |c| {
        c.blackboard.get_bool("player_near", c.now) == Some(true)
    });
    reg
}

const GUARD: &str = r#"{
  "initial": "idle",
  "states": {
    "idle": {
      "on_enter": "idle_in", "on_update": "idle_tick", "on_exit": "idle_out",
      "transitions": [
        {"to": "alert", "when": {"condition": "player_near"}},
        {"to": "sleep", "when": {"after": 10}}
      ]
    },
    "alert": {
      "on_enter": "alert_in", "on_update": "alert_tick", "on_exit": "alert_out",
      "initial": "watch",
      "states": {
        "watch": {
          "on_enter": "watch_in", "on_exit": "watch_out",
          "transitions": [{"to": "chase", "when": {"check": {"key": "dist", "op": "<", "value": 5}}}]
        },
        "chase": {"on_enter": "chase_in", "on_exit": "chase_out"}
      },
      "transitions": [{"to": "idle", "when": {"not": {"condition": "player_near"}}}]
    },
    "sleep": {
      "on_enter": "sleep_in",
      "transitions": [{"to": "idle", "when": {"event": "noise"}}]
    }
  }
}"#;

fn step(fsm: &mut Fsm, reg: &mut Registry, c: &mut Ctx, now: f64) -> ve_npc::FsmTick {
    c.now = now;
    c.dt = 0.1;
    fsm.tick(reg, c).expect("tick")
}

fn path(fsm: &Fsm) -> Vec<String> {
    fsm.state_path()
}

// ---- the state machine ----

#[test]
fn starting_enters_the_initial_state_and_runs_its_enter_action() {
    let log: Log = Rc::default();
    let mut reg = registry(&log);
    let mut fsm = Fsm::from_json(GUARD).expect("valid");
    let mut c = ctx();
    fsm.start(&mut reg, &mut c).expect("starts");
    assert_eq!(path(&fsm), vec!["idle"]);
    assert_eq!(*log.borrow(), vec!["idle_in"]);
}

#[test]
fn a_transition_exits_then_enters_and_a_composite_state_enters_its_initial_child() {
    let log: Log = Rc::default();
    let mut reg = registry(&log);
    let mut fsm = Fsm::from_json(GUARD).expect("valid");
    let mut c = ctx();
    fsm.start(&mut reg, &mut c).expect("starts");
    log.borrow_mut().clear();
    let quiet = step(&mut fsm, &mut reg, &mut c, 0.1);
    assert!(!quiet.changed);
    assert_eq!(
        *log.borrow(),
        vec!["idle_tick"],
        "on_update runs every tick"
    );
    log.borrow_mut().clear();
    c.blackboard.set("player_near", Value::Bool(true), 0.2);
    let moved = step(&mut fsm, &mut reg, &mut c, 0.2);
    assert!(moved.changed);
    assert_eq!(moved.path, vec!["alert", "watch"]);
    assert_eq!(path(&fsm), vec!["alert", "watch"]);
    assert_eq!(
        *log.borrow(),
        vec!["idle_out", "alert_in", "watch_in"],
        "exit the old state, enter outer then inner"
    );
    assert!(!step(&mut fsm, &mut reg, &mut c, 0.3).changed);
}

#[test]
fn the_child_state_decides_before_its_parent_and_leaving_exits_inner_first() {
    let log: Log = Rc::default();
    let mut reg = registry(&log);
    let mut fsm = Fsm::from_json(GUARD).expect("valid");
    let mut c = ctx();
    c.blackboard.set("player_near", Value::Bool(true), 0.0);
    fsm.start(&mut reg, &mut c).expect("starts");
    step(&mut fsm, &mut reg, &mut c, 0.1);
    assert_eq!(path(&fsm), vec!["alert", "watch"]);
    c.blackboard.set("dist", Value::Number(3.0), 0.2);
    // The parent would also like to leave (player no longer near) but the child's own transition goes first.
    c.blackboard.set("player_near", Value::Bool(false), 0.2);
    step(&mut fsm, &mut reg, &mut c, 0.3);
    assert_eq!(path(&fsm), vec!["alert", "chase"], "child first");
    log.borrow_mut().clear();
    step(&mut fsm, &mut reg, &mut c, 0.4);
    assert_eq!(path(&fsm), vec!["idle"], "then the parent's transition");
    let entries = log.borrow().clone();
    let at = |name: &str| {
        entries
            .iter()
            .position(|e| e == name)
            .unwrap_or_else(|| panic!("{name} missing in {entries:?}"))
    };
    assert!(
        at("chase_out") < at("alert_out") && at("alert_out") < at("idle_in"),
        "{entries:?}"
    );
}

#[test]
fn only_one_transition_is_taken_per_tick() {
    let log: Log = Rc::default();
    let mut reg = registry(&log);
    let mut fsm = Fsm::from_json(GUARD).expect("valid");
    let mut c = ctx();
    c.blackboard.set("player_near", Value::Bool(true), 0.0);
    c.blackboard.set("dist", Value::Number(1.0), 0.0);
    fsm.start(&mut reg, &mut c).expect("starts");
    step(&mut fsm, &mut reg, &mut c, 0.1);
    assert_eq!(
        path(&fsm),
        vec!["alert", "watch"],
        "idle to alert only; watch to chase waits for the next tick"
    );
    step(&mut fsm, &mut reg, &mut c, 0.2);
    assert_eq!(path(&fsm), vec!["alert", "chase"]);
}

#[test]
fn timers_count_from_entering_the_state_and_events_are_consumed() {
    let log: Log = Rc::default();
    let mut reg = registry(&log);
    let mut fsm = Fsm::from_json(GUARD).expect("valid");
    let mut c = ctx();
    c.now = 100.0;
    fsm.start(&mut reg, &mut c).expect("starts");
    step(&mut fsm, &mut reg, &mut c, 109.0);
    assert_eq!(path(&fsm), vec!["idle"], "9 s in");
    step(&mut fsm, &mut reg, &mut c, 110.5);
    assert_eq!(path(&fsm), vec!["sleep"], "10 s in");
    fsm.send_event("unrelated");
    step(&mut fsm, &mut reg, &mut c, 111.0);
    assert_eq!(path(&fsm), vec!["sleep"], "unknown events are ignored");
    fsm.send_event("noise");
    step(&mut fsm, &mut reg, &mut c, 112.0);
    assert_eq!(path(&fsm), vec!["idle"]);
    step(&mut fsm, &mut reg, &mut c, 113.0);
    assert_eq!(
        path(&fsm),
        vec!["idle"],
        "the timer restarted when idle was entered again"
    );
}

#[test]
fn conditions_combine_with_all_any_and_not() {
    let json = r#"{"initial":"a","states":{
        "a":{"transitions":[
            {"to":"b","when":{"all":[{"condition":"p"},{"not":{"condition":"q"}}]}},
            {"to":"c","when":{"any":[{"condition":"q"},{"event":"go"}]}}]},
        "b":{},"c":{}}}"#;
    let mut reg = Registry::new();
    reg.register_condition("p", |c| c.blackboard.get_bool("p", c.now) == Some(true));
    reg.register_condition("q", |c| c.blackboard.get_bool("q", c.now) == Some(true));
    let mut c = ctx();
    c.blackboard.set("p", Value::Bool(true), 0.0);
    let mut fsm = Fsm::from_json(json).expect("valid");
    fsm.start(&mut reg, &mut c).expect("starts");
    step(&mut fsm, &mut reg, &mut c, 0.1);
    assert_eq!(path(&fsm), vec!["b"], "p and not q");
    let mut fsm = Fsm::from_json(json).expect("valid");
    fsm.start(&mut reg, &mut c).expect("starts");
    c.blackboard.set("q", Value::Bool(true), 0.2);
    step(&mut fsm, &mut reg, &mut c, 0.3);
    assert_eq!(
        path(&fsm),
        vec!["c"],
        "any: q alone is enough, and the first matching transition in order wins"
    );
}

#[test]
fn linked_machines_are_reused_by_name() {
    let mut library = BTreeMap::new();
    library.insert(
        "patrol".to_owned(),
        r#"{"initial":"walk","states":{"walk":{"transitions":[{"to":"look","when":{"after":2}}]},"look":{"transitions":[{"to":"walk","when":{"after":1}}]}}}"#.to_owned(),
    );
    let json = r#"{"initial":"day","states":{
        "day":{"link":"patrol","transitions":[{"to":"night","when":{"event":"dusk"}}]},
        "night":{"link":"patrol"}}}"#;
    let mut reg = Registry::new();
    let mut fsm = Fsm::from_json_with_library(json, &library).expect("links");
    let mut c = ctx();
    c.now = 0.0;
    fsm.start(&mut reg, &mut c).expect("starts");
    assert_eq!(path(&fsm), vec!["day", "walk"]);
    step(&mut fsm, &mut reg, &mut c, 2.5);
    assert_eq!(path(&fsm), vec!["day", "look"]);
    fsm.send_event("dusk");
    step(&mut fsm, &mut reg, &mut c, 3.0);
    assert_eq!(
        path(&fsm),
        vec!["night", "walk"],
        "the same machine, entered fresh under another parent"
    );
    let mut cyclic = BTreeMap::new();
    cyclic.insert(
        "a".to_owned(),
        r#"{"initial":"s","states":{"s":{"link":"b"}}}"#.to_owned(),
    );
    cyclic.insert(
        "b".to_owned(),
        r#"{"initial":"s","states":{"s":{"link":"a"}}}"#.to_owned(),
    );
    assert!(
        Fsm::from_json_with_library(r#"{"initial":"x","states":{"x":{"link":"a"}}}"#, &cyclic)
            .is_err()
    );
    assert!(
        Fsm::from_json_with_library(
            r#"{"initial":"x","states":{"x":{"link":"missing"}}}"#,
            &library
        )
        .is_err()
    );
}

#[test]
fn bad_machines_are_refused_before_they_run() {
    assert!(Fsm::from_json("{nope").is_err());
    assert!(
        Fsm::from_json(r#"{"initial":"ghost","states":{"a":{}}}"#).is_err(),
        "initial must exist"
    );
    assert!(
        Fsm::from_json(
            r#"{"initial":"a","states":{"a":{"transitions":[{"to":"ghost","when":{"after":1}}]}}}"#
        )
        .is_err(),
        "targets must exist"
    );
    assert!(Fsm::from_json(r#"{"initial":"a","states":{}}"#).is_err());
    assert!(Fsm::from_json(r#"{"initial":"a","states":{"a":{"transitions":[{"to":"a","when":{"check":{"key":"x","op":"~","value":1}}}]}}}"#).is_err());
    let fsm = Fsm::from_json(r#"{"initial":"a","states":{"a":{"on_enter":"ghost_action","transitions":[{"to":"a","when":{"condition":"ghost_cond"}}]}}}"#).expect("shape is fine");
    let error = fsm
        .validate(&Registry::new())
        .expect_err("unknown names")
        .to_string();
    assert!(
        error.contains("ghost_action") && error.contains("ghost_cond"),
        "{error}"
    );
}

#[test]
fn the_machine_round_trips_through_json() {
    let fsm = Fsm::from_json(GUARD).expect("valid");
    let again = Fsm::from_json(&fsm.to_json()).expect("reloads");
    assert_eq!(again.to_json(), fsm.to_json());
}

// ---- the skill stack ----

fn skills(log: &Log) -> (SkillStack, Registry, Rc<Cell<u32>>) {
    let mut reg = Registry::new();
    for flag in ["scared", "enemy_near"] {
        let name = flag.to_owned();
        reg.register_condition(flag, move |c| {
            c.blackboard.get_bool(&name, c.now) == Some(true)
        });
    }
    let (l, steps) = (Rc::clone(log), Rc::new(Cell::new(0)));
    reg.register_action("flee", move |_| {
        l.borrow_mut().push("flee".to_owned());
        Status::Running
    });
    let seen = Rc::clone(&steps);
    let l = Rc::clone(log);
    reg.register_action("aim", move |_| {
        l.borrow_mut().push("aim".to_owned());
        Status::Success
    });
    reg.register_action("fire", move |_| {
        seen.set(seen.get() + 1);
        if seen.get() % 2 == 1 {
            Status::Running
        } else {
            Status::Success
        }
    });
    let l = Rc::clone(log);
    reg.register_action("stroll", move |_| {
        l.borrow_mut().push("stroll".to_owned());
        Status::Running
    });
    let mut stack = SkillStack::new();
    stack
        .add(
            "panic",
            90,
            Some("scared"),
            r#"{"type":"action","name":"flee"}"#,
        )
        .expect("adds");
    stack.add("fight", 50, Some("enemy_near"), r#"{"type":"sequence","children":[{"type":"action","name":"aim"},{"type":"action","name":"fire"}]}"#).expect("adds");
    stack
        .add("wander", 10, None, r#"{"type":"action","name":"stroll"}"#)
        .expect("adds");
    (stack, reg, steps)
}

fn pick(stack: &mut SkillStack, reg: &mut Registry, c: &mut Ctx, now: f64) -> ve_npc::SkillTick {
    c.now = now;
    c.dt = 0.1;
    stack.tick(reg, c).expect("tick")
}

#[test]
fn the_highest_priority_skill_that_wants_to_run_is_active() {
    let log: Log = Rc::default();
    let (mut stack, mut reg, _) = skills(&log);
    let mut c = ctx();
    let t = pick(&mut stack, &mut reg, &mut c, 0.0);
    assert_eq!(t.active.as_deref(), Some("wander"));
    assert!(t.switched, "the first choice counts as a switch");
    assert!(!pick(&mut stack, &mut reg, &mut c, 0.1).switched);
    c.blackboard.set("enemy_near", Value::Bool(true), 0.2);
    let t = pick(&mut stack, &mut reg, &mut c, 0.2);
    assert_eq!((t.active.as_deref(), t.switched), (Some("fight"), true));
    c.blackboard.set("scared", Value::Bool(true), 0.3);
    assert_eq!(
        pick(&mut stack, &mut reg, &mut c, 0.3).active.as_deref(),
        Some("panic")
    );
    c.blackboard.set("scared", Value::Bool(false), 0.4);
    assert_eq!(
        pick(&mut stack, &mut reg, &mut c, 0.4).active.as_deref(),
        Some("fight"),
        "back to the next that wants to run"
    );
    c.blackboard.set("enemy_near", Value::Bool(false), 0.5);
    assert_eq!(
        pick(&mut stack, &mut reg, &mut c, 0.5).active.as_deref(),
        Some("wander")
    );
}

#[test]
fn an_interrupted_skill_starts_over_when_it_comes_back() {
    let log: Log = Rc::default();
    let (mut stack, mut reg, shots) = skills(&log);
    let mut c = ctx();
    c.blackboard.set("enemy_near", Value::Bool(true), 0.0);
    pick(&mut stack, &mut reg, &mut c, 0.0);
    assert_eq!(shots.get(), 1, "aim succeeded, fire is running");
    c.blackboard.set("scared", Value::Bool(true), 0.1);
    pick(&mut stack, &mut reg, &mut c, 0.1);
    c.blackboard.set("scared", Value::Bool(false), 0.2);
    log.borrow_mut().clear();
    pick(&mut stack, &mut reg, &mut c, 0.2);
    assert_eq!(
        log.borrow().first().map(String::as_str),
        Some("aim"),
        "the sequence began again at its first child"
    );
}

#[test]
fn a_skill_that_finishes_runs_again_while_it_still_wants_to() {
    let log: Log = Rc::default();
    let (mut stack, mut reg, shots) = skills(&log);
    let mut c = ctx();
    c.blackboard.set("enemy_near", Value::Bool(true), 0.0);
    let statuses: Vec<_> = (0..4)
        .map(|i| pick(&mut stack, &mut reg, &mut c, f64::from(i) * 0.1).status)
        .collect();
    assert_eq!(
        statuses,
        vec![
            Status::Running,
            Status::Success,
            Status::Running,
            Status::Success
        ]
    );
    assert_eq!(shots.get(), 4);
}

#[test]
fn a_stack_where_nothing_wants_to_run_has_no_active_skill() {
    let mut reg = Registry::new();
    reg.register_condition("never", |_| false);
    reg.register_action("noop", |_| Status::Success);
    let mut stack = SkillStack::new();
    stack
        .add(
            "only",
            5,
            Some("never"),
            r#"{"type":"action","name":"noop"}"#,
        )
        .expect("adds");
    let mut c = ctx();
    let t = pick(&mut stack, &mut reg, &mut c, 0.0);
    assert_eq!(t.active, None);
    assert_eq!(t.status, Status::Failure);
}

#[test]
fn equal_priorities_go_to_the_skill_added_first_and_bad_skills_are_refused() {
    let mut reg = Registry::new();
    reg.register_action("a", |_| Status::Running);
    reg.register_action("b", |_| Status::Running);
    let mut stack = SkillStack::new();
    stack
        .add("first", 5, None, r#"{"type":"action","name":"a"}"#)
        .expect("adds");
    stack
        .add("second", 5, None, r#"{"type":"action","name":"b"}"#)
        .expect("adds");
    let mut c = ctx();
    assert_eq!(
        pick(&mut stack, &mut reg, &mut c, 0.0).active.as_deref(),
        Some("first")
    );
    assert!(
        stack
            .add("first", 9, None, r#"{"type":"action","name":"a"}"#)
            .is_err(),
        "names are unique"
    );
    assert!(stack.add("broken", 9, None, "{nope").is_err());
    let error = stack
        .validate(&Registry::new())
        .expect_err("unknown names")
        .to_string();
    assert!(error.contains('a') && error.contains('b'));
    let mut with_cond = SkillStack::new();
    with_cond
        .add(
            "x",
            1,
            Some("ghost_condition"),
            r#"{"type":"action","name":"a"}"#,
        )
        .expect("adds");
    assert!(
        with_cond
            .validate(&reg)
            .expect_err("unknown condition")
            .to_string()
            .contains("ghost_condition")
    );
}
