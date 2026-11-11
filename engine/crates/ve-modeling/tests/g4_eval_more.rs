//! G4b evaluation, part 2: group inputs, bypass, nesting, the expansion budget and recursion in a
//! loaded document (cases G4b-17, G4b-18, G4b-19 and G4b-E7).

#[path = "support/groups.rs"]
mod support;

use std::fmt::Write;

use serde_json::json;
use support::{commit, evaluate, named, volumes, wheels};
use ve_graph::{Instance, Node, OpId, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance * b.abs()
}

/// A bracket group (a plate and a `mount` input joined by a union) and an instance of it, `bracket`,
/// and a 1 m cube `post` that is not linked to it yet.
fn bracket() -> Session {
    let mut s = Session::with_builtin(4);
    commit(
        &mut s,
        json!([
            {"add_node": {"op": "solid.box@1", "params": {"size": [0.1, 0.1, 0.1]}, "label": "plate", "alias": "$b"}},
            {"create_group": {"name": "bracket", "members": ["$b"], "output": "$b", "alias": "$g"}},
            {"add_node": {"op": "group.input@1", "params": {"name": "mount"}, "label": "mount", "alias": "$in", "group": "$g"}},
            {"add_node": {"op": "solid.union@1", "label": "joined", "alias": "$u", "group": "$g"}},
            {"connect": {"from": {"node": "$in", "port": "out"}, "to": {"node": "$u", "port": "in"}}},
            {"connect": {"from": {"node": "$b", "port": "out"}, "to": {"node": "$u", "port": "in"}}},
            {"set_group_output": {"group": "$g", "node": "$u"}},
            {"add_instance": {"group": "$g", "label": "bracket"}},
            {"add_node": {"op": "solid.box@1", "params": {"size": [1, 1, 1]}, "label": "post"}}
        ]),
    );
    s
}

fn link_post(s: &mut Session) {
    let (post, instance) = (named(s, "post"), named(s, "bracket"));
    commit(
        s,
        json!([{"connect": {"from": {"node": post.as_str(), "port": "out"}, "to": {"node": instance.as_str(), "port": "mount"}}}]),
    );
}

#[test]
fn g4b_17_a_group_input_takes_what_the_instance_is_linked_to() {
    let mut s = bracket();
    link_post(&mut s);
    let e = evaluate(&mut Evaluator::default(), &s);
    let parts: Vec<_> = e.parts.iter().map(|p| p.node.clone()).collect();
    assert!(
        parts.contains(&named(&s, "bracket")),
        "the part is the instance itself, so it keeps its identity: {parts:?}"
    );
    let bracket_volume = e
        .parts
        .iter()
        .find(|p| p.node == named(&s, "bracket"))
        .unwrap()
        .volume;
    assert!(
        near(bracket_volume, 1.0, 1e-6),
        "the plate sits inside the post: {bracket_volume}"
    );
}

#[test]
fn g4b_17_an_unlinked_group_input_is_refused_naming_the_instance() {
    let s = bracket();
    let error = Evaluator::default()
        .evaluate(s.document(), Quality::Final)
        .unwrap_err();
    assert_eq!(error.code, "graph.missing_input");
    assert_eq!(error.node, Some(named(&s, "bracket")));
    assert!(error.message.contains("mount"), "{}", error.message);
}

#[test]
fn e7_a_bypassed_instance_passes_its_first_input_through() {
    let mut s = bracket();
    link_post(&mut s);
    let instance = named(&s, "bracket");
    commit(
        &mut s,
        json!([{"set_flags": {"node": instance.as_str(), "bypass": true}}]),
    );
    let e = evaluate(&mut Evaluator::default(), &s);
    let shown = e.parts.iter().find(|p| p.node == instance).unwrap();
    assert!(
        near(shown.volume, 1.0, 1e-6),
        "the post comes through unchanged"
    );
    assert_eq!(
        e.evaluated_nodes, 2,
        "the post and the pass-through; the group is not unfolded"
    );
}

#[test]
fn g4b_18_nested_groups_unfold_and_still_share() {
    let (mut s, wheel) = wheels(0);
    commit(
        &mut s,
        json!([
            {"add_instance": {"group": wheel.as_str(), "label": "left", "alias": "$l"}},
            {"add_instance": {"group": wheel.as_str(), "label": "right", "alias": "$r"}},
            {"add_node": {"op": "solid.union@1", "label": "axle", "alias": "$u"}},
            {"connect": {"from": {"node": "$l", "port": "out"}, "to": {"node": "$u", "port": "in"}}},
            {"connect": {"from": {"node": "$r", "port": "out"}, "to": {"node": "$u", "port": "in"}}},
            {"create_group": {"name": "axle", "members": ["$l", "$r", "$u"], "output": "$u", "alias": "$g"}},
            {"add_instance": {"group": "$g", "label": "front"}},
            {"add_instance": {"group": "$g", "label": "back"}}
        ]),
    );
    let e = evaluate(&mut Evaluator::default(), &s);
    assert_eq!(e.parts.len(), 2);
    assert_eq!(
        (e.parts[0].node.clone(), e.parts[1].node.clone()),
        (named(&s, "front"), named(&s, "back")),
        "each axle is a part under its own instance"
    );
    let (single, _) = wheels(1);
    let one = volumes(&evaluate(&mut Evaluator::default(), &single))[0];
    let v = volumes(&e);
    assert!(
        near(v[0], one, 1e-9) && near(v[1], one, 1e-9),
        "two equal wheels at one place union to one: {v:?}"
    );
    assert_eq!(
        e.evaluated_nodes, 6,
        "five operators for the wheel, one union for the axle, shared by every copy"
    );
}

