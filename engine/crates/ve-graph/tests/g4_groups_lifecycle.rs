//! G4b: groups and instances at the document level, part 2: upgrades, removal, undo, persistence,
//! group inputs, nesting and limits (cases G4b-12 to G4b-20, `docs/modeling/cases.md`).
#[path = "support/groups.rs"]
mod support;

use serde_json::json;
use support::{add_instances, batch, commit, named, overrides, reject, wheel};
use ve_graph::{Document, GroupId, Outcome, Session, to_text, upgrade_preview};

#[test]
fn g4b_12_an_upgrade_moves_one_instance_and_nothing_else() {
    let (mut s, id) = wheel();
    add_instances(&mut s, &id, &["a", "b"]);
    let (a, b) = (named(&s, "a"), named(&s, "b"));
    commit(
        &mut s,
        json!([
            {"set_instance_param": {"node": a.as_str(), "param": "radius", "value": 0.4}},
            {"set_instance_param": {"node": a.as_str(), "param": "width", "value": 0.3}}
        ]),
    );
    commit(
        &mut s,
        json!([{"unexpose_param": {"group": id.as_str(), "name": "width"}}]),
    );
    let d = reject(
        &mut s,
        json!([{"set_instance_param": {"node": a.as_str(), "param": "radius", "value": 0.5}}]),
    );
    assert_eq!(
        d[0].code, "group.outdated",
        "an instance that is behind must be upgraded first"
    );
    let preview = upgrade_preview(s.document(), s.registry(), &a).unwrap();
    assert_eq!(preview.dropped_overrides, ["width"]);
    assert!(preview.dropped_links.is_empty());
    let before: Document = s.document().clone();
    commit(&mut s, json!([{"upgrade_group": {"node": a.as_str()}}]));
    let after = s.document();
    let moved = after.node(&a).unwrap().instance.as_ref().unwrap();
    assert_eq!(moved.version, 4);
    assert_eq!(
        moved.overrides.keys().collect::<Vec<_>>(),
        ["radius"],
        "the matching override stays"
    );
    assert_eq!(
        after.group(&id),
        before.group(&id),
        "the group is unchanged"
    );
    assert_eq!(
        after.node(&b),
        before.node(&b),
        "the other instance is untouched"
    );
    let others = |d: &Document| {
        d.nodes
            .iter()
            .filter(|n| n.id != a)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(others(after), others(&before), "no other node changed");
    let d = reject(&mut s, json!([{"upgrade_group": {"node": a.as_str()}}]));
    assert_eq!(d[0].code, "group.up_to_date");
}

#[test]
fn g4b_13_the_interface_cannot_be_removed_from_under_a_group() {
    let (mut s, id) = wheel();
    for label in ["tyre", "wheel"] {
        let node = named(&s, label);
        let d = reject(
            &mut s,
            json!([{"remove_node": {"node": node.as_str(), "cascade": true}}]),
        );
        assert_eq!(
            d[0].code, "group.in_use",
            "{label} is part of the interface"
        );
    }
    commit(
        &mut s,
        json!([{"unexpose_param": {"group": id.as_str(), "name": "radius"}}]),
    );
    let hub = named(&s, "hub");
    let d = reject(
        &mut s,
        json!([{"remove_node": {"node": hub.as_str(), "cascade": true}}]),
    );
    assert_eq!(d[0].code, "group.in_use", "the hub still holds 'width'");
}

#[test]
fn g4b_14_a_group_with_instances_cannot_be_removed() {
    let (mut s, id) = wheel();
    add_instances(&mut s, &id, &["a", "b"]);
    let d = reject(&mut s, json!([{"remove_group": {"group": id.as_str()}}]));
    assert_eq!(d[0].code, "group.in_use");
    assert_eq!(d[0].related.len(), 2, "it names the instances");
    let nodes = s.document().nodes.len();
    let (a, b) = (named(&s, "a"), named(&s, "b"));
    commit(
        &mut s,
        json!([
            {"remove_node": {"node": a.as_str()}},
            {"remove_node": {"node": b.as_str()}},
            {"remove_group": {"group": id.as_str()}}
        ]),
    );
    assert!(s.document().groups.is_empty());
    assert_eq!(
        s.document().nodes.len(),
        nodes - 2 - 5,
        "the members went with the group"
    );
}

#[test]
fn g4b_15_undo_and_redo_restore_groups_exactly() {
    let empty = Session::with_builtin(4).document().clone();
    let (mut s, id) = wheel();
    let with_group = s.document().clone();
    add_instances(&mut s, &id, &["a"]);
    let a = named(&s, "a");
    commit(
        &mut s,
        json!([{"set_instance_param": {"node": a.as_str(), "param": "radius", "value": 0.4}}]),
    );
    let full = s.document().clone();
    s.undo().unwrap();
    assert!(overrides(&s, "a").is_empty());
    s.undo().unwrap();
    assert!(s.document().same_content(&with_group));
    s.undo().unwrap();
    assert!(
        s.document().same_content(&empty),
        "group, members, versions and aliases all undone"
    );
    for _ in 0..3 {
        s.redo().unwrap();
    }
    assert!(s.document().same_content(&full));
}

#[test]
fn g4b_16_groups_survive_a_save_and_old_documents_still_load() {
    let (mut s, id) = wheel();
    add_instances(&mut s, &id, &["a"]);
    let a = named(&s, "a");
    commit(
        &mut s,
        json!([{"set_instance_param": {"node": a.as_str(), "param": "radius", "value": 0.4}}]),
    );
    let json = s.document().canonical_json();
    let back: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, s.document());
    assert_eq!(back.canonical_json(), json, "the form is canonical");
    let plain = Document::new(1).canonical_json();
    assert!(
        !plain.contains("groups"),
        "a document without groups looks as it always did"
    );
    assert_eq!(
        serde_json::from_str::<Document>(&plain).unwrap(),
        Document::new(1)
    );
}

