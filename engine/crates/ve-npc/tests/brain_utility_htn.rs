//! Contract tests for the deciding layer of an NPC's mind (ADR 0063, step M3c): a utility brain that scores options from the blackboard with
//! response curves and some momentum, and an HTN planner that decomposes a goal into primitive actions through ordered methods, with a utility
//! score as a precondition (Guerrilla's pattern), plus a runner that executes a plan step by step.
#![expect(
    clippy::float_cmp,
    reason = "tests assert by failing and compare values the contract fixes"
)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ve_npc::{
    Blackboard, Ctx, Domain, HtnError, PlanLimits, PlanRunner, Registry, Rng, RunStatus, Status,
    UtilityBrain, Value,
};

fn ctx() -> Ctx {
    Ctx::new(Blackboard::new(), Rng::new(3))
}

fn state(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

// ---- utility ----

const NEEDS: &str = r#"{
  "options": [
    {"name": "eat",   "considerations": [{"key": "hunger", "curve": {"type": "linear", "min": 0, "max": 1}}]},
    {"name": "sleep", "considerations": [{"key": "energy", "curve": {"type": "inverse", "min": 0, "max": 1}}]},
    {"name": "work",  "weight": 0.5, "considerations": []}
  ]
}"#;

#[test]
fn an_option_scores_its_consideration_through_its_curve() {
    let mut brain = UtilityBrain::from_json(NEEDS).expect("valid");
    let mut c = ctx();
    c.blackboard.set("hunger", n(0.8), 0.0);
    c.blackboard.set("energy", n(0.9), 0.0);
    let choice = brain.choose(&c).expect("chooses");
    assert_eq!(choice.name.as_deref(), Some("eat"));
    let scores: BTreeMap<_, _> = choice.ranking.iter().cloned().collect();
    assert!((scores["eat"] - 0.8).abs() < 1e-9, "linear 0.8");
    assert!((scores["sleep"] - 0.1).abs() < 1e-9, "inverse of 0.9");
    assert!(
        (scores["work"] - 0.5).abs() < 1e-9,
        "no considerations: just the weight"
    );
    let order: Vec<_> = choice
        .ranking
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(order, vec!["eat", "work", "sleep"], "ranked best first");
}

#[test]
fn values_outside_the_range_are_clamped_and_a_missing_key_scores_nothing() {
    let mut brain = UtilityBrain::from_json(NEEDS).expect("valid");
    let mut c = ctx();
    c.blackboard.set("hunger", n(7.0), 0.0);
    let choice = brain.choose(&c).expect("chooses");
    let scores: BTreeMap<_, _> = choice.ranking.iter().cloned().collect();
    assert_eq!(scores["eat"], 1.0, "clamped to the top of the range");
    assert_eq!(scores["sleep"], 0.0, "energy was never set");
}

#[test]
fn several_considerations_multiply_with_a_compensation_for_their_number() {
    let json = r#"{"options":[{"name":"attack","considerations":[
        {"key":"a","curve":{"type":"linear","min":0,"max":1}},
        {"key":"b","curve":{"type":"linear","min":0,"max":1}}]}]}"#;
    let mut brain = UtilityBrain::from_json(json).expect("valid");
    let mut c = ctx();
    c.blackboard.set("a", n(0.5), 0.0);
    c.blackboard.set("b", n(0.5), 0.0);
    // product 0.25; makeup = 1 - 1/2 = 0.5; modification = (1 - 0.25) * 0.5 = 0.375; final = 0.25 + 0.375 * 0.25.
    let choice = brain.choose(&c).expect("chooses");
    assert!((choice.score - 0.343_75).abs() < 1e-9, "{}", choice.score);
}

#[test]
fn every_curve_shape_is_deterministic_algebra() {
    let json = r#"{"options":[
        {"name":"quad","considerations":[{"key":"x","curve":{"type":"quadratic","min":0,"max":10,"power":2}}]},
        {"name":"step","considerations":[{"key":"x","curve":{"type":"step","threshold":6}}]},
        {"name":"soft","considerations":[{"key":"x","curve":{"type":"soft_step","mid":5,"steepness":1}}]}]}"#;
    let mut brain = UtilityBrain::from_json(json).expect("valid");
    let mut c = ctx();
    c.blackboard.set("x", n(5.0), 0.0);
    let scores: BTreeMap<_, _> = brain
        .choose(&c)
        .expect("chooses")
        .ranking
        .into_iter()
        .collect();
    assert!((scores["quad"] - 0.25).abs() < 1e-9, "(5/10)^2");
    assert_eq!(scores["step"], 0.0, "5 is under the threshold of 6");
    assert!(
        (scores["soft"] - 0.5).abs() < 1e-9,
        "at the midpoint, 0.5 + 0.5 * t / (1 + |t|) with t = 0"
    );
    c.blackboard.set("x", n(7.0), 0.0);
    let scores: BTreeMap<_, _> = brain
        .choose(&c)
        .expect("chooses")
        .ranking
        .into_iter()
        .collect();
    assert_eq!(scores["step"], 1.0);
    assert!(
        (scores["soft"] - (0.5 + 0.5 * 2.0 / 3.0)).abs() < 1e-9,
        "t = 2: 0.5 + 0.5 * 2 / 3"
    );
}

