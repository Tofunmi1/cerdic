//! G4b: groups and instances at the document level, part 1: creating groups, instancing them,
//! overriding, definition edits, recursion and versions (cases G4b-01 to G4b-10, `docs/modeling/cases.md`).
//! Upgrades, removal, history, persistence, inputs and limits are in `g4_groups_lifecycle.rs`; the
//! evaluation of groups is tested in `ve-modeling` (G4b-E*).
#[path = "support/groups.rs"]
mod support;

use serde_json::{Value as Json, json};
use support::{add_instances, batch, commit, named, overrides, reject, wheel};
use ve_graph::{Command, Outcome, Session};

#[test]
fn g4b_01_creating_a_group_makes_its_nodes_members_not_parts() {
    let (s, id) = wheel();
    let doc = s.document();
    assert_eq!(doc.groups.len(), 1);
    assert_eq!(doc.members(&id).count(), 5);
    assert!(
        doc.nodes.iter().all(|n| n.group.as_ref() == Some(&id)),
        "no top-level node is left"
    );
    assert_eq!(
        doc.group(&id).unwrap().version,
        3,
        "created at 1, two parameters exposed"
    );
    assert_eq!(doc.group(&id).unwrap().output, named(&s, "wheel"));
}

#[test]
fn g4b_02_a_member_may_not_read_a_node_outside_the_group() {
    let mut s = Session::with_builtin(4);
    commit(
        &mut s,
        json!([
            {"add_node": {"op": "solid.box@1", "params": {"size": [1, 1, 1]}, "label": "outside", "alias": "$a"}},
            {"add_node": {"op": "xform.move@1", "params": {"by": [0, 1, 0]}, "label": "inside", "alias": "$m"}},
            {"connect": {"from": {"node": "$a", "port": "out"}, "to": {"node": "$m", "port": "in"}}}
        ]),
    );
    let inside = named(&s, "inside");
    let d = reject(
        &mut s,
        json!([{"create_group": {"name": "g", "members": [inside.as_str()], "output": inside.as_str()}}]),
    );
    assert_eq!(d[0].code, "group.leaks");
    assert!(d[0].related.contains(&named(&s, "outside")));
}

#[test]
fn g4b_03_nothing_outside_may_read_a_member() {
    let mut s = Session::with_builtin(4);
    commit(
        &mut s,
        json!([
            {"add_node": {"op": "solid.box@1", "params": {"size": [1, 1, 1]}, "label": "inside", "alias": "$a"}},
            {"add_node": {"op": "xform.move@1", "params": {"by": [0, 1, 0]}, "label": "reader", "alias": "$m"}},
            {"connect": {"from": {"node": "$a", "port": "out"}, "to": {"node": "$m", "port": "in"}}}
        ]),
    );
    let inside = named(&s, "inside");
    let d = reject(
        &mut s,
        json!([{"create_group": {"name": "g", "members": [inside.as_str()], "output": inside.as_str()}}]),
    );
    assert_eq!(d[0].code, "group.leaks");
    let top = reject(
        &mut s,
        json!([
            {"create_group": {"name": "g", "members": [inside.as_str()], "output": inside.as_str(), "alias": "$g"}},
            {"set_output": {"name": "part", "from": {"node": inside.as_str(), "port": "out"}}}
        ]),
    );
    assert!(
        !top.is_empty(),
        "a group member cannot be a named output either"
    );
}