/// A bracket group: one box and a `mount` input, joined by a union that is the result.
fn bracket() -> (Session, GroupId) {
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
            {"set_group_output": {"group": "$g", "node": "$u"}}
        ]),
    );
    let id = s.document().groups[0].id.clone();
    (s, id)
}

#[test]
fn g4b_17_an_instance_links_to_the_inputs_of_its_group_by_name() {
    let (mut s, id) = bracket();
    assert_eq!(
        s.document().group(&id).unwrap().version,
        3,
        "an input and a new result changed the interface twice"
    );
    commit(
        &mut s,
        json!([
            {"add_instance": {"group": id.as_str(), "label": "bracket", "alias": "$i"}},
            {"add_node": {"op": "solid.box@1", "params": {"size": [1, 1, 1]}, "label": "post", "alias": "$p"}},
            {"connect": {"from": {"node": "$p", "port": "out"}, "to": {"node": "$i", "port": "mount"}}}
        ]),
    );
    let instance = named(&s, "bracket");
    let post = named(&s, "post");
    assert_eq!(
        s.document().node(&instance).unwrap().inputs["mount"][0].node,
        post
    );
    let d = reject(
        &mut s,
        json!([{"connect": {"from": {"node": post.as_str(), "port": "out"}, "to": {"node": instance.as_str(), "port": "nope"}}}]),
    );
    assert_eq!(d[0].code, "graph.unknown_port");
    assert!(d[0].suggestion.as_deref().unwrap().contains("mount"));
    commit(
        &mut s,
        json!([{"add_node": {"op": "profile.polygon@1", "params": {"points": [[0, 0], [1, 0], [0, 1]]}, "label": "flat", "alias": "$f"}}]),
    );
    let flat = named(&s, "flat");
    let d = reject(
        &mut s,
        json!([{"connect": {"from": {"node": flat.as_str(), "port": "out"}, "to": {"node": instance.as_str(), "port": "mount"}}}]),
    );
    assert_eq!(d[0].code, "graph.port_type");
}