#[test]
fn g4b_19_unfolding_too_much_is_refused_before_any_kernel_call() {
    let mut s = Session::with_builtin(4);
    commit(
        &mut s,
        json!([
            {"add_node": {"op": "solid.box@1", "params": {"size": [1, 1, 1]}, "alias": "$b"}},
            {"create_group": {"name": "g0", "members": ["$b"], "output": "$b"}}
        ]),
    );
    // Each level is eight instances of the one below, joined: 8^5 boxes in the top group.
    for level in 1..=5 {
        let previous = s.document().groups.last().unwrap().id.clone();
        let mut commands = Vec::new();
        let mut members = vec!["$u".to_owned()];
        commands.push(json!({"add_node": {"op": "solid.union@1", "alias": "$u"}}));
        for i in 0..8 {
            let alias = format!("$i{i}");
            commands.push(json!({"add_instance": {"group": previous.as_str(), "alias": alias}}));
            commands.push(json!({"connect": {"from": {"node": alias, "port": "out"}, "to": {"node": "$u", "port": "in"}}}));
            members.push(alias);
        }
        commands.push(json!({"create_group": {"name": format!("g{level}"), "members": members, "output": "$u"}}));
        commit(&mut s, json!(commands));
    }
    let top = s.document().groups.last().unwrap().id.clone();
    commit(
        &mut s,
        json!([{"add_instance": {"group": top.as_str(), "label": "big"}}]),
    );
    let mut ev = Evaluator::default();
    let error = ev.begin(s.document(), Quality::Final).unwrap_err();
    assert_eq!(error.code, "budget.expanded_nodes");
    assert_eq!(error.limit, Some(20_000.0));
    assert!(error.observed.unwrap() > 20_000.0);
    assert_eq!(ev.evaluated_nodes(), 0, "no kernel call was made");
}

#[test]
fn g4e_02_a_group_written_as_text_evaluates_like_the_one_the_editor_makes() {
    let mut text = String::from(
        "group wheel\n  tyre = solid.torus@1(major=0.3, minor=0.06, segments=48, tube_segments=16)\n  hub = solid.cylinder@1(height=0.2, radius=0.08, segments=32)\n  spoke = solid.box@1(size=[0.22, 0.03, 0.03])\n  spokes = xform.radial@1(spoke, count=4, radius=0.14)\n  wheel = solid.union@1(tyre, hub, spokes)\n  expose radius = tyre.major\n  expose width = hub.height\n  result wheel\nend\n",
    );
    for i in 0..4 {
        writeln!(
            text,
            "w{i} = wheel()\nm{i} = xform.move@1(w{i}, by=[{i}, 0, 0])"
        )
        .unwrap();
    }
    let batch = parse_text(&builtin_registry(), &text).unwrap();
    let mut s = Session::with_builtin(4);
    commit(&mut s, serde_json::to_value(&batch.commands).unwrap());
    let from_text = evaluate(&mut Evaluator::default(), &s);
    let (editor, _) = wheels(4);
    let from_editor = evaluate(&mut Evaluator::default(), &editor);
    assert_eq!(from_text.parts.len(), 4);
    for (a, b) in volumes(&from_text).iter().zip(volumes(&from_editor)) {
        assert!(near(*a, b, 1e-9), "{a} against {b}");
    }
    assert_eq!(
        from_text.evaluated_nodes, 9,
        "the prototype runs once here too"
    );
}

#[test]
fn recursion_in_a_loaded_document_is_refused_when_it_is_evaluated() {
    let (s, group) = wheels(1);
    let mut doc = s.document().clone();
    // The commands would refuse this; a file written by something else could contain it.
    let version = doc.group(&group).unwrap().version;
    doc.nodes.push(Node {
        id: ve_graph::NodeId::parse("n_zzzzzzzz").unwrap(),
        op: OpId::parse("group.use@1").unwrap(),
        label: None,
        params: std::collections::BTreeMap::new(),
        inputs: std::collections::BTreeMap::new(),
        flags: ve_graph::Flags::default(),
        group: Some(group.clone()),
        instance: Some(Instance {
            group,
            version,
            overrides: std::collections::BTreeMap::new(),
        }),
    });
    let error = Evaluator::default()
        .evaluate(&doc, Quality::Final)
        .unwrap_err();
    assert_eq!(error.code, "group.recursive");
}