#[test]
fn momentum_keeps_the_current_choice_unless_another_is_clearly_better() {
    let json = r#"{"momentum":0.2,"options":[
        {"name":"eat","considerations":[{"key":"hunger","curve":{"type":"linear","min":0,"max":1}}]},
        {"name":"sleep","considerations":[{"key":"tired","curve":{"type":"linear","min":0,"max":1}}]}]}"#;
    let mut brain = UtilityBrain::from_json(json).expect("valid");
    let mut c = ctx();
    c.blackboard.set("hunger", n(0.6), 0.0);
    c.blackboard.set("tired", n(0.2), 0.0);
    assert_eq!(
        brain.choose(&c).expect("chooses").name.as_deref(),
        Some("eat")
    );
    c.blackboard.set("tired", n(0.65), 0.1);
    assert_eq!(
        brain.choose(&c).expect("chooses").name.as_deref(),
        Some("eat"),
        "0.6 * 1.2 = 0.72 beats 0.65"
    );
    c.blackboard.set("tired", n(0.9), 0.2);
    assert_eq!(
        brain.choose(&c).expect("chooses").name.as_deref(),
        Some("sleep"),
        "0.9 beats 0.72"
    );
    c.blackboard.set("hunger", n(0.8), 0.3);
    assert_eq!(
        brain.choose(&c).expect("chooses").name.as_deref(),
        Some("sleep"),
        "0.9 * 1.2 = 1.08 beats 0.8"
    );
}

