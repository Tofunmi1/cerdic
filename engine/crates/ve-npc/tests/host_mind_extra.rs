#![expect(clippy::expect_used, reason = "tests assert by failing")]
//! Additional repair, filler, reflection, persistence, and command tests.

use serde_json::{Value, json};
use ve_npc::{
    FallbackTable, FillerSpec, MemoryKind, MindBudget, NpcHost, NpcSpec, Reason, Response,
};

fn roomy() -> MindBudget {
    MindBudget {
        calls_per_minute: 100,
        max_concurrent: 100,
        tokens_per_minute: 1_000_000,
        timeout_secs: 8.0,
    }
}
fn host(names: &[&str]) -> NpcHost {
    let mut h = NpcHost::new(roomy());
    for name in names {
        h.register(NpcSpec {
            name: (*name).into(),
            persona: format!("You are {name}."),
            rules: "Be kind.".into(),
            goals: vec![],
            world_facts: vec![],
        })
        .expect("register");
    }
    h
}
fn reply(text: &str) -> String {
    json!({"intents":[{"name":"say","args":{"text":text}}]}).to_string()
}

#[test]
fn refused_repair_falls_back_and_retry_counts_are_per_npc() {
    let mut h = host(&["a", "b"]);
    h.set_fallback(
        "a",
        FallbackTable::new().line(Reason::PlayerSpoke, "fallback"),
    );
    h.player_says("a", "first", 0.0).expect("ask");
    let a = h.take_outbox().remove(0);
    h.deliver(
        Response {
            id: a.id,
            outcome: Ok("bad".into()),
        },
        0.1,
    );
    h.player_says("b", "hello", 0.2).expect("ask");
    let b = h
        .take_outbox()
        .into_iter()
        .find(|r| r.npc == "b")
        .expect("b request");
    h.deliver(
        Response {
            id: b.id,
            outcome: Ok("bad".into()),
        },
        0.3,
    );
    assert_eq!(h.retries_used("a"), 1);
    assert_eq!(h.retries_used("b"), 1);
    let retry = h
        .take_outbox()
        .into_iter()
        .find(|r| r.npc == "b")
        .expect("b retry");
    h.deliver(
        Response {
            id: retry.id,
            outcome: Ok(reply("ok")),
        },
        0.4,
    );
    assert_eq!(h.retries_used("b"), 1);
}

#[test]
fn rate_limited_repair_uses_fallback_immediately() {
    let mut h = NpcHost::new(MindBudget {
        calls_per_minute: 1,
        ..roomy()
    });
    h.register(NpcSpec {
        name: "a".into(),
        persona: "A".into(),
        rules: String::new(),
        goals: vec![],
        world_facts: vec![],
    })
    .expect("register");
    h.set_fallback(
        "a",
        FallbackTable::new().line(Reason::PlayerSpoke, "at once"),
    );
    h.player_says("a", "hello", 0.0).expect("ask");
    let r = h.take_outbox().remove(0);
    let result = h.deliver(
        Response {
            id: r.id,
            outcome: Ok("invalid".into()),
        },
        0.1,
    );
    assert!(result.is_some_and(|d| d.fallback));
    assert!(h.take_outbox().is_empty());
    assert_eq!(
        h.session("a")
            .expect("session")
            .transcript()
            .last()
            .expect("line")
            .text,
        "at once"
    );
}

