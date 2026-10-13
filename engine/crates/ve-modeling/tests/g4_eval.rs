//! G4b evaluation, part 1: a group instanced several times is evaluated once, overrides and definition
//! edits behave as specified, and failures name the instance (cases G4b-E1 to G4b-E8 and G4b-11,
//! `docs/modeling/cases.md`). Inputs, nesting and budgets are in `g4_eval_more.rs`.

#[path = "support/groups.rs"]
mod support;

use serde_json::json;
use support::{commit, evaluate, named, volumes, wheels};
use ve_graph::Session;
use ve_modeling::{Evaluator, Quality, hash};

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance * b.abs()
}

/// The volume the kernel gives a plain torus with the wheel tyre's facets and a ring radius of
/// `major`. The kernel's torus is a polygon (a 16-gon tube turned in 48 steps), so it is about 2.5 %
/// under `2 pi^2 R r^2`; the wheel's tyre must match it exactly, however it was reached.
fn torus(major: f64) -> f64 {
    let mut s = Session::with_builtin(4);
    commit(
        &mut s,
        json!([{"add_node": {"op": "solid.torus@1", "params": {"major": major, "minor": 0.06, "segments": 48, "tube_segments": 16}}}]),
    );
    evaluate(&mut Evaluator::default(), &s).parts[0].volume
}

#[test]
fn e1_four_equal_wheels_evaluate_the_prototype_once() {
    let (s, _) = wheels(4);
    let mut ev = Evaluator::default();
    let e = evaluate(&mut ev, &s);
    assert_eq!(e.parts.len(), 4);
    let v = volumes(&e);
    assert!(v.iter().all(|x| near(*x, v[0], 1e-9)), "{v:?}");
    let (single, _) = wheels(1);
    let one = volumes(&evaluate(&mut Evaluator::default(), &single))[0];
    assert!(
        near(v[0], one, 1e-9),
        "a wheel is the same wheel however many there are"
    );
    assert_eq!(
        e.evaluated_nodes, 9,
        "five operators for the first wheel, none for the other three, four moves"
    );
}

#[test]
fn e2_an_override_changes_one_wheel_by_the_analytic_amount() {
    let (mut s, _) = wheels(4);
    let w1 = named(&s, "w1");
    commit(
        &mut s,
        json!([{"set_instance_param": {"node": w1.as_str(), "param": "radius", "value": 0.4}}]),
    );
    let e = evaluate(&mut Evaluator::default(), &s);
    let v = volumes(&e);
    assert!(
        near(v[0], v[2], 1e-9) && near(v[0], v[3], 1e-9),
        "the others are unchanged: {v:?}"
    );
    let grew = v[1] - v[0];
    // The spoke tips overlap the tyre at 0.3 and not at 0.4, a few parts in a thousand.
    let expected = torus(0.4) - torus(0.3);
    assert!(near(grew, expected, 0.01), "{grew} against {expected}");
    assert_eq!(
        e.evaluated_nodes, 11,
        "the first wheel (5), the tyre and union of the overridden one (2), four moves"
    );
}

#[test]
fn e3_a_definition_edit_reaches_every_instance_and_recomputes_once() {
    let (mut s, _) = wheels(4);
    let mut ev = Evaluator::default();
    let before = volumes(&evaluate(&mut ev, &s));
    let spoke = named(&s, "spoke");
    commit(
        &mut s,
        json!([{"set_param": {"node": spoke.as_str(), "param": "size", "value": [0.22, 0.03, 0.06]}}]),
    );
    let e = evaluate(&mut ev, &s);
    let after = volumes(&e);
    assert!(
        after.iter().all(|x| near(*x, after[0], 1e-9)),
        "all four changed alike"
    );
    assert!(after[0] > before[0], "wider spokes are more material");
    assert_eq!(
        e.evaluated_nodes, 7,
        "the spoke, the pattern and the union once, plus the four moves that read them"
    );
}

#[test]
fn e4_editing_one_instances_move_recomputes_only_that_move() {
    let (mut s, _) = wheels(4);
    let mut ev = Evaluator::default();
    evaluate(&mut ev, &s);
    let m2 = named(&s, "m2");
    commit(
        &mut s,
        json!([{"set_param": {"node": m2.as_str(), "param": "by", "value": [5.0, 0.0, 0.0]}}]),
    );
    let e = evaluate(&mut ev, &s);
    assert_eq!(
        e.evaluated_nodes, 1,
        "the prototype and the other moves come from the cache"
    );
    assert!(e.cache_hits >= 8);
}

