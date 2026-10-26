//! Additional compact-text boundary, round-trip and malformed-program coverage.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture helpers fail loudly"
)]

use proptest::prelude::*;
use ve_graph::{
    CommandBatch, Diagnostic, Document, OpId, Outcome, Session, TransactionId, builtin_registry,
    parse_text, parse_text_in, to_text, to_text_compact,
};

fn build(text: &str) -> Document {
    let mut session = Session::with_builtin(5);
    let batch = parse_text(session.registry(), text).unwrap();
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_edges").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    session.document().clone()
}

fn error(text: &str) -> Diagnostic {
    parse_text(&builtin_registry(), text).unwrap_err().remove(0)
}

fn roundtrip(text: &str) -> String {
    let registry = builtin_registry();
    let doc = build(text);
    let compact = to_text_compact(&registry, &doc).unwrap();
    assert_eq!(
        to_text(&registry, &build(&compact)).unwrap(),
        to_text(&registry, &doc).unwrap(),
        "{compact}"
    );
    compact
}

#[test]
fn lists_triples_local_constants_and_outputs_expand_in_order() {
    let compact = roundtrip(
        "let h = 2\nfor [x, y, z] in [[-1, 2, 3], [4, 5, 6]]\n let w = h / 2\n n_{x} = solid.box(size=[{w}, {y}, {z}])\n output o_{x} = n_{x}\nend\na = solid.box(size=[{h}, 1, 1])",
    );
    assert!(compact.contains("n_m1"));
    assert!(compact.contains("output o_4 = n_4"));
    let d = build("for i in [0.5, 1.5]\n p_{round(i)} = solid.box(size=[{i}, 1, 1])\nend");
    assert_eq!(d.nodes.len(), 2);
}

#[test]
fn empty_and_descending_ranges_emit_nothing_and_bounds_read_constants() {
    let d = build(
        "for i in 2..0\n p_{i} = solid.box(size=[1, 1, 1])\nend\nfor j in []\n p_{j} = solid.box(size=[1, 1, 1])\nend\nlet a = -2\nlet b = 0\nfor k in a..b\n p_{k} = solid.box(size=[1, 1, 1])\nend",
    );
    assert_eq!(d.nodes.len(), 2);
    assert_eq!(d.nodes[0].label.as_deref(), Some("p_m2"));
}

#[test]
fn three_levels_work_and_four_levels_are_refused_even_in_an_empty_loop() {
    let body = "for i in 0..1\nfor j in 0..1\nfor k in 0..1\np_{i}_{j}_{k} = solid.box(size=[1, 1, 1])\nend\nend\nend";
    assert_eq!(build(body).nodes.len(), 1);
    assert_eq!(
        error(&format!("for unused in []\n{body}\nend")).code,
        "parse.syntax"
    );
}

#[test]
fn bad_loop_shapes_and_fractional_names_are_diagnostics() {
    for header in [
        "for i in 0.5..2",
        "for [a,b] in [[1]]",
        "for [a,a] in [[1,2]]",
        "for pi in [1]",
        "for i in [[1,2]]",
    ] {
        assert!(
            parse_text(
                &builtin_registry(),
                &format!("{header}\np = solid.box(size=[1,1,1])\nend")
            )
            .is_err()
        );
    }
    assert_eq!(
        error("for i in [0.5]\np_{i} = solid.box(size=[1,1,1])\nend").code,
        "parse.expression"
    );
}

#[test]
fn later_errors_keep_the_original_line_and_nested_errors_name_every_binding() {
    let e = error("for i in 0..4\np_{i} = solid.box(size=[1,1,1])\nend\nbad = missing.op()");
    assert_eq!(e.span.unwrap().line, 4);
    assert!(!e.message.contains("iteration"));
    let e = error("for i in [2]\nfor [x,z] in [[3,4]]\np = missing.op()\nend\nend");
    assert_eq!(e.span.unwrap().line, 3);
    for binding in ["i=2", "x=3", "z=4"] {
        assert!(e.message.contains(binding), "{e:?}");
    }
    let e = error("for i in [1]\np = solid.box(size=[{1 / 0},1,1])\nend");
    assert_eq!(e.span.unwrap().line, 2);
    assert!(e.message.contains("in iteration i=1"));
    let e = error("for i in [1]\np = solid.box(size=[1,1,1]) $\nend");
    assert_eq!(e.span.unwrap().line, 2);
    assert!(e.message.contains("i=1"));
}

#[test]
fn application_diagnostics_keep_the_iteration_and_line_too() {
    let registry = builtin_registry();
    let batch = parse_text(
        &registry,
        "for i in [2]\np_{i} = solid.box(size=[1,1,1]) | xform.move(by=[0,1,0])\nend",
    )
    .unwrap();
    let located = batch.locate(Diagnostic::error("test", "later failure").at_command(0));
    assert_eq!(located.span.unwrap().line, 2);
    assert!(located.message.contains("i=2"));
}

#[test]
fn expansion_limits_include_pipe_stages_and_allow_exactly_two_thousand() {
    let registry = builtin_registry();
    assert!(
        parse_text(
            &registry,
            "for i in 0..2000\np_{i} = solid.box(size=[1,1,1])\nend"
        )
        .is_ok()
    );
    assert_eq!(
        error("for i in 0..1001\np_{i} = solid.box(size=[1,1,1]) | xform.move(by=[0,1,0])\nend")
            .code,
        "parse.too_big"
    );
    assert!(parse_text(&registry, "base = solid.box(size=[1,1,1])\nfor i in 0..1999\np_{i} = base | xform.move(by=[0,1,0])\nend").is_ok());
}