#[test]
fn g4b_04_four_instances_are_four_nodes_pinned_to_the_current_version() {
    let (mut s, id) = wheel();
    let mut adds = vec![];
    for i in 0..4 {
        let (w, m) = (format!("$w{i}"), format!("$m{i}"));
        adds.push(json!({"add_instance": {"group": id.as_str(), "label": format!("wheel {i}"), "alias": w}}));
        adds.push(json!({"add_node": {"op": "xform.move@1", "params": {"by": [f64::from(i), 0, 0]}, "alias": m}}));
        adds.push(json!({"connect": {"from": {"node": w, "port": "out"}, "to": {"node": m, "port": "in"}}}));
    }
    commit(&mut s, Json::Array(adds));
    let instances: Vec<_> = s.document().instances_of(&id).collect();
    assert_eq!(instances.len(), 4);
    let distinct: std::collections::BTreeSet<_> = instances.iter().map(|n| &n.id).collect();
    assert_eq!(distinct.len(), 4);
    assert!(
        instances
            .iter()
            .all(|n| n.op.to_string() == "group.use@1" && n.group.is_none())
    );
    assert!(
        instances
            .iter()
            .all(|n| n.instance.as_ref().unwrap().version == 3)
    );
}

#[test]
fn g4b_05_an_override_belongs_to_one_instance() {
    let (mut s, id) = wheel();
    add_instances(&mut s, &id, &["a", "b", "c"]);
    let (before, group_before) = (
        s.document().clone(),
        s.document().group(&id).unwrap().clone(),
    );
    let b = named(&s, "b");
    commit(
        &mut s,
        json!([{"set_instance_param": {"node": b.as_str(), "param": "radius", "value": 0.4}}]),
    );
    assert_eq!(overrides(&s, "b").len(), 1);
    assert!(overrides(&s, "a").is_empty() && overrides(&s, "c").is_empty());
    assert_eq!(
        s.document().group(&id).unwrap(),
        &group_before,
        "the definition is untouched"
    );
    let changed: Vec<_> = s
        .document()
        .nodes
        .iter()
        .zip(&before.nodes)
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 1, "only that instance differs");
}

#[test]
fn g4b_06_an_override_is_checked_like_the_parameter_it_stands_for() {
    let (mut s, id) = wheel();
    add_instances(&mut s, &id, &["a"]);
    let a = named(&s, "a");
    let d = reject(
        &mut s,
        json!([{"set_instance_param": {"node": a.as_str(), "param": "radius", "value": -1.0}}]),
    );
    assert_eq!(
        d[0].code, "param.range",
        "the torus's own range, not a new rule"
    );
    let d = reject(
        &mut s,
        json!([{"set_instance_param": {"node": a.as_str(), "param": "spokes", "value": 3}}]),
    );
    assert_eq!(d[0].code, "group.unknown_param");
    assert!(
        d[0].suggestion.as_deref().unwrap().contains("radius"),
        "it lists what is exposed"
    );
    let tyre = named(&s, "tyre");
    let d = reject(
        &mut s,
        json!([{"set_instance_param": {"node": tyre.as_str(), "param": "radius", "value": 1.0}}]),
    );
    assert_eq!(d[0].code, "group.not_instance");
}

#[test]
fn g4b_07_clearing_an_override_returns_to_the_definition() {
    let (mut s, id) = wheel();
    add_instances(&mut s, &id, &["a"]);
    let a = named(&s, "a");
    commit(
        &mut s,
        json!([{"set_instance_param": {"node": a.as_str(), "param": "width", "value": 0.5}}]),
    );
    assert_eq!(overrides(&s, "a").len(), 1);
    commit(
        &mut s,
        json!([{"clear_instance_param": {"node": a.as_str(), "param": "width"}}]),
    );
    assert!(overrides(&s, "a").is_empty());
    let d = reject(
        &mut s,
        json!([{"clear_instance_param": {"node": a.as_str(), "param": "width"}}]),
    );
    assert_eq!(
        d[0].code, "group.unknown_param",
        "there is nothing left to clear"
    );
}

#[test]
fn g4b_08_editing_a_member_edits_the_definition_for_every_instance() {
    let (mut s, id) = wheel();
    add_instances(&mut s, &id, &["a", "b"]);
    let instances: Vec<_> = s.document().instances_of(&id).cloned().collect();
    let version = s.document().group(&id).unwrap().version;
    let tyre = named(&s, "tyre");
    commit(
        &mut s,
        json!([{"set_param": {"node": tyre.as_str(), "param": "minor", "value": 0.08}}]),
    );
    assert_eq!(
        s.document().instances_of(&id).cloned().collect::<Vec<_>>(),
        instances,
        "no instance data or ID changed"
    );
    assert_eq!(
        s.document().group(&id).unwrap().version,
        version,
        "not an interface change"
    );
}

