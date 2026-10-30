//! ADR 0069: pipes, loops and the compact printer are expansion in the text layer. Each case reads compact text and long-hand text
//! and asserts they build the same document, node for node.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "case helpers unwrap fixtures that are known to be valid and fail loudly otherwise"
)]
use std::collections::BTreeMap;

use ve_graph::{
    CommandBatch, Diagnostic, Document, Outcome, Session, TransactionId, builtin_registry,
    parse_text, to_text,
};

/// What a document is, without ids: each node's label, operator, parameters and the labels it reads, plus the outputs.
fn shape(doc: &Document) -> Vec<String> {
    let labels: BTreeMap<_, _> = doc
        .nodes
        .iter()
        .map(|n| (n.id.clone(), n.label.clone().unwrap_or_default()))
        .collect();
    let mut lines: Vec<String> = doc
        .nodes
        .iter()
        .map(|n| {
            let inputs: Vec<String> = n
                .inputs
                .iter()
                .map(|(port, refs)| {
                    let from: Vec<&str> = refs.iter().map(|r| labels[&r.node].as_str()).collect();
                    format!("{port}<-{}", from.join("+"))
                })
                .collect();
            format!(
                "{} = {} {:?} [{}]",
                n.label.as_deref().unwrap_or(""),
                n.op,
                n.params,
                inputs.join(" ")
            )
        })
        .collect();
    lines.extend(
        doc.outputs
            .iter()
            .map(|o| format!("output {} <- {}", o.name, labels[&o.from.node])),
    );
    lines
}

fn build(text: &str) -> Result<Document, Vec<Diagnostic>> {
    let registry = builtin_registry();
    let batch = parse_text(&registry, text)?;
    let mut session = Session::with_builtin(5);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_text").unwrap(),
        label: None,
        commands: batch.commands.clone(),
    });
    match outcome {
        Outcome::Committed { .. } => Ok(session.document().clone()),
        Outcome::Rejected { diagnostics } => {
            Err(diagnostics.into_iter().map(|d| batch.locate(d)).collect())
        }
        other => panic!("{other:?}"),
    }
}

fn doc(text: &str) -> Document {
    build(text).unwrap_or_else(|e| panic!("{text}\n{e:?}"))
}

fn same(compact: &str, longhand: &str) {
    assert_eq!(shape(&doc(compact)), shape(&doc(longhand)), "{compact}");
}

fn first_error(text: &str) -> Diagnostic {
    build(text).unwrap_err().remove(0)
}

// ---- pipes ----

#[test]
fn a_pipe_names_its_stages_and_feeds_each_into_the_next() {
    same(
        "post = solid.box(size=[0.08, 1.2, 0.08]) | xform.move(by=[0, 0.6, 0]) | xform.array(count_x=3, spacing=[0.75, 0, 0])\noutput p = post",
        "post_1 = solid.box(size=[0.08, 1.2, 0.08])\npost_2 = xform.move(post_1, by=[0, 0.6, 0])\npost = xform.array(post_2, count_x=3, spacing=[0.75, 0, 0])\noutput p = post",
    );
}

#[test]
fn a_pipe_may_start_from_an_existing_node() {
    same(
        "a = solid.box(size=[1, 1, 1])\nb = a | xform.move(by=[0, 1, 0])\noutput o = b",
        "a = solid.box(size=[1, 1, 1])\nb = xform.move(a, by=[0, 1, 0])\noutput o = b",
    );
}

#[test]
fn inputs_written_in_a_later_stage_follow_the_piped_one() {
    same(
        "a = solid.box(size=[1, 1, 1])\nc = solid.box(size=[2, 1, 1]) | solid.union(a)\noutput o = c",
        "a = solid.box(size=[1, 1, 1])\nc_1 = solid.box(size=[2, 1, 1])\nc = solid.union(c_1, a)\noutput o = c",
    );
}

#[test]
fn a_line_ending_in_a_pipe_continues_on_the_next_line() {
    same(
        "p = solid.box(size=[1, 1, 1]) |\n  xform.move(by=[1, 0, 0]) |\n  xform.mirror(axis=\"x\")\noutput o = p",
        "p_1 = solid.box(size=[1, 1, 1])\np_2 = xform.move(p_1, by=[1, 0, 0])\np = xform.mirror(p_2, axis=\"x\")\noutput o = p",
    );
}