#[test]
fn loops_preserve_the_existing_f64_expression_semantics() {
    let registry = builtin_registry();
    let from_loop = build("for i in [1]\np = solid.box(size=[{0.1 + i * 0.05},1,1])\nend");
    let from_expression = build("p = solid.box(size=[{0.1 + 1 * 0.05},1,1])");
    assert_eq!(
        to_text(&registry, &from_loop).unwrap(),
        to_text(&registry, &from_expression).unwrap()
    );
}

#[test]
fn loop_keywords_accept_whitespace_and_remain_usable_as_node_names() {
    let doc = build(
        "for\ti\tin\t[1]\ngroup = solid.box(size=[1,1,1])\nfor = xform.move(group, by=[0,1,0])\nresult = xform.move(for, by=[0,1,0])\nend",
    );
    assert_eq!(doc.nodes.len(), 3);
    assert_eq!(
        error("for i in [1]\ngroup\tg\nend\nend").code,
        "parse.syntax"
    );
}

#[test]
fn generated_names_clash_with_later_nodes_and_existing_document_labels() {
    let e = error(
        "p = solid.box(size=[1,1,1]) | xform.move(by=[0,1,0])\np_1 = solid.box(size=[1,1,1])",
    );
    assert_eq!(e.code, "parse.name_taken");
    let registry = builtin_registry();
    let existing = build("p_1 = solid.box(size=[1,1,1])");
    let e = parse_text_in(
        &registry,
        &existing,
        "p = solid.box(size=[1,1,1]) | xform.move(by=[0,1,0])",
    )
    .unwrap_err();
    assert_eq!(e[0].code, "parse.name_taken");
}

#[test]
fn fractional_expressions_in_multiline_values_and_length_units_remain_numbers() {
    roundtrip("let w = 0.5\np = solid.box(size=[\n{w},\n{w}mm,\n1])");
}

#[test]
fn exact_degrees_fallback_radians_defaults_and_flags_roundtrip() {
    let compact = roundtrip(
        "p_1 = solid.box(size=[1,1,1])\np = xform.rotate(p_1, angles=[-2deg, 0deg, 0.123456789rad], muted=true)\noutput p",
    );
    assert!(compact.contains("-2deg"), "{compact}");
    assert!(compact.contains("0.123456789rad"), "{compact}");
    assert!(compact.contains("muted=true"));
    let compact = roundtrip("p = solid.sphere(radius=1, segments=24)");
    assert!(!compact.contains("segments="), "{compact}");
}

#[test]
fn older_versions_are_kept_when_a_newer_operator_is_registered() {
    let mut registry = builtin_registry();
    let mut newer = registry
        .get(&OpId::parse("solid.box@1").unwrap())
        .unwrap()
        .clone();
    newer.id = OpId::parse("solid.box@2").unwrap();
    registry.register(newer).unwrap();
    let doc = build("p = solid.box@1(size=[1,1,1])");
    assert!(
        to_text_compact(&registry, &doc)
            .unwrap()
            .contains("solid.box@1")
    );
}

#[test]
fn group_exposes_and_results_protect_stage_labels_and_instances_roundtrip() {
    let compact = roundtrip(
        "group g\np_1 = solid.box(size=[1,1,1])\np = xform.move(p_1, by=[0,1,0])\nexpose size = p_1.size\nresult p\nend\na = g(size=[2,2,2])\noutput a",
    );
    assert!(!compact.contains(" | "), "{compact}");
    let compact = roundtrip(
        "group g\np_1 = solid.box(size=[1,1,1])\np = xform.move(p_1, by=[0,1,0])\nresult p\nend\na = g()\noutput a",
    );
    assert!(compact.contains(" | "), "{compact}");
}

#[test]
fn outputs_extra_reads_and_nonconsecutive_labels_prevent_folding() {
    let compact = roundtrip(
        "p_1 = solid.box(size=[1,1,1])\np = xform.move(p_1, by=[0,1,0])\noutput first = p_1\noutput p",
    );
    assert!(!compact.contains(" | "));
    let compact = roundtrip("p_1 = solid.box(size=[1,1,1])\np = solid.union(p_1, p_1)");
    assert!(!compact.contains(" | "));
    let compact = roundtrip(
        "p_1 = solid.box(size=[1,1,1])\nx = solid.box(size=[2,2,2])\np = xform.move(p_1, by=[0,1,0])",
    );
    assert!(!compact.contains(" | "));
    let compact = roundtrip("p_2 = solid.box(size=[1,1,1])\np = xform.move(p_2, by=[0,1,0])");
    assert!(!compact.contains(" | "));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 4000, ..ProptestConfig::default() })]
    #[test]
    fn random_loop_and_pipe_programs_never_panic(
        count in 0..8_i32, start in -3..3_i32, edits in proptest::collection::vec((any::<usize>(), any::<u8>()), 0..8),
        pattern in 0..3_u8,
    ) {
        let header = match pattern {
            0 => format!("for i in {start}..{count}"),
            1 => "for i in [-1, 0, 2]".to_owned(),
            _ => "for [i, j] in [[0, 1], [2, 3]]".to_owned(),
        };
        let text = format!("{header}\nfor k in 0..2\np_{{i}}_{{k}} = solid.box(size=[1, 1, {{k + 1}}]) |\n xform.move(by=[{{i}}, 0, 0])\nend\nend");
        let mut bytes = text.into_bytes();
        let alphabet = b"forendin[]{}|._=(),012-+ \n#\"";
        for (index, ch) in edits { let index = index % (bytes.len() + 1); bytes.insert(index, alphabet[usize::from(ch) % alphabet.len()]); }
        let text = String::from_utf8(bytes).unwrap();
        let registry = builtin_registry();
        let _ = parse_text(&registry, &text);
    }
}
