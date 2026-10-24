//! Additional utility and HTN edge case contract tests.
#![expect(clippy::float_cmp, reason = "contract assertions use exact values")]

use std::collections::BTreeMap;

use ve_npc::{
    Blackboard, Ctx, Domain, Plan, PlanLimits, PlanRunner, Registry, Rng, RunStatus, Status,
    UtilityBrain, Value,
};

fn ctx() -> Ctx {
    Ctx::new(Blackboard::new(), Rng::new(1))
}
fn no_score(_: &str) -> f64 {
    0.0
}

#[test]
fn utility_boundaries_compensation_momentum_and_ties() {
    let json = r#"{"options":[{"name":"step","considerations":[{"key":"x","curve":{"type":"step","threshold":2}}]},{"name":"inv","considerations":[{"key":"x","curve":{"type":"inverse","min":0,"max":1}}]},{"name":"zero","weight":0,"considerations":[]},{"name":"three","considerations":[{"key":"a","curve":{"type":"linear","min":0,"max":1}},{"key":"b","curve":{"type":"linear","min":0,"max":1}},{"key":"c","curve":{"type":"linear","min":0,"max":1}}]}]}"#;
    let mut brain = UtilityBrain::from_json(json).expect("valid");
    let mut c = ctx();
    c.blackboard.set("x", Value::Number(2.0), 0.0);
    for key in ["a", "b", "c"] {
        c.blackboard.set(key, Value::Number(0.5), 0.0);
    }
    let out = brain.choose(&c).expect("choice");
    assert_eq!(
        out.ranking.iter().find(|x| x.0 == "step").expect("step").1,
        1.0
    );
    assert_eq!(
        out.ranking
            .iter()
            .find(|x| x.0 == "inv")
            .expect("inverse")
            .1,
        0.0
    );
    assert_eq!(
        out.ranking.iter().find(|x| x.0 == "zero").expect("zero").1,
        0.0
    );
    assert!(
        (out.ranking
            .iter()
            .find(|x| x.0 == "three")
            .expect("three")
            .1
            - 0.197_916_666_666_666_66)
            .abs()
            < 1e-12
    );
    assert_eq!(out.ranking[0].0, "step");
    let mut m = UtilityBrain::from_json(r#"{"momentum":0,"options":[{"name":"a","considerations":[]},{"name":"b","considerations":[]}] }"#)
        .expect("valid");
    let tied = m.choose(&c).expect("first");
    assert_eq!(tied.name.as_deref(), Some("a"));
    assert_eq!(tied.ranking[0].0, "a");
    assert_eq!(tied.ranking[1].0, "b");
    assert_eq!(m.choose(&c).expect("second").name.as_deref(), Some("a"));
    let mut forget=UtilityBrain::from_json(r#"{"options":[{"name":"a","considerations":[{"key":"a","curve":{"type":"linear","min":0,"max":1}}]},{"name":"b","considerations":[{"key":"b","curve":{"type":"linear","min":0,"max":1}}]}]}"#).expect("valid");
    c.blackboard.set("a", Value::Number(1.0), 0.0);
    c.blackboard.set("b", Value::Number(0.0), 0.0);
    assert_eq!(forget.choose(&c).expect("a").name.as_deref(), Some("a"));
    c.blackboard.set("a", Value::Number(0.0), 0.0);
    c.blackboard.set("b", Value::Number(0.0), 0.0);
    assert_eq!(forget.choose(&c).expect("none").name, None);
    c.blackboard.set("a", Value::Number(0.0), 0.0);
    c.blackboard.set("b", Value::Number(1.0), 0.0);
    assert_eq!(forget.choose(&c).expect("b").name.as_deref(), Some("b"));
}

#[test]
fn htn_effects_preconditions_siblings_and_limits() {
    let d=Domain::from_json(r#"{"tasks":{"goal":{"methods":[{"name":"empty","when":[{"utility":{"option":"x","min":0.5}}],"do":[]}]},"both":{"methods":[{"name":"two","when":[],"do":["set","check","set","check"]}]},"set":{"primitive":"a","effects":[{"add":{"key":"n","amount":1}},{"set":{"key":"text","value":"ok"}}]},"check":{"primitive":"c","pre":[{"key":"text","op":"==","value":"ok"}]}}}"#).expect("domain");
    let empty = d
        .plan(&BTreeMap::new(), "goal", &PlanLimits::default(), &|_| 0.5)
        .expect("empty plan");
    assert!(empty.steps.is_empty());
    let p = d
        .plan(&BTreeMap::new(), "both", &PlanLimits::default(), &no_score)
        .expect("shared task");
    assert_eq!(p.final_state["n"], Value::Number(2.0));
    let add = Domain::from_json(
        r#"{"tasks":{"x":{"primitive":"a","effects":[{"add":{"key":"n","amount":2}}]}}}"#,
    )
    .expect("domain")
    .plan(&BTreeMap::new(), "x", &PlanLimits::default(), &no_score)
    .expect("plan");
    assert_eq!(add.final_state["n"], Value::Number(2.0));
    let node = d.plan(
        &BTreeMap::new(),
        "goal",
        &PlanLimits {
            max_depth: 1,
            max_nodes: 1,
        },
        &|_| 0.5,
    );
    assert!(node.is_ok());
    let exact = Domain::from_json(r#"{"tasks":{"one":{"primitive":"a"}}}"#)
        .expect("domain")
        .plan(
            &BTreeMap::new(),
            "one",
            &PlanLimits {
                max_depth: 1,
                max_nodes: 1,
            },
            &no_score,
        );
    assert!(exact.is_ok(), "limits are inclusive");
    let depth = Domain::from_json(
        r#"{"tasks":{"a":{"methods":[{"name":"m","when":[],"do":["b"]}]},"b":{"primitive":"b"}}}"#,
    )
    .expect("domain")
    .plan(
        &BTreeMap::new(),
        "a",
        &PlanLimits {
            max_depth: 1,
            max_nodes: 10,
        },
        &no_score,
    );
    assert!(depth.is_err());
}

#[test]
fn runner_handles_empty_plan_and_successes_in_one_tick() {
    let mut empty = PlanRunner::new(Plan {
        steps: vec![],
        final_state: BTreeMap::new(),
    });
    let mut r = Registry::new();
    assert_eq!(
        empty.tick(&mut r, &mut ctx()).expect("tick"),
        RunStatus::Done
    );
    let mut plan = PlanRunner::new(Plan {
        steps: (0..4)
            .map(|i| ve_npc::Step {
                task: i.to_string(),
                action: "ok".into(),
            })
            .collect(),
        final_state: BTreeMap::new(),
    });
    r.register_action("ok", |_| Status::Success);
    assert_eq!(
        plan.tick(&mut r, &mut ctx()).expect("tick"),
        RunStatus::Done
    );
}

#[test]
fn seeded_random_bytes_never_panic_parsers() {
    let mut seed = 0x1234_5678_u64;
    for _ in 0..3000 {
        let len = usize::try_from(seed % 64).unwrap_or_default();
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                u8::try_from(seed & 255).unwrap_or_default()
            })
            .collect();
        let s = String::from_utf8_lossy(&bytes);
        let _ = UtilityBrain::from_json(&s);
        let _ = Domain::from_json(&s);
    }
}