#[test]
fn a_stage_name_that_is_taken_is_refused_and_says_which() {
    let e = first_error(
        "post_1 = solid.box(size=[1, 1, 1])\npost = solid.box(size=[1, 1, 1]) | xform.move(by=[0, 1, 0])",
    );
    assert_eq!(e.code, "parse.name_taken");
    assert!(e.message.contains("post_1"), "{}", e.message);
}

#[test]
fn a_pipe_stage_with_nothing_to_read_is_refused() {
    let e = first_error("p = solid.box(size=[1, 1, 1]) | solid.sphere(radius=1)");
    assert_eq!(e.code, "parse.syntax");
    assert!(e.message.contains("input"), "{}", e.message);
}

// ---- loops ----

#[test]
fn a_range_loop_writes_one_statement_per_whole_number() {
    same(
        "for i in 0..3\n  p_{i} = solid.box(size=[0.1, 0.1, {0.25 + i * 0.25}])\nend\noutput o = p_2",
        "p_0 = solid.box(size=[0.1, 0.1, 0.25])\np_1 = solid.box(size=[0.1, 0.1, 0.5])\np_2 = solid.box(size=[0.1, 0.1, 0.75])\noutput o = p_2",
    );
}

#[test]
fn a_list_loop_takes_a_pair_per_item() {
    same(
        "for [x, z] in [[1, 2], [3, 4]]\n  t_{x} = solid.box(size=[0.5, 0.5, 0.5]) | xform.move(by=[{x}, 0, {z}])\nend\noutput o = t_3",
        "t_1_1 = solid.box(size=[0.5, 0.5, 0.5])\nt_1 = xform.move(t_1_1, by=[1, 0, 2])\nt_3_1 = solid.box(size=[0.5, 0.5, 0.5])\nt_3 = xform.move(t_3_1, by=[3, 0, 4])\noutput o = t_3",
    );
}

#[test]
fn loops_nest_and_see_both_variables() {
    same(
        "for i in 0..2\n  for j in 0..2\n    c_{i}_{j} = solid.box(size=[0.1, 0.1, 0.1]) | xform.move(by=[{i}, 0, {j}])\n  end\nend\noutput o = c_1_1",
        "c_0_0_1 = solid.box(size=[0.1, 0.1, 0.1])\nc_0_0 = xform.move(c_0_0_1, by=[0, 0, 0])\nc_0_1_1 = solid.box(size=[0.1, 0.1, 0.1])\nc_0_1 = xform.move(c_0_1_1, by=[0, 0, 1])\nc_1_0_1 = solid.box(size=[0.1, 0.1, 0.1])\nc_1_0 = xform.move(c_1_0_1, by=[1, 0, 0])\nc_1_1_1 = solid.box(size=[0.1, 0.1, 0.1])\nc_1_1 = xform.move(c_1_1_1, by=[1, 0, 1])\noutput o = c_1_1",
    );
}

#[test]
fn a_negative_number_in_a_name_reads_as_m_and_the_number() {
    same(
        "for i in -1..1\n  n_{i} = solid.box(size=[1, 1, 1])\nend\noutput o = n_0",
        "n_m1 = solid.box(size=[1, 1, 1])\nn_0 = solid.box(size=[1, 1, 1])\noutput o = n_0",
    );
}

#[test]
fn a_loop_reads_names_defined_before_it_and_constants() {
    same(
        "let h = 2\nbase = solid.box(size=[1, 1, 1])\nfor i in 0..2\n  s_{i} = xform.move(base, by=[{i}, {h}, 0])\nend\noutput o = s_1",
        "base = solid.box(size=[1, 1, 1])\ns_0 = xform.move(base, by=[0, 2, 0])\ns_1 = xform.move(base, by=[1, 2, 0])\noutput o = s_1",
    );
}

#[test]
fn a_loop_that_expands_past_the_limit_is_refused_with_the_way_out() {
    let e = first_error("for i in 0..3000\n  p_{i} = solid.box(size=[1, 1, 1])\nend");
    assert_eq!(e.code, "parse.too_big");
    let hint = e.suggestion.clone().unwrap_or_default();
    assert!(hint.contains("inst.array"), "{hint}");
}

