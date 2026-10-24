//! Additional behaviour tree edge-case contracts.
use std::collections::BTreeMap;
use std::{cell::Cell, rc::Rc};

use ve_npc::{BehaviorTree, Blackboard, Ctx, Registry, Rng, Status, Value};

#[test]
fn blackboard_overwrite_ttl_and_points() {
    let mut board = Blackboard::new();
    board.set_for("x", Value::Number(1.0), 0.0, 4.0);
    board.set_for("x", Value::Point([1.0, 2.0, 3.0]), 3.0, 0.0);
    assert_eq!(board.age("x", 3.0), Some(0.0));
    assert_eq!(board.get_point("x", 3.0), Some([1.0, 2.0, 3.0]));
    assert_eq!(board.get("x", 3.01), None);
}

#[test]
fn selectors_and_nested_sequences_resume_running_nodes() {
    let mut reg = Registry::new();
    let mut count = 0;
    reg.register_action("run", move |_| {
        count += 1;
        if count < 3 {
            Status::Running
        } else {
            Status::Success
        }
    });
    let spec = r#"{"type":"selector","children":[{"type":"sequence","children":[{"type":"condition","name":"no"},{"type":"action","name":"run"}]},{"type":"sequence","children":[{"type":"action","name":"run"}]}]}"#;
    reg.register_condition("no", |_| false);
    let mut tree = BehaviorTree::from_json(spec).expect("tree");
    let mut ctx = Ctx::new(Blackboard::new(), Rng::new(2));
    assert_eq!(
        tree.tick(&mut reg, &mut ctx).expect("tick"),
        Status::Running
    );
    assert_eq!(
        tree.tick(&mut reg, &mut ctx).expect("tick"),
        Status::Running
    );
    assert_eq!(
        tree.tick(&mut reg, &mut ctx).expect("tick"),
        Status::Success
    );
}

#[test]
fn parallel_resets_after_finishing_and_repeat_failure_clears_count() {
    let mut reg = Registry::new();
    let mut n = 0;
    reg.register_action("step", move |_| {
        n += 1;
        if n == 2 {
            Status::Failure
        } else {
            Status::Success
        }
    });
    let mut repeat = BehaviorTree::from_json(
        r#"{"type":"repeat","times":2,"child":{"type":"action","name":"step"}}"#,
    )
    .expect("tree");
    let mut ctx = Ctx::new(Blackboard::new(), Rng::new(3));
    assert_eq!(
        repeat.tick(&mut reg, &mut ctx).expect("tick"),
        Status::Running
    );
    assert_eq!(
        repeat.tick(&mut reg, &mut ctx).expect("tick"),
        Status::Failure
    );
    assert_eq!(
        repeat.tick(&mut reg, &mut ctx).expect("tick"),
        Status::Running
    );

    let mut parallel = BehaviorTree::from_json(
        r#"{"type":"parallel","policy":"all","children":[{"type":"action","name":"ok"}]}"#,
    )
    .expect("tree");
    reg.register_action("ok", |_| Status::Success);
    assert_eq!(
        parallel.tick(&mut reg, &mut ctx).expect("tick"),
        Status::Success
    );
    assert_eq!(
        parallel.tick(&mut reg, &mut ctx).expect("tick"),
        Status::Success
    );
}

#[test]
fn check_supports_not_equal_and_less_equal() {
    let mut registry = Registry::new();
    let mut ctx = Ctx::new(Blackboard::new(), Rng::new(5));
    ctx.blackboard.set("count", Value::Number(2.0), 0.0);
    let mut not_equal =
        BehaviorTree::from_json(r#"{"type":"check","key":"count","op":"!=","value":3}"#)
            .expect("tree");
    let mut less_equal =
        BehaviorTree::from_json(r#"{"type":"check","key":"count","op":"<=","value":2}"#)
            .expect("tree");
    assert_eq!(
        not_equal.tick(&mut registry, &mut ctx).expect("tick"),
        Status::Success
    );
    assert_eq!(
        less_equal.tick(&mut registry, &mut ctx).expect("tick"),
        Status::Success
    );
}

#[test]
fn tree_depth_limit_is_a_build_error_and_random_bytes_never_panic() {
    let mut deep = r#"{"type":"condition","name":"x"}"#.to_owned();
    for _ in 0..500 {
        deep = format!(r#"{{"type":"inverter","child":{deep}}}"#);
    }
    assert!(
        BehaviorTree::from_json(&deep).is_err(),
        "documented maximum nesting depth is 64"
    );
    let mut rng = Rng::new(0x1234);
    for _ in 0..10_000 {
        let len = (rng.next_u64() % 80) as usize;
        let bytes: Vec<u8> = (0..len)
            .map(|_| u8::try_from(rng.next_u64() & u64::from(u8::MAX)).expect("masked byte fits"))
            .collect();
        let input = String::from_utf8_lossy(&bytes);
        let _ = BehaviorTree::from_json(&input);
    }
}

#[test]
fn subtree_library_entries_can_be_reused() {
    let library = BTreeMap::from([(
        "leaf".to_owned(),
        r#"{"type":"action","name":"ok"}"#.to_owned(),
    )]);
    let spec = r#"{"type":"sequence","children":[{"type":"subtree","name":"leaf"},{"type":"subtree","name":"leaf"}]}"#;
    let mut tree = BehaviorTree::from_json_with_library(spec, &library).expect("tree");
    let mut registry = Registry::new();
    let calls = Rc::new(Cell::new(0));
    let seen = Rc::clone(&calls);
    registry.register_action("ok", move |_| {
        seen.set(seen.get() + 1);
        Status::Success
    });
    assert_eq!(
        tree.tick(&mut registry, &mut Ctx::new(Blackboard::new(), Rng::new(8)))
            .expect("tick"),
        Status::Success
    );
    assert_eq!(calls.get(), 2);
}