#[test]
fn ties_go_to_the_first_option_and_nothing_scores_nothing() {
    let json = r#"{"options":[{"name":"a","weight":0.5,"considerations":[]},{"name":"b","weight":0.5,"considerations":[]}]}"#;
    let mut brain = UtilityBrain::from_json(json).expect("valid");
    assert_eq!(
        brain.choose(&ctx()).expect("chooses").name.as_deref(),
        Some("a")
    );
    let mut none = UtilityBrain::from_json(r#"{"options":[{"name":"a","considerations":[{"key":"missing","curve":{"type":"linear","min":0,"max":1}}]}]}"#).expect("valid");
    let choice = none.choose(&ctx()).expect("chooses");
    assert_eq!(choice.name, None, "every option scored zero");
}

#[test]
fn bad_utility_specs_are_refused() {
    assert!(UtilityBrain::from_json("{nope").is_err());
    assert!(UtilityBrain::from_json(r#"{"options":[]}"#).is_err());
    assert!(
        UtilityBrain::from_json(
            r#"{"options":[{"name":"a","considerations":[{"key":"k","curve":{"type":"wavy"}}]}]}"#
        )
        .is_err()
    );
    assert!(UtilityBrain::from_json(r#"{"options":[{"name":"a","considerations":[{"key":"k","curve":{"type":"linear","min":1,"max":1}}]}]}"#).is_err(), "an empty range");
    assert!(
        UtilityBrain::from_json(
            r#"{"options":[{"name":"a","considerations":[]},{"name":"a","considerations":[]}]}"#
        )
        .is_err(),
        "duplicate names"
    );
}

// ---- HTN ----

const DOMAIN: &str = r#"{
  "tasks": {
    "get_food": {"methods": [
      {"name": "buy",  "when": [{"key": "money", "op": ">=", "value": 5}], "do": ["walk_to_shop", "buy_food"]},
      {"name": "hunt", "when": [], "do": ["hunt_deer"]}
    ]},
    "walk_to_shop": {"primitive": "walk", "pre": [], "effects": [{"set": {"key": "at", "value": "shop"}}]},
    "buy_food": {"primitive": "buy", "pre": [{"key": "at", "op": "==", "value": "shop"}, {"key": "money", "op": ">=", "value": 5}],
                 "effects": [{"add": {"key": "money", "amount": -5}}, {"add": {"key": "food", "amount": 1}}]},
    "hunt_deer": {"primitive": "hunt", "pre": [], "effects": [{"add": {"key": "food", "amount": 1}}]},
    "cook": {"primitive": "cook", "pre": [{"key": "food", "op": ">=", "value": 1}], "effects": [{"add": {"key": "food", "amount": -1}}, {"add": {"key": "meals", "amount": 1}}]},
    "make_meal": {"methods": [{"name": "default", "when": [], "do": ["get_food", "cook"]}]}
  }
}"#;

fn no_scores(_: &str) -> f64 {
    0.0
}

fn actions(plan: &ve_npc::Plan) -> Vec<String> {
    plan.steps.iter().map(|s| s.action.clone()).collect()
}

#[test]
fn methods_are_tried_in_order_and_the_first_that_applies_wins() {
    let domain = Domain::from_json(DOMAIN).expect("valid");
    let rich = state(&[("money", n(10.0))]);
    let plan = domain
        .plan(&rich, "get_food", &PlanLimits::default(), &no_scores)
        .expect("a plan");
    assert_eq!(actions(&plan), vec!["walk", "buy"]);
    let poor = state(&[("money", n(0.0))]);
    let plan = domain
        .plan(&poor, "get_food", &PlanLimits::default(), &no_scores)
        .expect("a plan");
    assert_eq!(actions(&plan), vec!["hunt"]);
}

#[test]
fn effects_flow_through_the_plan_and_the_final_state_is_reported() {
    let domain = Domain::from_json(DOMAIN).expect("valid");
    let plan = domain
        .plan(
            &state(&[("money", n(10.0))]),
            "make_meal",
            &PlanLimits::default(),
            &no_scores,
        )
        .expect("a plan");
    assert_eq!(
        actions(&plan),
        vec!["walk", "buy", "cook"],
        "cook needs the food that buying produced"
    );
    assert_eq!(plan.final_state["money"], n(5.0));
    assert_eq!(plan.final_state["meals"], n(1.0));
    assert_eq!(plan.final_state["food"], n(0.0));
}

#[test]
fn a_method_whose_later_step_cannot_run_is_abandoned_for_the_next() {
    let json = r#"{"tasks":{
        "goal":{"methods":[
            {"name":"first","when":[],"do":["easy","impossible"]},
            {"name":"second","when":[],"do":["fallback"]}]},
        "easy":{"primitive":"easy","pre":[],"effects":[{"set":{"key":"x","value":true}}]},
        "impossible":{"primitive":"never","pre":[{"key":"nope","op":"==","value":true}],"effects":[]},
        "fallback":{"primitive":"fallback","pre":[],"effects":[]}}}"#;
    let domain = Domain::from_json(json).expect("valid");
    let plan = domain
        .plan(&BTreeMap::new(), "goal", &PlanLimits::default(), &no_scores)
        .expect("backtracks");
    assert_eq!(actions(&plan), vec!["fallback"]);
    assert!(
        !plan.final_state.contains_key("x"),
        "the abandoned method left no trace in the state"
    );
}

#[test]
fn a_utility_score_can_be_a_precondition() {
    let json = r#"{"tasks":{
        "engage":{"methods":[
            {"name":"attack","when":[{"utility":{"option":"attack","min":0.5}}],"do":["strike"]},
            {"name":"circle","when":[],"do":["circle"]}]},
        "strike":{"primitive":"strike","pre":[],"effects":[]},
        "circle":{"primitive":"circle","pre":[],"effects":[]}}}"#;
    let domain = Domain::from_json(json).expect("valid");
    let eager = |option: &str| if option == "attack" { 0.7 } else { 0.0 };
    let shy = |option: &str| if option == "attack" { 0.3 } else { 0.0 };
    let limits = PlanLimits::default();
    assert_eq!(
        actions(
            &domain
                .plan(&BTreeMap::new(), "engage", &limits, &eager)
                .expect("plan")
        ),
        vec!["strike"]
    );
    assert_eq!(
        actions(
            &domain
                .plan(&BTreeMap::new(), "engage", &limits, &shy)
                .expect("plan")
        ),
        vec!["circle"]
    );
}

#[test]
fn recursion_is_allowed_but_bounded_by_depth_and_node_limits() {
    let travel = r#"{"tasks":{
        "travel":{"methods":[
            {"name":"arrived","when":[{"key":"left","op":"<=","value":0}],"do":[]},
            {"name":"step","when":[{"key":"left","op":">","value":0}],"do":["move","travel"]}]},
        "move":{"primitive":"move","pre":[],"effects":[{"add":{"key":"left","amount":-1}}]}}}"#;
    let domain = Domain::from_json(travel).expect("valid");
    let plan = domain
        .plan(
            &state(&[("left", n(4.0))]),
            "travel",
            &PlanLimits::default(),
            &no_scores,
        )
        .expect("plan");
    assert_eq!(actions(&plan).len(), 4);
    let far = domain.plan(
        &state(&[("left", n(1000.0))]),
        "travel",
        &PlanLimits {
            max_depth: 16,
            max_nodes: 10_000,
        },
        &no_scores,
    );
    assert!(matches!(far, Err(HtnError::Limit(_))), "{far:?}");
    let forever = r#"{"tasks":{"loop":{"methods":[{"name":"again","when":[],"do":["loop"]}]}}}"#;
    let domain = Domain::from_json(forever).expect("valid");
    let result = domain.plan(
        &BTreeMap::new(),
        "loop",
        &PlanLimits {
            max_depth: 32,
            max_nodes: 100_000,
        },
        &no_scores,
    );
    assert!(
        matches!(result, Err(HtnError::Limit(_))),
        "no stack overflow, a limit error: {result:?}"
    );
}