#[test]
fn an_error_in_a_loop_body_names_the_iteration_and_the_body_line() {
    let e = first_error("for i in 0..3\n  p_{i} = solid.box(size=[{i - 1}, 1, 1])\nend");
    assert!(e.message.contains("i=0"), "{}", e.message);
    assert_eq!(e.span.map(|s| s.line), Some(2));
}

#[test]
fn a_group_inside_a_loop_is_refused() {
    let e = first_error(
        "for i in 0..2\n  group g\n    a = solid.box(size=[1, 1, 1])\n    result a\n  end\nend",
    );
    assert_eq!(e.code, "parse.syntax");
}

#[test]
fn a_loop_without_end_is_refused() {
    let e = first_error("for i in 0..2\n  p_{i} = solid.box(size=[1, 1, 1])");
    assert_eq!(e.code, "parse.syntax");
    assert!(e.message.contains("end"), "{}", e.message);
}

// ---- the compact printer ----

const FENCE: &str = "\
post_1 = solid.box@1(size=[0.08, 1.2, 0.08])
post_2 = xform.move@1(post_1, by=[0, 0.6, 0])
post = xform.array@1(post_2, count_x=3, spacing=[0.75, 0, 0])
lean_1 = solid.box@1(size=[0.1, 1, 0.1])
lean = xform.rotate@1(lean_1, angles=[2deg, 0deg, 10deg])
joined = solid.union@1(post, lean)
output fence = joined
";

#[test]
fn the_compact_text_reads_back_to_the_same_document() {
    let registry = builtin_registry();
    let original = doc(FENCE);
    let compact = ve_graph::to_text_compact(&registry, &original).unwrap();
    assert_eq!(shape(&doc(&compact)), shape(&original), "{compact}");
}

#[test]
fn the_compact_text_is_shorter_and_has_no_versions_or_noisy_radians() {
    let registry = builtin_registry();
    let original = doc(FENCE);
    let canonical = to_text(&registry, &original).unwrap();
    let compact = ve_graph::to_text_compact(&registry, &original).unwrap();
    assert!(!compact.contains('@'), "{compact}");
    assert!(!compact.contains("rad"), "{compact}");
    assert!(
        compact.contains("2deg") && compact.contains("10deg"),
        "{compact}"
    );
    assert!(
        compact.len() * 10 <= canonical.len() * 8,
        "compact {} bytes against canonical {}\n{compact}",
        compact.len(),
        canonical.len()
    );
}

#[test]
fn a_chain_of_single_use_nodes_prints_as_a_pipe() {
    let registry = builtin_registry();
    let compact = ve_graph::to_text_compact(&registry, &doc(FENCE)).unwrap();
    assert!(compact.contains(" | "), "{compact}");
    assert!(compact.contains("post = solid.box"), "{compact}");
}

#[test]
fn a_node_that_is_not_named_like_a_stage_keeps_its_own_line() {
    let registry = builtin_registry();
    let original =
        doc("a = solid.box(size=[1, 1, 1])\nb = xform.move(a, by=[0, 1, 0])\noutput o = b");
    let compact = ve_graph::to_text_compact(&registry, &original).unwrap();
    assert!(!compact.contains(" | "), "{compact}");
    assert_eq!(shape(&doc(&compact)), shape(&original), "{compact}");
}

#[test]
fn a_shared_node_is_never_piped_away() {
    let registry = builtin_registry();
    let original = doc(
        "s_1 = solid.box(size=[1, 1, 1])\ns = xform.move(s_1, by=[0, 1, 0])\nt = xform.move(s_1, by=[0, 2, 0])\noutput a = s\noutput b = t",
    );
    let compact = ve_graph::to_text_compact(&registry, &original).unwrap();
    assert_eq!(shape(&doc(&compact)), shape(&original), "{compact}");
}

#[test]
fn the_canonical_text_is_the_same_whichever_way_the_document_was_written() {
    let registry = builtin_registry();
    let long = doc(
        "post_1 = solid.box(size=[1, 1, 1])\npost = xform.move(post_1, by=[0, 1, 0])\noutput o = post",
    );
    let piped = doc("post = solid.box(size=[1, 1, 1]) | xform.move(by=[0, 1, 0])\noutput o = post");
    assert_eq!(
        to_text(&registry, &long).unwrap(),
        to_text(&registry, &piped).unwrap()
    );
}
