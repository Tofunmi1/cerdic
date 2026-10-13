//! Acceptance cases G1b-01 to G1b-09: saving, loading, migration and failure handling
//! (see `docs/modeling/cases.md`). Cases 10 to 12 (the page bridge) are in `web/tests/document.mjs`.
#![expect(
    clippy::unwrap_used,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

use std::collections::BTreeMap;

use ve_graph::{
    Alias, Command, CommandBatch, DocStore, Document, MemoryBackend, MigrationStep, Migrator,
    NodeRef, OutRef, Outcome, Session, TransactionId, Value, builtin_registry,
};

const FIXTURE: &str = include_str!("fixtures/schema0_vase.json");

fn batch(s: &Session, tag: &str, commands: Vec<Command>) -> CommandBatch {
    CommandBatch {
        doc: s.document().id.clone(),
        base_revision: s.document().revision,
        transaction: TransactionId::parse(&format!("t_{tag}{}", s.document().revision)).unwrap(),
        label: None,
        commands,
    }
}

fn points(n: usize) -> Value {
    let pt = |i: usize| {
        let x = f64::from(u32::try_from(i % 1000).unwrap()) * 1e-3;
        Value::List(vec![Value::Num(x), Value::Num(x * 0.5 + 0.1)])
    };
    Value::List((0..n).map(pt).collect())
}

fn polygon(n: usize) -> Command {
    Command::AddNode {
        op: "profile.polygon@1".into(),
        params: [("points".to_owned(), points(n))].into(),
        label: None,
        alias: None,
        group: None,
    }
}

/// A small document with links, an output, a label and an editor position.
fn sample() -> Session {
    let mut s = Session::with_builtin(7);
    let alias = |a: &str| NodeRef::Alias(Alias::parse(a).unwrap());
    let b = batch(
        &s,
        "sample",
        vec![
            Command::AddNode {
                op: "profile.polygon@1".into(),
                params: [("points".to_owned(), points(5))].into(),
                label: Some("outline".into()),
                alias: Some(Alias::parse("$p").unwrap()),
                group: None,
            },
            Command::AddNode {
                op: "solid.revolve@1".into(),
                params: BTreeMap::new(),
                label: None,
                alias: Some(Alias::parse("$r").unwrap()),
                group: None,
            },
            Command::Connect {
                from: OutRef {
                    node: alias("$p"),
                    port: "out".into(),
                },
                to: ve_graph::InputRef {
                    node: alias("$r"),
                    port: "in".into(),
                    index: None,
                },
            },
            Command::SetOutput {
                name: "body".into(),
                from: Some(OutRef {
                    node: alias("$r"),
                    port: "out".into(),
                }),
            },
            Command::EditorSetPosition {
                node: alias("$p"),
                x: 12.0,
                y: -3.0,
            },
        ],
    );
    assert!(matches!(s.apply(&b), Outcome::Committed { .. }));
    s
}

fn store() -> DocStore<MemoryBackend> {
    DocStore::new(MemoryBackend::new(), "case")
}

fn load(
    store: &DocStore<MemoryBackend>,
) -> Result<Option<ve_graph::Loaded>, Box<ve_graph::Diagnostic>> {
    store.load(&builtin_registry(), &Migrator::builtin())
}

#[test]
fn g1b_01_save_then_load_gives_the_same_document() {
    let s = sample();
    let mut st = store();
    assert_eq!(st.save(s.document()).unwrap(), 1);
    let loaded = load(&st).unwrap().unwrap();
    assert_eq!(
        &loaded.document,
        s.document(),
        "IDs, layout, revision and the ID counter all survive"
    );
    assert_eq!(
        (loaded.generation, loaded.read_only, loaded.migrated_from),
        (1, false, None)
    );
    assert!(loaded.issues.is_empty());
    assert!(
        load(&store()).unwrap().is_none(),
        "nothing saved yet is not an error"
    );
}

#[test]
fn g1b_02_a_torn_write_cannot_damage_the_newest_save() {
    let first = sample();
    let mut second = sample();
    let b = batch(&second, "more", vec![polygon(4)]);
    assert!(matches!(second.apply(&b), Outcome::Committed { .. }));
    // Writes 0 and 1 are fine; write 2 is cut in half but reports success.
    let mut st = DocStore::new(MemoryBackend::new().with_torn_write_after(2), "case");
    st.save(first.document()).unwrap();
    st.save(second.document()).unwrap();
    let before: Vec<_> = ["a", "b"]
        .iter()
        .map(|s| st.backend().raw(&format!("ve.doc.case.{s}")).cloned())
        .collect();
    st.save(first.document()).unwrap(); // the torn one: lands in the older slot
    let loaded = load(&st).unwrap().unwrap();
    assert_eq!(
        &loaded.document,
        second.document(),
        "the newest intact save wins"
    );
    assert_eq!(loaded.generation, 2);
    assert_eq!(loaded.issues.len(), 1, "the damaged copy is reported");
    assert_eq!(loaded.issues[0].code, "store.corrupt");
    // Loading never writes.
    let after: Vec<_> = ["a", "b"]
        .iter()
        .map(|s| st.backend().raw(&format!("ve.doc.case.{s}")).cloned())
        .collect();
    assert_ne!(before, after, "the torn save changed one slot");
    let again = load(&st).unwrap().unwrap();
    assert_eq!(again.generation, 2);
}

