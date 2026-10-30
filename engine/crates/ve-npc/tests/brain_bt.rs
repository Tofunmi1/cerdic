//! Contract tests for the fast layer of an NPC's mind (ADR 0063 and 0064, step M3a): the blackboard everything shares, a seeded generator,
//! named conditions and actions in a registry, and behaviour trees described as data that run deterministically with memory.
#![expect(
    clippy::expect_used,
    clippy::many_single_char_names,
    reason = "tests assert by failing and name small fixtures a, c, t"
)]

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ve_npc::{BehaviorTree, Blackboard, BrainError, Command, Ctx, Registry, Rng, Status, Value};

fn ctx() -> Ctx {
    Ctx::new(Blackboard::new(), Rng::new(7))
}

fn tick(tree: &mut BehaviorTree, reg: &mut Registry, c: &mut Ctx, now: f64, dt: f64) -> Status {
    c.now = now;
    c.dt = dt;
    tree.tick(reg, c).expect("tick")
}

/// An action that counts how many times it ran and answers from a script, then repeats the last answer.
fn scripted(reg: &mut Registry, name: &str, script: &[Status]) -> Rc<Cell<u32>> {
    let count = Rc::new(Cell::new(0));
    let (seen, script) = (Rc::clone(&count), script.to_vec());
    reg.register_action(name, move |_ctx| {
        let i = seen.get() as usize;
        seen.set(seen.get() + 1);
        script[i.min(script.len() - 1)]
    });
    count
}

fn tree(json: &str) -> BehaviorTree {
    BehaviorTree::from_json(json).expect("valid tree")
}

// ---- blackboard ----

#[test]
fn the_blackboard_stores_typed_values_that_can_expire() {
    let mut bb = Blackboard::new();
    bb.set("hp", Value::Number(40.0), 0.0);
    bb.set("name", Value::Text("Anna".into()), 0.0);
    bb.set("angry", Value::Bool(true), 0.0);
    bb.set_for("sees_player", Value::Bool(true), 10.0, 5.0);
    assert_eq!(bb.get_number("hp", 1.0), Some(40.0));
    assert_eq!(bb.get_text("name", 1.0).as_deref(), Some("Anna"));
    assert_eq!(bb.get_bool("angry", 1.0), Some(true));
    assert_eq!(
        bb.get_number("name", 1.0),
        None,
        "wrong type is None, not a panic"
    );
    assert_eq!(bb.get_bool("sees_player", 14.9), Some(true));
    assert_eq!(
        bb.get_bool("sees_player", 15.1),
        None,
        "expired values are gone"
    );
    assert_eq!(bb.age("hp", 12.5), Some(12.5));
    assert_eq!(
        bb.keys(20.0),
        vec!["angry", "hp", "name"],
        "sorted, and without the expired key"
    );
    assert_eq!(bb.remove("hp"), Some(Value::Number(40.0)));
    assert_eq!(bb.get("hp", 1.0), None);
}

#[test]
fn the_generator_is_seeded_and_fixed() {
    let mut a = Rng::new(0);
    assert_eq!(
        a.next_u64(),
        0xE220_A839_7B1D_CDAF,
        "splitmix64 from seed 0"
    );
    let (mut x, mut y) = (Rng::new(99), Rng::new(99));
    for _ in 0..100 {
        assert_eq!(x.next_u64(), y.next_u64());
    }
    let mut z = Rng::new(5);
    for _ in 0..1000 {
        let f = z.next_f64();
        assert!((0.0..1.0).contains(&f));
        let r = z.range(2.0, 3.0);
        assert!((2.0..3.0).contains(&r));
    }
    assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
}

// ---- behaviour trees ----