#[test]
fn e5_a_material_on_an_instance_reaches_the_part() {
    let (mut s, _) = wheels(1);
    let moved = named(&s, "m0");
    commit(
        &mut s,
        json!([
            {"add_node": {"op": "solid.material@1", "params": {"color": [0.8, 0.1, 0.1], "metallic": 0, "roughness": 0.3}, "label": "paint", "alias": "$p"}},
            {"connect": {"from": {"node": moved.as_str(), "port": "out"}, "to": {"node": "$p", "port": "in"}}}
        ]),
    );
    let e = evaluate(&mut Evaluator::default(), &s);
    assert_eq!(e.parts.len(), 1);
    assert_eq!(e.parts[0].node, named(&s, "paint"));
    assert!(e.parts[0].material.is_some());
}

#[test]
fn e6_a_failure_inside_a_member_names_the_instance_and_the_member() {
    let (mut s, _) = wheels(2);
    let tyre = named(&s, "tyre");
    // A tube thicker than its ring is a kernel refusal, not a parameter range error.
    commit(
        &mut s,
        json!([{"set_param": {"node": tyre.as_str(), "param": "minor", "value": 0.5}}]),
    );
    let error = Evaluator::default()
        .evaluate(s.document(), Quality::Final)
        .unwrap_err();
    assert_eq!(error.code, "kernel.invalid_input");
    let instances: Vec<_> = ["w0", "w1"].iter().map(|l| named(&s, l)).collect();
    assert!(
        error.node.as_ref().is_some_and(|n| instances.contains(n)),
        "the diagnostic is about an instance, not an internal copy: {:?}",
        error.node
    );
    assert_eq!(
        error.related.first(),
        Some(&tyre),
        "and lists the member to look at"
    );
}

#[test]
fn e7_a_muted_instance_has_no_result() {
    let (mut s, _) = wheels(3);
    let w1 = named(&s, "w1");
    commit(
        &mut s,
        json!([{"set_flags": {"node": w1.as_str(), "muted": true}}]),
    );
    let v = volumes(&evaluate(&mut Evaluator::default(), &s));
    assert!(v[1].abs() < f64::EPSILON, "the muted wheel is empty: {v:?}");
    assert!(
        near(v[0], v[2], 1e-9) && v[0] > 0.0,
        "the others are untouched"
    );
}

fn hash_of(s: &Session) -> String {
    hash(s.document())
}

#[test]
fn e8_the_evaluation_hash_follows_what_evaluation_depends_on() {
    let (mut s, id) = wheels(2);
    let base = hash_of(&s);
    let (spoke, w0) = (named(&s, "spoke"), named(&s, "w0"));
    commit(
        &mut s,
        json!([{"editor_set_position": {"node": w0.as_str(), "x": 10.0, "y": 20.0}}]),
    );
    commit(
        &mut s,
        json!([{"set_group_name": {"group": id.as_str(), "name": "tyre_and_hub"}}]),
    );
    commit(
        &mut s,
        json!([{"set_label": {"node": w0.as_str(), "label": "front wheel"}}]),
    );
    assert_eq!(
        hash_of(&s),
        base,
        "layout, renaming and labels do not matter"
    );
    let mut seen = vec![base];
    for command in [
        json!({"set_param": {"node": spoke.as_str(), "param": "size", "value": [0.22, 0.03, 0.04]}}),
        json!({"set_instance_param": {"node": w0.as_str(), "param": "radius", "value": 0.35}}),
        json!({"expose_param": {"group": id.as_str(), "name": "count", "node": named(&s, "spokes").as_str(), "param": "count"}}),
        json!({"set_group_output": {"group": id.as_str(), "node": named(&s, "hub").as_str()}}),
    ] {
        commit(&mut s, json!([command]));
        let now = hash_of(&s);
        assert!(!seen.contains(&now), "each of these changes what evaluates");
        seen.push(now);
    }
}

#[test]
fn g4b_11_an_instance_behind_its_group_is_refused_by_name() {
    let (mut s, id) = wheels(3);
    commit(
        &mut s,
        json!([{"unexpose_param": {"group": id.as_str(), "name": "width"}}]),
    );
    let error = Evaluator::default()
        .evaluate(s.document(), Quality::Final)
        .unwrap_err();
    assert_eq!(error.code, "group.outdated");
    let instances: Vec<_> = ["w0", "w1", "w2"].iter().map(|l| named(&s, l)).collect();
    assert!(error.node.as_ref().is_some_and(|n| instances.contains(n)));
    // Upgrading each instance (an explicit edit) makes the document evaluate again.
    let commands: Vec<_> = instances
        .iter()
        .map(|i| json!({"upgrade_group": {"node": i.as_str()}}))
        .collect();
    commit(&mut s, json!(commands));
    assert_eq!(evaluate(&mut Evaluator::default(), &s).parts.len(), 3);
}