#[test]
fn no_applicable_method_is_no_plan_and_bad_names_are_errors() {
    let domain = Domain::from_json(DOMAIN).expect("valid");
    assert!(
        matches!(
            domain.plan(
                &state(&[("money", n(0.0))]),
                "cook",
                &PlanLimits::default(),
                &no_scores
            ),
            Err(HtnError::NoPlan(_))
        ),
        "cook needs food"
    );
    assert!(matches!(
        domain.plan(
            &BTreeMap::new(),
            "ghost",
            &PlanLimits::default(),
            &no_scores
        ),
        Err(HtnError::UnknownTask(_))
    ));
    assert!(
        Domain::from_json(
            r#"{"tasks":{"a":{"methods":[{"name":"m","when":[],"do":["missing"]}]}}}"#
        )
        .is_err(),
        "a method names a task that does not exist"
    );
    assert!(Domain::from_json("{nope").is_err());
    assert!(
        Domain::from_json(r#"{"tasks":{"a":{"methods":[]}}}"#).is_err(),
        "a compound task needs at least one method"
    );
    let validated = Domain::from_json(DOMAIN).expect("valid");
    let error = validated
        .validate(&Registry::new())
        .expect_err("no actions registered")
        .to_string();
    for name in ["walk", "buy", "hunt", "cook"] {
        assert!(error.contains(name), "{name} missing from: {error}");
    }
}

#[test]
fn planning_is_deterministic_and_the_domain_round_trips() {
    let domain = Domain::from_json(DOMAIN).expect("valid");
    let s = state(&[("money", n(10.0))]);
    let a = domain
        .plan(&s, "make_meal", &PlanLimits::default(), &no_scores)
        .expect("plan");
    let b = domain
        .plan(&s, "make_meal", &PlanLimits::default(), &no_scores)
        .expect("plan");
    assert_eq!(actions(&a), actions(&b));
    assert_eq!(
        Domain::from_json(&domain.to_json())
            .expect("reloads")
            .to_json(),
        domain.to_json()
    );
}

#[test]
fn a_runner_executes_a_plan_one_registered_action_at_a_time() {
    let domain = Domain::from_json(DOMAIN).expect("valid");
    let plan = domain
        .plan(
            &state(&[("money", n(10.0))]),
            "make_meal",
            &PlanLimits::default(),
            &no_scores,
        )
        .expect("plan");
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let mut reg = Registry::new();
    for name in ["walk", "buy", "cook", "hunt"] {
        let (log, label) = (Rc::clone(&log), name.to_owned());
        reg.register_action(name, move |_| {
            log.borrow_mut().push(label.clone());
            if label == "buy" && log.borrow().iter().filter(|e| *e == "buy").count() == 1 {
                Status::Running
            } else {
                Status::Success
            }
        });
    }
    let mut runner = PlanRunner::new(plan);
    let mut c = ctx();
    assert_eq!(
        runner.tick(&mut reg, &mut c).expect("tick"),
        RunStatus::Running,
        "walk done, buy is running"
    );
    assert_eq!(
        runner.tick(&mut reg, &mut c).expect("tick"),
        RunStatus::Done,
        "buy finishes, cook runs, nothing is left"
    );
    assert_eq!(*log.borrow(), vec!["walk", "buy", "buy", "cook"]);
    assert_eq!(
        runner.tick(&mut reg, &mut c).expect("tick"),
        RunStatus::Done,
        "ticking a finished plan stays done"
    );
}

#[test]
fn a_failing_step_fails_the_run_so_the_caller_can_replan() {
    let domain = Domain::from_json(DOMAIN).expect("valid");
    let plan = domain
        .plan(
            &state(&[("money", n(10.0))]),
            "get_food",
            &PlanLimits::default(),
            &no_scores,
        )
        .expect("plan");
    let mut reg = Registry::new();
    reg.register_action("walk", |_| Status::Success);
    reg.register_action("buy", |_| Status::Failure);
    let mut runner = PlanRunner::new(plan);
    let mut c = ctx();
    assert_eq!(
        runner.tick(&mut reg, &mut c).expect("tick"),
        RunStatus::Failed { step: 1 }
    );
    assert_eq!(
        runner.tick(&mut reg, &mut c).expect("tick"),
        RunStatus::Failed { step: 1 },
        "it stays failed until replaced"
    );
}