#[test]
fn a_sequence_stops_at_the_first_failure_and_resumes_at_a_running_child() {
    let mut reg = Registry::new();
    let first = scripted(&mut reg, "first", &[Status::Success]);
    let slow = scripted(
        &mut reg,
        "slow",
        &[Status::Running, Status::Running, Status::Success],
    );
    let never = scripted(&mut reg, "never", &[Status::Success]);
    reg.register_condition("no", |_| false);
    let mut t = tree(
        r#"{"type":"sequence","children":[{"type":"action","name":"first"},{"type":"action","name":"slow"},{"type":"action","name":"never"}]}"#,
    );
    let mut c = ctx();
    assert_eq!(tick(&mut t, &mut reg, &mut c, 0.0, 0.1), Status::Running);
    assert_eq!(tick(&mut t, &mut reg, &mut c, 0.1, 0.1), Status::Running);
    assert_eq!(
        first.get(),
        1,
        "the finished first child is not run again while a later child is running"
    );
    assert_eq!(
        tick(&mut t, &mut reg, &mut c, 0.2, 0.1),
        Status::Success,
        "slow finishes, never is last... "
    );
    assert_eq!(slow.get(), 3);
    assert_eq!(never.get(), 1);
    let mut failing = tree(
        r#"{"type":"sequence","children":[{"type":"condition","name":"no"},{"type":"action","name":"never"}]}"#,
    );
    assert_eq!(
        tick(&mut failing, &mut reg, &mut c, 1.0, 0.1),
        Status::Failure
    );
    assert_eq!(never.get(), 1, "children after a failure are not ticked");
}

#[test]
fn a_selector_takes_the_first_success_and_fails_when_all_fail() {
    let mut reg = Registry::new();
    reg.register_condition("no", |_| false);
    let chosen = scripted(&mut reg, "chosen", &[Status::Success]);
    let after = scripted(&mut reg, "after", &[Status::Success]);
    let mut t = tree(
        r#"{"type":"selector","children":[{"type":"condition","name":"no"},{"type":"action","name":"chosen"},{"type":"action","name":"after"}]}"#,
    );
    let mut c = ctx();
    assert_eq!(tick(&mut t, &mut reg, &mut c, 0.0, 0.1), Status::Success);
    assert_eq!((chosen.get(), after.get()), (1, 0));
    let mut all_fail = tree(
        r#"{"type":"selector","children":[{"type":"condition","name":"no"},{"type":"condition","name":"no"}]}"#,
    );
    assert_eq!(
        tick(&mut all_fail, &mut reg, &mut c, 1.0, 0.1),
        Status::Failure
    );
}

