//! Additional scheduler boundary and deterministic stress coverage.
#![expect(clippy::panic, reason = "tests assert by failing")]

use ve_npc::{Ask, Mind, MindBudget, Reason, Recording, Response, ScriptedProvider, Vocabulary};

fn fresh(budget: MindBudget) -> Mind {
    Mind::new(Vocabulary::with_builtins(), budget)
}
fn id(ask: Ask) -> ve_npc::RequestId {
    match ask {
        Ask::Sent(id) | Ask::Queued(id) => id,
        Ask::Refused(r) => panic!("refused {r:?}"),
    }
}
const GOOD: &str = r#"{"intents":[{"name":"say","args":{"text":"ok"}}]}"#;

#[test]
fn minute_edge_multibyte_empty_recording_and_foreign_id() {
    let budget = MindBudget {
        calls_per_minute: 1,
        max_concurrent: 8,
        tokens_per_minute: 2,
        timeout_secs: 8.0,
    };
    let mut m = fresh(budget);
    let first = id(m.ask("a", Reason::Idle, "éééééééé", 0.0));
    assert_eq!(
        m.ask("a", Reason::Idle, "x", 60.0),
        Ask::Refused(ve_npc::Refusal::RateLimited)
    );
    assert_eq!(
        m.ask("b", Reason::Idle, "éééééééééééé", 61.0),
        Ask::Refused(ve_npc::Refusal::OverTokenBudget)
    );
    let empty = Recording::from_json(&Recording::default().to_json()).expect("empty round trip");
    assert!(empty.to_json().contains("entries"));
    let mut other = fresh(MindBudget::default());
    assert!(
        other
            .deliver(
                Response {
                    id: first,
                    outcome: Ok(GOOD.into())
                },
                1.0
            )
            .is_none()
    );
}

#[test]
fn recording_replay_consumes_identical_prompts_in_order() {
    let mut m = fresh(MindBudget::default());
    m.start_recording();
    let mut ids = Vec::new();
    for _ in 0..2 {
        ids.push(id(m.ask("a", Reason::Idle, "same", 0.0)));
    }
    for request in m.take_outbox() {
        m.deliver(
            Response {
                id: request.id,
                outcome: Ok(GOOD.into()),
            },
            1.0,
        )
        .expect("delivery");
    }
    let mut p = ScriptedProvider::from_recording(&m.recording());
    for request_id in ids {
        let request = ve_npc::Request {
            id: request_id,
            npc: "a".into(),
            prompt: "same".into(),
            reason: Reason::Idle,
            priority: 10,
            issued_at: 0.0,
        };
        assert_eq!(p.respond(&request).outcome, Ok(GOOD.into()));
    }
}

#[test]
fn seeded_scheduler_stress_preserves_uniqueness_and_slots() {
    let budget = MindBudget {
        calls_per_minute: 1000,
        max_concurrent: 7,
        tokens_per_minute: 1_000_000,
        timeout_secs: 9.0,
    };
    let mut m = fresh(budget);
    let mut seed = 7u64;
    let mut out_ids = std::collections::BTreeSet::new();
    let mut done = std::collections::BTreeSet::new();
    for step in 0..3000 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let n = (seed >> 32) % 20;
        let npc = format!("n{n}");
        match seed % 11 {
            0 => {
                m.cancel_npc(&npc);
            }
            1 => {
                for d in m.tick(f64::from(step) / 3.0) {
                    assert!(done.insert(d.request));
                }
            }
            _ => {
                let _ = m.ask(
                    &npc,
                    [Reason::Idle, Reason::Event, Reason::PlayerSpoke]
                        [usize::try_from(seed % 3).unwrap_or(0)],
                    "prompt",
                    f64::from(step) / 3.0,
                );
            }
        }
        let batch = m.take_outbox();
        assert!(batch.len() <= 7);
        for request in batch {
            assert!(out_ids.insert(request.id));
            assert!(done.insert(request.id));
            let _ = m.deliver(
                Response {
                    id: request.id,
                    outcome: Ok(GOOD.into()),
                },
                f64::from(step) / 3.0,
            );
        }
        assert!(m.pending() <= 3000);
    }
}
