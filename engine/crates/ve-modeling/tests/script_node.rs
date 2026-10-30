//! `script.js@1`: behaviour a model writes, saved in the document as a node so it travels with the world. The evaluation lists the scripts
//! (name, source, enabled, node); the page starts, restarts and stops them to match. The node has no inputs and changes no shape. Written
//! before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(18);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_script").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

const BOX: &str = "body = solid.box@1(size=[1, 1, 1])\noutput body = body\n";

#[test]
fn a_script_node_is_listed_with_its_name_source_and_node() {
    let ev = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log('hi')\", name=\"hello\")\n"
    ))
    .unwrap();
    assert_eq!(ev.scripts.len(), 1, "{:?}", ev.scripts);
    assert_eq!(ev.scripts[0].name, "hello");
    assert_eq!(ev.scripts[0].source, "log('hi')");
    assert!(ev.scripts[0].enabled);
    assert!(!ev.scripts[0].node.is_empty());
}

#[test]
fn the_name_defaults_to_the_nodes_label() {
    let ev = evaluate(&format!("{BOX}bobbing = script.js@1(source=\"log(1)\")\n")).unwrap();
    assert_eq!(ev.scripts[0].name, "bobbing");
}

#[test]
fn a_disabled_script_is_listed_so_the_page_can_stop_it() {
    let ev = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log(1)\", name=\"quiet\", enabled=false)\n"
    ))
    .unwrap();
    assert!(!ev.scripts[0].enabled);
}

#[test]
fn scripts_keep_the_order_they_were_written_in() {
    let ev = evaluate(&format!("{BOX}a = script.js@1(source=\"log('a')\", name=\"first\")\nb = script.js@1(source=\"log('b')\", name=\"second\")\nc = script.js@1(source=\"log('c')\", name=\"third\")\n")).unwrap();
    let names: Vec<_> = ev.scripts.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["first", "second", "third"]);
}

#[test]
fn escapes_in_the_source_become_real_newlines_and_quotes() {
    let ev = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"let a = 1;\\nlog(\\\"two\\\");\", name=\"escapes\")\n"
    ))
    .unwrap();
    assert_eq!(ev.scripts[0].source, "let a = 1;\nlog(\"two\");");
}

#[test]
fn no_script_nodes_means_no_scripts() {
    assert!(evaluate(BOX).unwrap().scripts.is_empty());
}

#[test]
fn the_node_changes_no_shape() {
    let with = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log(1)\", name=\"x\")\n"
    ))
    .unwrap();
    let without = evaluate(BOX).unwrap();
    assert_eq!(with.parts.len(), without.parts.len());
    assert_eq!(with.parts[0].positions, without.parts[0].positions);
}

#[test]
fn bad_names_and_sources_are_refused_with_a_message_that_says_what_to_change() {
    let bad_name = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log(1)\", name=\"Bad Name\")\n"
    ))
    .unwrap_err();
    assert!(
        bad_name.contains("script.name") || bad_name.contains("lowercase"),
        "{bad_name}"
    );
    let leading_digit = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log(1)\", name=\"3d\")\n"
    ))
    .unwrap_err();
    assert!(
        leading_digit.contains("script.name") || leading_digit.contains("lowercase"),
        "{leading_digit}"
    );
    let long_name = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log(1)\", name=\"{}\")\n",
        "a".repeat(33)
    ))
    .unwrap_err();
    assert!(
        long_name.contains("script.name") || long_name.contains("32"),
        "{long_name}"
    );
    let duplicate = evaluate(&format!("{BOX}a = script.js@1(source=\"log(1)\", name=\"same\")\nb = script.js@1(source=\"log(2)\", name=\"same\")\n")).unwrap_err();
    assert!(
        duplicate.contains("same"),
        "a duplicate name is named: {duplicate}"
    );
    assert!(
        evaluate(&format!(
            "{BOX}s = script.js@1(source=\"\", name=\"empty\")\n"
        ))
        .is_err(),
        "an empty source"
    );
    assert!(
        evaluate(&format!(
            "{BOX}s = script.js@1(source=\"   \", name=\"blank\")\n"
        ))
        .is_err(),
        "a blank source"
    );
    let too_long = format!(
        "{BOX}s = script.js@1(source=\"{}\", name=\"big\")\n",
        "x".repeat(65_537)
    );
    assert!(evaluate(&too_long).is_err(), "a source over 65,536 bytes");
}

#[test]
fn a_changed_source_is_not_served_from_the_cache() {
    // Two evaluations in one process: the second must show the new source.
    let first = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log('v1')\", name=\"cached\")\n"
    ))
    .unwrap();
    let second = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log('v2')\", name=\"cached\")\n"
    ))
    .unwrap();
    assert_eq!(first.scripts[0].source, "log('v1')");
    assert_eq!(second.scripts[0].source, "log('v2')");
}

#[test]
fn the_evaluation_json_carries_the_scripts_for_the_page() {
    let ev = evaluate(&format!(
        "{BOX}s = script.js@1(source=\"log(1)\", name=\"hello\", enabled=true)\n"
    ))
    .unwrap();
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["scripts"][0]["name"], "hello");
    assert_eq!(json["scripts"][0]["source"], "log(1)");
    assert_eq!(json["scripts"][0]["enabled"], true);
    assert!(json["scripts"][0]["node"].is_string());
}
