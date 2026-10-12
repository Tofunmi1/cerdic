//! Acceptance cases G1a-01 to G1a-03, 05 to 07 and the alias cases (see `docs/modeling/cases.md`).
//! Cases 04 and 08 to 17 are in `g1a_history.rs`; 18 in `ve-geometry`; 19 and 20 in
//! `g1a_property.rs`.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

#[path = "support/helpers.rs"]
mod helpers;

use helpers::{
    add, add_as, add_box, add_union, alias, batch, commit, input, last_id, link, node, num, reject,
    vec3,
};
use ve_graph::{Command, NodeId, OutRef, Outcome, Session, Value};

fn points(pts: &[[f64; 2]]) -> Value {
    Value::List(
        pts.iter()
            .map(|p| Value::List(vec![num(p[0]), num(p[1])]))
            .collect(),
    )
}

#[test]
fn g1a_01_adding_a_node_issues_an_id_and_fills_defaults() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add("solid.sphere@1", &[("radius", num(1.0))])]);
    let doc = s.document();
    assert_eq!((doc.revision, doc.nodes.len()), (1, 1));
    assert!(NodeId::parse(doc.nodes[0].id.as_str()).is_ok());
    assert_eq!(doc.nodes[0].params.get("segments"), Some(&Value::Int(24)));
}

#[test]
fn g1a_02_commands_and_their_json_form_give_identical_documents() {
    let commands = vec![
        add_box(),
        add_union(),
        add("solid.sphere@1", &[("radius", num(2.0))]),
    ];
    let mut a = Session::with_builtin(5);
    commit(&mut a, commands.clone());
    let mut b = Session::with_builtin(5);
    let json = serde_json::to_string(&batch(&b, commands)).unwrap();
    assert!(matches!(b.apply_json(&json), Outcome::Committed { .. }));
    assert_eq!(a.document().canonical_json(), b.document().canonical_json());
}

#[test]
fn g1a_03_a_failing_command_rejects_the_whole_batch_and_names_its_index() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box()]);
    let ghost = NodeId::parse("n_zzzzzzzz").unwrap();
    let diagnostics = reject(
        &mut s,
        vec![
            add_box(),
            add("solid.sphere@1", &[("radius", num(1.0))]),
            Command::SetFlags {
                node: node(&ghost),
                muted: Some(true),
                bypass: None,
            },
        ],
    );
    assert_eq!(diagnostics[0].code, "graph.unknown_node");
    assert_eq!(diagnostics[0].command, Some(2));
    assert_eq!(s.document().revision, 1);
}

#[test]
fn g1a_05_a_profile_cannot_go_into_a_solid_port() {
    let mut s = Session::with_builtin(1);
    let square = points(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
    commit(
        &mut s,
        vec![add("profile.polygon@1", &[("points", square)])],
    );
    let profile = last_id(&s);
    commit(&mut s, vec![add_union()]);
    let union = last_id(&s);
    let diagnostics = reject(&mut s, vec![link(&profile, &union)]);
    assert_eq!(diagnostics[0].code, "graph.port_type");
    assert_eq!(diagnostics[0].node.as_ref(), Some(&union));
}

#[test]
fn g1a_06_a_value_outside_its_range_is_refused_with_the_range() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add("solid.sphere@1", &[("radius", num(1.0))])]);
    let id = last_id(&s);
    let d = reject(
        &mut s,
        vec![Command::SetParam {
            node: node(&id),
            param: "radius".into(),
            value: num(-1.0),
        }],
    );
    assert_eq!(d[0].code, "param.range");
    assert_eq!(d[0].observed, Some(-1.0));
    assert!(d[0].limit.is_some());
}

#[test]
fn g1a_07_non_finite_numbers_are_refused() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add("solid.sphere@1", &[("radius", num(1.0))])]);
    let id = last_id(&s);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let d = reject(
            &mut s,
            vec![Command::SetParam {
                node: node(&id),
                param: "radius".into(),
                value: num(bad),
            }],
        );
        assert_eq!(d[0].code, "param.non_finite");
    }
}

#[test]
fn g1a_21_aliases_let_one_batch_build_and_link_a_graph() {
    let mut s = Session::with_builtin(1);
    let outer = points(&[[0.0, 0.0], [0.06, 0.0], [0.05, 0.3], [0.0, 0.3]]);
    commit(
        &mut s,
        vec![
            add_as("profile.polygon@1", &[("points", outer)], Some("$outer")),
            add_as("solid.revolve@1", &[], Some("$body")),
            Command::Connect {
                from: OutRef {
                    node: alias("$outer"),
                    port: "out".into(),
                },
                to: input(alias("$body")),
            },
            Command::SetOutput {
                name: "vase".into(),
                from: Some(OutRef {
                    node: alias("$body"),
                    port: "out".into(),
                }),
            },
        ],
    );
    let doc = s.document();
    assert_eq!(doc.nodes.len(), 2);
    assert_eq!(doc.outputs.len(), 1);
    assert_eq!(doc.nodes[1].inputs["in"][0].node, doc.nodes[0].id);
}

#[test]
fn g1a_22_aliases_are_scoped_to_their_batch_and_must_be_defined_first() {
    let mut s = Session::with_builtin(1);
    commit(
        &mut s,
        vec![add_as(
            "solid.box@1",
            &[("size", vec3(1.0, 1.0, 1.0))],
            Some("$a"),
        )],
    );
    // The alias from the previous batch is gone.
    let d = reject(
        &mut s,
        vec![Command::SetLabel {
            node: alias("$a"),
            label: None,
        }],
    );
    assert_eq!(d[0].code, "graph.unknown_node");
    // Using an alias before the command that defines it fails; so does defining it twice.
    let size = [("size", vec3(1.0, 1.0, 1.0))];
    let early = vec![
        Command::SetLabel {
            node: alias("$b"),
            label: None,
        },
        add_as("solid.box@1", &size, Some("$b")),
    ];
    assert_eq!(reject(&mut s, early)[0].code, "graph.unknown_node");
    let twice = vec![
        add_as("solid.box@1", &size, Some("$c")),
        add_as("solid.box@1", &size, Some("$c")),
    ];
    assert_eq!(reject(&mut s, twice)[0].code, "graph.duplicate_id");
}