#[test]
fn decorators_invert_succeed_repeat_wait_and_cool_down() {
    let mut reg = Registry::new();
    reg.register_condition("no", |_| false);
    let mut c = ctx();
    let mut inv = tree(r#"{"type":"inverter","child":{"type":"condition","name":"no"}}"#);
    assert_eq!(tick(&mut inv, &mut reg, &mut c, 0.0, 0.1), Status::Success);
    let mut ok = tree(r#"{"type":"succeeder","child":{"type":"condition","name":"no"}}"#);
    assert_eq!(tick(&mut ok, &mut reg, &mut c, 0.0, 0.1), Status::Success);

    let step = scripted(&mut reg, "step", &[Status::Success]);
    let mut rep = tree(r#"{"type":"repeat","times":3,"child":{"type":"action","name":"step"}}"#);
    let seen: Vec<_> = (0..3)
        .map(|i| tick(&mut rep, &mut reg, &mut c, f64::from(i), 0.1))
        .collect();
    assert_eq!(
        seen,
        vec![Status::Running, Status::Running, Status::Success]
    );
    assert_eq!(step.get(), 3);

    let mut wait = tree(r#"{"type":"wait","secs":2.0}"#);
    assert_eq!(tick(&mut wait, &mut reg, &mut c, 0.0, 1.0), Status::Running);
    assert_eq!(tick(&mut wait, &mut reg, &mut c, 1.0, 1.0), Status::Success);
    assert_eq!(
        tick(&mut wait, &mut reg, &mut c, 2.0, 1.0),
        Status::Running,
        "it starts over after finishing"
    );

    let job = scripted(&mut reg, "job", &[Status::Success]);
    let mut cd = tree(r#"{"type":"cooldown","secs":2.0,"child":{"type":"action","name":"job"}}"#);
    assert_eq!(tick(&mut cd, &mut reg, &mut c, 0.0, 0.1), Status::Success);
    assert_eq!(
        tick(&mut cd, &mut reg, &mut c, 1.0, 0.1),
        Status::Failure,
        "cooling down"
    );
    assert_eq!(job.get(), 1);
    assert_eq!(tick(&mut cd, &mut reg, &mut c, 2.5, 0.1), Status::Success);
    assert_eq!(job.get(), 2);
}

#[test]
fn parallel_has_two_policies() {
    let mut reg = Registry::new();
    scripted(&mut reg, "slow", &[Status::Running, Status::Success]);
    scripted(&mut reg, "quick", &[Status::Success]);
    let mut c = ctx();
    let all = r#"{"type":"parallel","policy":"all","children":[{"type":"action","name":"slow"},{"type":"action","name":"quick"}]}"#;
    let mut t = tree(all);
    assert_eq!(
        tick(&mut t, &mut reg, &mut c, 0.0, 0.1),
        Status::Running,
        "all: wait for the slow one"
    );
    assert_eq!(tick(&mut t, &mut reg, &mut c, 0.1, 0.1), Status::Success);
    let mut reg2 = Registry::new();
    scripted(&mut reg2, "slow", &[Status::Running]);
    scripted(&mut reg2, "quick", &[Status::Success]);
    let mut one = tree(&all.replace("\"all\"", "\"one\""));
    assert_eq!(
        tick(&mut one, &mut reg2, &mut c, 0.0, 0.1),
        Status::Success,
        "one: the quick one is enough"
    );
}

#[test]
fn built_in_check_and_set_nodes_use_the_blackboard() {
    let mut reg = Registry::new();
    let mut c = ctx();
    c.blackboard.set("hp", Value::Number(25.0), 0.0);
    let mut low = tree(r#"{"type":"check","key":"hp","op":"<","value":30}"#);
    assert_eq!(tick(&mut low, &mut reg, &mut c, 0.0, 0.1), Status::Success);
    let mut high = tree(r#"{"type":"check","key":"hp","op":">=","value":30}"#);
    assert_eq!(tick(&mut high, &mut reg, &mut c, 0.0, 0.1), Status::Failure);
    let mut missing = tree(r#"{"type":"check","key":"nothing","op":"==","value":true}"#);
    assert_eq!(
        tick(&mut missing, &mut reg, &mut c, 0.0, 0.1),
        Status::Failure,
        "a missing key fails the check"
    );
    let mut set = tree(r#"{"type":"set","key":"mood","value":"afraid"}"#);
    assert_eq!(tick(&mut set, &mut reg, &mut c, 3.0, 0.1), Status::Success);
    assert_eq!(
        c.blackboard.get_text("mood", 3.0).as_deref(),
        Some("afraid")
    );
    let mut text_eq = tree(r#"{"type":"check","key":"mood","op":"==","value":"afraid"}"#);
    assert_eq!(
        tick(&mut text_eq, &mut reg, &mut c, 3.0, 0.1),
        Status::Success
    );
}

#[test]
fn actions_emit_commands_in_order_and_conditions_read_the_context() {
    let mut reg = Registry::new();
    reg.register_condition("player_near", |ctx| {
        ctx.blackboard.get_bool("player_near", ctx.now) == Some(true)
    });
    reg.register_action("greet", |ctx| {
        ctx.commands.push(Command::LookAt {
            target: [1.0, 0.0, 2.0],
        });
        ctx.commands.push(Command::Say {
            text: "Welcome!".into(),
        });
        Status::Success
    });
    let mut t = tree(
        r#"{"type":"sequence","children":[{"type":"condition","name":"player_near"},{"type":"action","name":"greet"}]}"#,
    );
    let mut c = ctx();
    assert_eq!(tick(&mut t, &mut reg, &mut c, 0.0, 0.1), Status::Failure);
    assert!(c.commands.is_empty());
    c.blackboard.set("player_near", Value::Bool(true), 0.0);
    assert_eq!(tick(&mut t, &mut reg, &mut c, 0.1, 0.1), Status::Success);
    assert_eq!(
        c.commands,
        vec![
            Command::LookAt {
                target: [1.0, 0.0, 2.0]
            },
            Command::Say {
                text: "Welcome!".into()
            }
        ]
    );
}

#[test]
fn unknown_names_and_bad_trees_are_caught_before_running() {
    let reg = Registry::new();
    let t = tree(
        r#"{"type":"sequence","children":[{"type":"condition","name":"ghost"},{"type":"action","name":"phantom"}]}"#,
    );
    let error = t.validate(&reg).expect_err("unknown names");
    let text = error.to_string();
    assert!(text.contains("ghost") && text.contains("phantom"), "{text}");
    assert!(BehaviorTree::from_json("{not json").is_err());
    assert!(BehaviorTree::from_json(r#"{"type":"teleport"}"#).is_err());
    assert!(
        BehaviorTree::from_json(r#"{"type":"sequence","children":[]}"#).is_err(),
        "an empty sequence is a mistake"
    );
    assert!(BehaviorTree::from_json(r#"{"type":"check","key":"x","op":"~","value":1}"#).is_err());
}

#[test]
fn subtrees_are_linked_by_name_and_cycles_are_refused() {
    let mut library = BTreeMap::new();
    library.insert(
        "greet_flow".to_owned(),
        r#"{"type":"action","name":"greet"}"#.to_owned(),
    );
    let mut reg = Registry::new();
    let greet = scripted(&mut reg, "greet", &[Status::Success]);
    let mut t = BehaviorTree::from_json_with_library(
        r#"{"type":"sequence","children":[{"type":"subtree","name":"greet_flow"}]}"#,
        &library,
    )
    .expect("links");
    let mut c = ctx();
    assert_eq!(tick(&mut t, &mut reg, &mut c, 0.0, 0.1), Status::Success);
    assert_eq!(greet.get(), 1);
    let mut cyclic = BTreeMap::new();
    cyclic.insert(
        "a".to_owned(),
        r#"{"type":"subtree","name":"b"}"#.to_owned(),
    );
    cyclic.insert(
        "b".to_owned(),
        r#"{"type":"subtree","name":"a"}"#.to_owned(),
    );
    assert!(
        BehaviorTree::from_json_with_library(r#"{"type":"subtree","name":"a"}"#, &cyclic).is_err()
    );
    assert!(
        BehaviorTree::from_json_with_library(r#"{"type":"subtree","name":"missing"}"#, &library)
            .is_err()
    );
}

#[test]
fn a_tick_has_a_node_budget() {
    let mut reg = Registry::new();
    reg.register_condition("yes", |_| true);
    let children = vec![r#"{"type":"condition","name":"yes"}"#; 20].join(",");
    let mut t = tree(&format!(
        "{{\"type\":\"sequence\",\"children\":[{children}]}}"
    ));
    let mut c = ctx();
    t.set_node_budget(10);
    c.now = 0.0;
    assert!(matches!(
        t.tick(&mut reg, &mut c),
        Err(BrainError::TickBudget { .. })
    ));
    t.set_node_budget(100);
    assert_eq!(t.tick(&mut reg, &mut c).expect("fits"), Status::Success);
}

#[test]
fn the_same_seed_gives_the_same_decisions_and_the_spec_round_trips() {
    let json = r#"{"type":"selector","children":[{"type":"sequence","children":[{"type":"action","name":"coin"},{"type":"action","name":"heads"}]},{"type":"action","name":"tails"}]}"#;
    let run = || {
        let mut reg = Registry::new();
        reg.register_action("coin", |ctx| {
            if ctx.rng.next_f64() < 0.5 {
                Status::Success
            } else {
                Status::Failure
            }
        });
        reg.register_action("heads", |ctx| {
            ctx.commands.push(Command::Say {
                text: "heads".into(),
            });
            Status::Success
        });
        reg.register_action("tails", |ctx| {
            ctx.commands.push(Command::Say {
                text: "tails".into(),
            });
            Status::Success
        });
        let mut t = tree(json);
        let mut c = Ctx::new(Blackboard::new(), Rng::new(42));
        for i in 0..40 {
            tick(&mut t, &mut reg, &mut c, f64::from(i), 0.1);
        }
        c.commands
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    assert!(
        a.contains(&Command::Say {
            text: "heads".into()
        }) && a.contains(&Command::Say {
            text: "tails".into()
        }),
        "both outcomes occur over 40 flips"
    );
    let t = tree(json);
    assert_eq!(
        BehaviorTree::from_json(&t.to_json())
            .expect("reloads")
            .to_json(),
        t.to_json()
    );
}