#[test]
fn retry_correction_appears_once_and_three_fillers_rotate() {
    let mut h = host(&["a"]);
    h.set_filler(
        "a",
        FillerSpec {
            after_secs: 0.0,
            lines: vec!["one".into(), "two".into(), "three".into()],
        },
    );
    let mut seen = Vec::new();
    for i in 0..3 {
        h.player_says("a", "question", f64::from(i * 10))
            .expect("ask");
        let first = h.take_outbox().remove(0);
        h.deliver(
            Response {
                id: first.id,
                outcome: Ok("bad".into()),
            },
            f64::from(i * 10) + 0.1,
        );
        let retry = h.take_outbox().remove(0);
        assert_eq!(retry.prompt.matches("## Correction").count(), 1);
        h.tick(f64::from(i * 10) + 1.0);
        seen.extend(
            h.drain_events("a")
                .expect("events")
                .into_iter()
                .filter_map(|e| {
                    if let ve_npc::SessionEvent::Filler { text } = e {
                        Some(text)
                    } else {
                        None
                    }
                }),
        );
        h.deliver(
            Response {
                id: retry.id,
                outcome: Ok(reply("fine")),
            },
            f64::from(i * 10) + 2.0,
        );
    }
    assert_eq!(seen, vec!["one", "two", "three"]);
    let mut other = host(&["a"]);
    other.player_says("a", "hi", 0.0).expect("ask");
    other.take_outbox();
    other.tick(5.0);
    assert!(
        !other
            .drain_events("a")
            .expect("events")
            .iter()
            .any(|e| matches!(e, ve_npc::SessionEvent::Filler { .. }))
    );
}

#[test]
fn normal_reflect_intent_is_pinned_without_touching_pending_reflection() {
    let mut h = host(&["a"]);
    for (s, v) in [("x", 0.8), ("y", 0.4), ("z", 0.7)] {
        h.observe("a", s, v, 1.0).expect("observe");
    }
    h.maybe_reflect("a", 2.0).expect("reflect");
    let reflection = h.take_outbox().remove(0);
    h.player_says("a", "hi", 3.0).expect("player");
    let normal = h.take_outbox().remove(0);
    let body=json!({"intents":[{"name":"say","args":{"text":"hello"}},{"name":"reflect","args":{"text":"The player is helpful."}}]}).to_string();
    h.deliver(
        Response {
            id: normal.id,
            outcome: Ok(body),
        },
        4.0,
    );
    assert!(
        h.memory("a")
            .expect("memory")
            .recent(20)
            .iter()
            .any(|m| m.kind == MemoryKind::Reflection && m.pinned)
    );
    assert!(h.maybe_reflect("a", 5.0).is_ok());
    h.deliver(
        Response {
            id: reflection.id,
            outcome: Ok("{}".into()),
        },
        6.0,
    );
}

#[test]
fn reflection_prompt_limits_recent_non_reflections_and_auto_reflects() {
    let mut h = host(&["a"]);
    for i in 0..20 {
        h.observe("a", &format!("memory {i}"), 0.1, f64::from(i))
            .expect("observe");
    }
    h.observe("a", "enough", 1.0, 30.0).expect("observe");
    assert!(h.maybe_reflect("a", 31.0).expect("ask"));
    let p = h.take_outbox().remove(0).prompt;
    assert!(!p.contains("say("));
    assert!(p.contains("memory 19"));
    assert!(!p.contains("memory 0"));
    let mut a = host(&["a"]);
    a.set_reflection_threshold(0.5);
    a.observe("a", "something", 0.6, 1.0).expect("observe");
    a.set_auto_reflect(true);
    a.tick(2.0);
    assert_eq!(a.take_outbox().len(), 1);
}

#[test]
fn new_state_roundtrips_and_new_commands_never_panic_under_seeded_inputs() {
    let mut h = host(&["a"]);
    h.set_filler(
        "a",
        FillerSpec {
            after_secs: 2.0,
            lines: vec!["hmm".into()],
        },
    );
    h.set_reflection_threshold(0.4);
    h.set_auto_reflect(true);
    h.player_says("a", "hi", 0.0).expect("ask");
    let request = h.take_outbox().remove(0);
    h.deliver(
        Response {
            id: request.id,
            outcome: Ok("bad".into()),
        },
        1.0,
    );
    let json = h.to_json();
    let restored = NpcHost::from_json(&json, roomy()).expect("load");
    assert_eq!(restored.to_json(), json);
    for seed in 0..500_u64 {
        let input = match seed % 3 {
            0 => json!({"cmd":"set_filler","npc":"a","after_secs":1.0,"lines":[seed.to_string()]}),
            1 => json!({"cmd":"maybe_reflect","npc":"a","now":seed}),
            _ => json!({"cmd":"set_reflection_threshold","value":0.5}),
        };
        let out: Value =
            serde_json::from_str(&h.handle(&input.to_string())).expect("json response");
        assert!(out.is_object());
    }
}