#[test]
fn g1b_03_damaged_files_are_reported_and_the_previous_save_is_kept() {
    let mut st = store();
    let mut s = sample();
    st.save(s.document()).unwrap();
    let older = s.document().clone();
    let b = batch(&s, "edit", vec![polygon(4)]);
    assert!(matches!(s.apply(&b), Outcome::Committed { .. }));
    st.save(s.document()).unwrap();
    let newest = st.backend().raw("ve.doc.case.b").unwrap().clone();
    st.backend_mut()
        .put_raw("ve.doc.case.b", &newest[..newest.len() / 2]);
    let loaded = load(&st).unwrap().unwrap();
    assert_eq!(loaded.document, older, "the older intact slot is loaded");
    assert_eq!(loaded.issues[0].code, "store.corrupt");
    // Both damaged: an error, with the damage named.
    let slot_a = st.backend().raw("ve.doc.case.a").unwrap().clone();
    st.backend_mut()
        .put_raw("ve.doc.case.a", &slot_a[..slot_a.len() / 3]);
    assert_eq!(load(&st).unwrap_err().code, "store.corrupt");
    // A flipped byte inside the document is caught by the checksum.
    let mut st2 = store();
    st2.save(s.document()).unwrap();
    let text = st2
        .backend()
        .raw("ve.doc.case.a")
        .unwrap()
        .replace("outline", "outlinX");
    st2.backend_mut().put_raw("ve.doc.case.a", &text);
    assert_eq!(load(&st2).unwrap_err().code, "store.corrupt");
}

#[test]
fn g1b_04_a_file_from_a_newer_version_is_refused_and_never_overwritten() {
    let mut st = store();
    st.save(sample().document()).unwrap();
    let text =
        st.backend()
            .raw("ve.doc.case.a")
            .unwrap()
            .replacen("\"schema\":1,", "\"schema\":99,", 1);
    assert!(text.contains("\"schema\":99,"));
    st.backend_mut().put_raw("ve.doc.case.a", &text);
    assert_eq!(load(&st).unwrap_err().code, "store.newer_version");
    // A newer build may checksum differently: still recognised as newer, never as damage.
    let other_sum = text.replacen("\"checksum\":\"", "\"checksum\":\"ffff", 1);
    st.backend_mut().put_raw("ve.doc.case.a", &other_sum);
    assert_eq!(load(&st).unwrap_err().code, "store.newer_version");
    assert_eq!(
        st.save(sample().document()).unwrap_err().code,
        "store.newer_version"
    );
    st.backend_mut().put_raw("ve.doc.case.a", &text);
    let doc = sample();
    assert_eq!(
        st.save(doc.document()).unwrap_err().code,
        "store.newer_version"
    );
    assert_eq!(
        st.backend().raw("ve.doc.case.a"),
        Some(&text),
        "the file is byte for byte unchanged"
    );
    assert!(
        st.backend().raw("ve.doc.case.b").is_none(),
        "nothing else was written either"
    );
}

#[test]
fn g1b_05_unknown_operators_keep_the_file_and_the_other_nodes_viewable() {
    use ve_graph::{Arity, OpId, OperatorSpec, OutputSpec, PortSpec, PortType, Readiness};
    let mut registry = builtin_registry();
    registry
        .register(OperatorSpec {
            id: OpId::parse("solid.future@1").unwrap(),
            doc: "an operator from a newer build",
            inputs: vec![PortSpec {
                name: "in",
                accepts: &[PortType::Solid],
                arity: Arity::One,
            }],
            output: OutputSpec::Fixed(PortType::Solid),
            params: vec![],
            readiness: Readiness::Draft,
        })
        .unwrap();
    let mut s = Session::new(registry, Document::new(3));
    let add = |op: &str| Command::AddNode {
        op: op.into(),
        params: BTreeMap::new(),
        label: None,
        alias: None,
        group: None,
    };
    let b = batch(
        &s,
        "future",
        vec![add("solid.future@1"), add("solid.union@1")],
    );
    assert!(matches!(s.apply(&b), Outcome::Committed { .. }));
    let mut st = store();
    st.save(s.document()).unwrap();
    let raw = st.backend().raw("ve.doc.case.a").cloned();

    let loaded = load(&st).unwrap().unwrap(); // this build does not know solid.future
    assert!(loaded.read_only);
    assert_eq!(
        loaded.document.nodes.len(),
        2,
        "every node is still there to view"
    );
    let issue = &loaded.issues[0];
    assert_eq!(issue.code, "store.unknown_op");
    assert_eq!(issue.related, vec![loaded.document.nodes[0].id.clone()]);
    assert_eq!(
        st.backend().raw("ve.doc.case.a").cloned(),
        raw,
        "the file is kept"
    );
}