#[test]
fn g4b_13b_removing_a_group_input_is_an_interface_change() {
    let (mut s, id) = bracket();
    let (input, joined) = (named(&s, "mount"), named(&s, "joined"));
    let d = reject(&mut s, json!([{"remove_node": {"node": input.as_str()}}]));
    assert_eq!(d[0].code, "graph.in_use", "the union still reads it");
    commit(
        &mut s,
        json!([
            {"disconnect": {"to": {"node": joined.as_str(), "port": "in", "index": 0}}},
            {"remove_node": {"node": input.as_str()}}
        ]),
    );
    assert_eq!(s.document().group(&id).unwrap().version, 4);
}

#[test]
fn g4b_18_groups_nest_and_a_group_of_groups_can_be_instanced() {
    let (mut s, wheel_id) = wheel();
    commit(
        &mut s,
        json!([
            {"add_instance": {"group": wheel_id.as_str(), "label": "left", "alias": "$l"}},
            {"add_instance": {"group": wheel_id.as_str(), "label": "right", "alias": "$r"}},
            {"add_node": {"op": "solid.union@1", "label": "axle", "alias": "$u"}},
            {"connect": {"from": {"node": "$l", "port": "out"}, "to": {"node": "$u", "port": "in"}}},
            {"connect": {"from": {"node": "$r", "port": "out"}, "to": {"node": "$u", "port": "in"}}},
            {"create_group": {"name": "axle", "members": ["$l", "$r", "$u"], "output": "$u", "alias": "$g"}},
            {"add_instance": {"group": "$g", "label": "front"}},
            {"add_instance": {"group": "$g", "label": "back"}}
        ]),
    );
    assert_eq!(s.document().groups.len(), 2);
    let axle = s
        .document()
        .groups
        .iter()
        .find(|g| g.name == "axle")
        .unwrap()
        .id
        .clone();
    assert_eq!(s.document().instances_of(&axle).count(), 2);
    assert_eq!(
        s.document().instances_of(&wheel_id).count(),
        2,
        "the two inside the axle group"
    );
}

#[test]
fn g4b_19_nesting_is_bounded() {
    let mut s = Session::with_builtin(4);
    commit(
        &mut s,
        json!([
            {"add_node": {"op": "solid.box@1", "params": {"size": [1, 1, 1]}, "alias": "$b"}},
            {"create_group": {"name": "g0", "members": ["$b"], "output": "$b"}}
        ]),
    );
    let mut deepest = 0;
    for level in 1..=12 {
        let previous = s.document().groups.last().unwrap().id.clone();
        commit(
            &mut s,
            json!([{"add_instance": {"group": previous.as_str(), "label": format!("i{level}"), "alias": "$i"}}]),
        );
        let instance = named(&s, &format!("i{level}"));
        let created = json!([{"create_group": {"name": format!("g{level}"), "members": [instance.as_str()], "output": instance.as_str()}}]);
        let outcome = s.apply(&batch(&s, created));
        if let Outcome::Rejected { diagnostics } = outcome {
            assert_eq!(diagnostics[0].code, "budget.group_depth");
            assert_eq!(diagnostics[0].limit, Some(8.0));
            assert_eq!(diagnostics[0].observed, Some(9.0));
            break;
        }
        deepest = level;
    }
    assert_eq!(deepest, 7, "g7 is eight levels deep, the most allowed");
}

#[test]
fn g4b_20_the_text_form_refuses_groups_rather_than_misprint_them() {
    let (s, _) = wheel();
    let error = to_text(s.registry(), s.document()).unwrap_err();
    assert_eq!(error.code, "text.unsupported");
    let plain = Session::with_builtin(4);
    assert!(to_text(plain.registry(), plain.document()).is_ok());
}