#[test]
fn g4b_09_a_group_may_not_contain_itself() {
    let (mut s, id) = wheel();
    let d = reject(
        &mut s,
        json!([{"add_instance": {"group": id.as_str(), "into": id.as_str()}}]),
    );
    assert_eq!(d[0].code, "group.recursive");
    // Through a second group: B holds an instance of A, then A may not hold an instance of B.
    commit(
        &mut s,
        json!([
            {"add_instance": {"group": id.as_str(), "label": "inner", "alias": "$i"}},
            {"create_group": {"name": "rack", "members": ["$i"], "output": "$i", "alias": "$b"}}
        ]),
    );
    let rack = s
        .document()
        .groups
        .iter()
        .find(|g| g.name == "rack")
        .unwrap()
        .id
        .clone();
    let d = reject(
        &mut s,
        json!([{"add_instance": {"group": rack.as_str(), "into": id.as_str()}}]),
    );
    assert_eq!(d[0].code, "group.recursive");
}

#[test]
fn g4b_10_an_interface_change_raises_the_version_and_leaves_instances_behind() {
    let (mut s, id) = wheel();
    add_instances(&mut s, &id, &["a"]);
    let version = |s: &Session| s.document().group(&id).unwrap().version;
    assert_eq!(version(&s), 3);
    commit(
        &mut s,
        json!([{"unexpose_param": {"group": id.as_str(), "name": "width"}}]),
    );
    assert_eq!(version(&s), 4);
    let (spokes, hub) = (named(&s, "spokes"), named(&s, "hub"));
    commit(
        &mut s,
        json!([{"expose_param": {"group": id.as_str(), "name": "spokes", "node": spokes.as_str(), "param": "count"}}]),
    );
    assert_eq!(version(&s), 5);
    commit(
        &mut s,
        json!([{"set_group_output": {"group": id.as_str(), "node": hub.as_str()}}]),
    );
    assert_eq!(version(&s), 6);
    commit(
        &mut s,
        json!([{"set_group_output": {"group": id.as_str(), "node": hub.as_str()}}]),
    );
    assert_eq!(
        version(&s),
        6,
        "choosing the same result again changes nothing"
    );
    let a = s.document().node(&named(&s, "a")).unwrap();
    assert_eq!(
        a.instance.as_ref().unwrap().version,
        3,
        "the instance keeps the version it was made against"
    );
}

#[test]
fn the_group_commands_use_a_documented_wire_form() {
    let id = "g_0123abcd";
    for json in [
        json!({"add_instance": {"group": id, "label": "w", "into": id}}),
        json!({"set_instance_param": {"node": "n_0123abcd", "param": "radius", "value": 0.4}}),
        json!({"upgrade_group": {"node": "n_0123abcd"}}),
        json!({"expose_param": {"group": id, "name": "radius", "node": "n_0123abcd", "param": "major"}}),
    ] {
        let command: Command = serde_json::from_value(json).unwrap();
        let written = serde_json::to_value(&command).unwrap();
        assert_eq!(serde_json::from_value::<Command>(written).unwrap(), command);
    }
    assert!(
        serde_json::from_str::<Command>(r#"{"add_instance":{"group":"g_0123abcd","evil":1}}"#)
            .is_err()
    );
    // A repeated transaction key applies nothing a second time, also for group commands.
    let (mut s, group) = wheel();
    let once = batch(
        &s,
        json!([{"add_instance": {"group": group.as_str(), "label": "once"}}]),
    );
    assert!(matches!(s.apply(&once), Outcome::Committed { .. }));
    assert!(matches!(s.apply(&once), Outcome::AlreadyApplied { .. }));
    assert_eq!(s.document().instances_of(&group).count(), 1);
}