#[test]
fn g1b_06_an_old_format_file_migrates_and_stays_editable() {
    let mut st = store();
    st.backend_mut().put_raw("ve.doc.case.a", FIXTURE);
    let loaded = load(&st).unwrap().unwrap();
    assert_eq!(loaded.migrated_from, Some(0));
    assert!(loaded.issues.is_empty() && !loaded.read_only);
    let doc = &loaded.document;
    assert_eq!((doc.schema, doc.revision, doc.nodes.len()), (1, 12, 5));
    let revolve = &doc.nodes[2];
    assert_eq!(revolve.inputs["in"].len(), 1, "a single link became a list");
    assert_eq!(doc.nodes[4].inputs["in"].len(), 2);
    assert_eq!(doc.outputs[0].name, "vase");
    assert_eq!(
        doc.editor.positions.len(),
        2,
        "positions survive; frames are gone"
    );

    // The migrated document is a normal one: editing works and issues IDs that do not collide.
    let mut s = Session::new(builtin_registry(), loaded.document.clone());
    let b = batch(
        &s,
        "after",
        vec![Command::AddNode {
            op: "solid.box@1".into(),
            params: [("size".to_owned(), Value::List(vec![Value::Num(1.0); 3]))].into(),
            label: None,
            alias: None,
            group: None,
        }],
    );
    assert!(matches!(s.apply(&b), Outcome::Committed { .. }));
    let ids: std::collections::BTreeSet<_> =
        s.document().nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(ids.len(), 6);
    let mut out = store();
    out.save(s.document()).unwrap();
    assert_eq!(
        &load(&out).unwrap().unwrap().document,
        s.document(),
        "and it saves in the current format"
    );
}

#[test]
fn g1b_07_a_failing_migration_step_keeps_the_file() {
    let mut st = store();
    st.backend_mut().put_raw("ve.doc.case.a", FIXTURE);
    let failing = Migrator::with_steps(vec![MigrationStep {
        from: 0,
        run: |_| Err("cannot read the old links".into()),
    }]);
    let err = st.load(&builtin_registry(), &failing).unwrap_err();
    assert_eq!(err.code, "store.migration_failed");
    assert!(err.message.contains("cannot read the old links"));
    assert_eq!(
        st.backend().raw("ve.doc.case.a").map(String::as_str),
        Some(FIXTURE)
    );
    assert_eq!(
        st.load(&builtin_registry(), &Migrator::with_steps(Vec::new()))
            .unwrap_err()
            .code,
        "store.migration_failed"
    );
}

#[test]
fn g1b_08_a_full_store_refuses_the_save_and_keeps_the_previous_one() {
    let mut st = DocStore::new(MemoryBackend::new().with_quota(6_000), "case");
    let mut s = sample();
    st.save(s.document()).unwrap();
    let first = s.document().clone();
    let b = batch(&s, "big", vec![polygon(400)]);
    assert!(matches!(s.apply(&b), Outcome::Committed { .. }));
    let err = st.save(s.document()).unwrap_err();
    assert_eq!(err.code, "store.quota");
    assert!(
        err.limit.unwrap() > 6_000.0,
        "the message says how many bytes were needed"
    );
    let loaded = load(&st).unwrap().unwrap();
    assert_eq!(loaded.document, first, "the previous save is intact");
    assert_eq!(loaded.generation, 1);
}

#[test]
fn g1b_09_a_document_up_to_the_size_limit_saves_and_one_past_it_is_refused() {
    let mut s = Session::with_builtin(1);
    let mut last_ok = None;
    let refusal = loop {
        let b = batch(&s, "fill", vec![polygon(4096)]);
        match s.apply(&b) {
            Outcome::Committed { .. } => last_ok = Some(s.document().clone()),
            Outcome::Rejected { diagnostics } => break diagnostics,
            other => panic!("{other:?}"),
        }
        assert!(
            s.document().nodes.len() < 100,
            "the limit was never reached"
        );
    };
    assert_eq!(refusal[0].code, "budget.source");
    assert_eq!(refusal[0].limit, Some(2_097_152.0));
    let doc = last_ok.unwrap();
    assert!(doc.canonical_json().len() <= 2_097_152);
    assert!(
        doc.canonical_json().len() > 1_900_000,
        "the accepted document is close to the limit"
    );
    let mut st = store();
    st.save(&doc).unwrap();
    assert_eq!(load(&st).unwrap().unwrap().document, doc);
}
