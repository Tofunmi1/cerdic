//! Contract tests for the mind scheduler (ADR 0064, step M2b): what a game asks a language model, in what order, within which budgets, what
//! happens when the answer is late, wrong or never comes, and how a whole session is recorded and replayed without the model.
#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    reason = "tests assert by failing and use small loop counters as seconds"
)]

use ve_npc::{
    Ask, FallbackTable, Mind, MindBudget, Reason, Recording, Refusal, Request, Response,
    ScriptedProvider, Vocabulary,
};

const HELLO: &str = r#"{"intents":[{"name":"say","args":{"text":"Hello, traveller."}}]}"#;

fn mind(budget: MindBudget) -> Mind {
    Mind::new(Vocabulary::with_builtins(), budget)
}

fn roomy() -> MindBudget {
    MindBudget {
        calls_per_minute: 100,
        max_concurrent: 100,
        tokens_per_minute: 1_000_000,
        timeout_secs: 8.0,
    }
}

fn id_of(ask: &Ask) -> ve_npc::RequestId {
    match ask {
        Ask::Sent(id) | Ask::Queued(id) => *id,
        Ask::Refused(why) => panic!("refused: {why:?}"),
    }
}

fn say_text(delivery: &ve_npc::Delivery) -> String {
    match delivery
        .intents
        .first()
        .expect("an intent")
        .args
        .get("text")
        .expect("text")
    {
        ve_npc::ArgValue::Text(t) => t.clone(),
        other => panic!("not text: {other:?}"),
    }
}

#[test]
fn a_request_leaves_through_the_outbox_with_a_priority_from_its_reason() {
    let mut m = mind(roomy());
    let id = id_of(&m.ask("anna", Reason::PlayerSpoke, "prompt one", 0.0));
    let out = m.take_outbox();
    assert_eq!(out.len(), 1);
    assert_eq!(
        (out[0].id, out[0].npc.as_str(), out[0].prompt.as_str()),
        (id, "anna", "prompt one")
    );
    assert_eq!(out[0].priority, 100);
    assert!(m.take_outbox().is_empty(), "taken means taken");
    let priorities: Vec<u8> = [
        Reason::PlayerSpoke,
        Reason::Event,
        Reason::BrainRequest,
        Reason::Reflection,
        Reason::Idle,
    ]
    .iter()
    .map(|r| {
        let mut m = mind(roomy());
        m.ask("x", *r, "p", 0.0);
        m.take_outbox()[0].priority
    })
    .collect();
    assert_eq!(priorities, vec![100, 60, 50, 40, 10]);
}

#[test]
fn one_npc_cannot_ask_more_than_its_calls_per_minute() {
    let mut m = mind(MindBudget {
        calls_per_minute: 2,
        ..roomy()
    });
    assert!(matches!(
        m.ask("anna", Reason::Idle, "a", 0.0),
        Ask::Sent(_)
    ));
    assert!(matches!(
        m.ask("anna", Reason::Idle, "b", 10.0),
        Ask::Sent(_)
    ));
    assert_eq!(
        m.ask("anna", Reason::Idle, "c", 20.0),
        Ask::Refused(Refusal::RateLimited)
    );
    assert!(
        matches!(m.ask("ben", Reason::Idle, "d", 20.0), Ask::Sent(_)),
        "another NPC has its own allowance"
    );
    assert!(
        matches!(m.ask("anna", Reason::Idle, "e", 61.0), Ask::Sent(_)),
        "the window slides past the first call"
    );
}

#[test]
fn too_many_in_flight_wait_their_turn_highest_priority_first() {
    let mut m = mind(MindBudget {
        max_concurrent: 1,
        ..roomy()
    });
    let a = id_of(&m.ask("a", Reason::Event, "a", 0.0));
    assert!(matches!(m.ask("b", Reason::Idle, "b", 0.0), Ask::Queued(_)));
    let d = id_of(&m.ask("d", Reason::PlayerSpoke, "d", 0.0));
    let out = m.take_outbox();
    assert_eq!(
        out.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![a],
        "only one slot"
    );
    assert_eq!(m.pending(), 3);
    let reply = Response {
        id: a,
        outcome: Ok(HELLO.to_owned()),
    };
    assert!(m.deliver(reply, 1.0).is_some());
    let out = m.take_outbox();
    assert_eq!(
        out.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![d],
        "the player's NPC jumps the idle one"
    );
}

#[test]
fn a_prompt_that_would_blow_the_token_budget_is_refused_until_the_minute_passes() {
    let mut m = mind(MindBudget {
        tokens_per_minute: 1_500,
        ..roomy()
    });
    let long = "x".repeat(4_000);
    assert!(
        matches!(m.ask("a", Reason::Idle, &long, 0.0), Ask::Sent(_)),
        "about 1,000 tokens fits"
    );
    assert_eq!(
        m.ask("b", Reason::Idle, &long, 5.0),
        Ask::Refused(Refusal::OverTokenBudget)
    );
    assert!(matches!(
        m.ask("b", Reason::Idle, &long, 61.0),
        Ask::Sent(_)
    ));
}

#[test]
fn a_good_reply_is_delivered_as_validated_intents() {
    let mut m = mind(roomy());
    let id = id_of(&m.ask("anna", Reason::PlayerSpoke, "hi", 0.0));
    let delivery = m
        .deliver(
            Response {
                id,
                outcome: Ok(HELLO.to_owned()),
            },
            0.5,
        )
        .expect("known request");
    assert_eq!((delivery.npc.as_str(), delivery.fallback), ("anna", false));
    assert_eq!(say_text(&delivery), "Hello, traveller.");
    assert!(delivery.rejected.is_empty());
    assert!(
        m.deliver(
            Response {
                id,
                outcome: Ok(HELLO.to_owned())
            },
            0.6
        )
        .is_none(),
        "an answer is only taken once"
    );
}

#[test]
fn a_provider_error_or_nonsense_falls_back_to_a_canned_line() {
    let mut m = mind(roomy());
    m.set_fallback(
        "anna",
        FallbackTable::new()
            .line(Reason::PlayerSpoke, "Hm? Say that again.")
            .line(Reason::Idle, "Lovely weather."),
    );
    let a = id_of(&m.ask("anna", Reason::PlayerSpoke, "hi", 0.0));
    let failed = m
        .deliver(
            Response {
                id: a,
                outcome: Err("503 service unavailable".to_owned()),
            },
            1.0,
        )
        .expect("known");
    assert!(failed.fallback);
    assert_eq!(say_text(&failed), "Hm? Say that again.");
    let b = id_of(&m.ask("anna", Reason::Idle, "hi", 2.0));
    let junk = m
        .deliver(
            Response {
                id: b,
                outcome: Ok("I am just a language model.".to_owned()),
            },
            3.0,
        )
        .expect("known");
    assert!(junk.fallback, "no valid intent survived");
    assert_eq!(say_text(&junk), "Lovely weather.");
    assert!(
        !junk.rejected.is_empty(),
        "what was wrong is still reported"
    );
    let c = id_of(&m.ask("ben", Reason::PlayerSpoke, "hi", 4.0));
    let silent = m
        .deliver(
            Response {
                id: c,
                outcome: Err("down".to_owned()),
            },
            5.0,
        )
        .expect("known");
    assert!(
        silent.fallback && silent.intents.is_empty(),
        "no table, no line, no intents"
    );
}

#[test]
fn a_late_answer_is_replaced_by_the_fallback_and_then_ignored() {
    let mut m = mind(MindBudget {
        timeout_secs: 8.0,
        ..roomy()
    });
    m.set_fallback(
        "anna",
        FallbackTable::new().line(Reason::PlayerSpoke, "One moment..."),
    );
    let id = id_of(&m.ask("anna", Reason::PlayerSpoke, "hi", 0.0));
    assert!(m.tick(7.9).is_empty());
    let expired = m.tick(8.1);
    assert_eq!(expired.len(), 1);
    assert!(expired[0].fallback);
    assert_eq!(say_text(&expired[0]), "One moment...");
    assert!(
        m.deliver(
            Response {
                id,
                outcome: Ok(HELLO.to_owned())
            },
            9.0
        )
        .is_none(),
        "too late"
    );
    assert_eq!(m.pending(), 0);
}

#[test]
fn fallback_lines_are_chosen_the_same_way_every_run() {
    let table = FallbackTable::new()
        .line(Reason::Idle, "one")
        .line(Reason::Idle, "two")
        .line(Reason::Idle, "three");
    let run = || {
        let mut m = mind(roomy());
        m.set_fallback("anna", table.clone());
        (0..6)
            .map(|i| {
                let id = id_of(&m.ask("anna", Reason::Idle, "p", f64::from(i) * 100.0));
                say_text(
                    &m.deliver(
                        Response {
                            id,
                            outcome: Err("x".to_owned()),
                        },
                        0.0,
                    )
                    .expect("known"),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
    let lines = run();
    assert!(
        lines
            .iter()
            .all(|l| ["one", "two", "three"].contains(&l.as_str()))
    );
    assert!(
        lines.iter().any(|l| l != &lines[0]),
        "it does not just repeat one line"
    );
}

#[test]
fn cancelling_an_npc_drops_its_queued_and_in_flight_requests() {
    let mut m = mind(MindBudget {
        max_concurrent: 1,
        ..roomy()
    });
    let a = id_of(&m.ask("anna", Reason::Event, "a", 0.0));
    m.ask("anna", Reason::Idle, "b", 0.0);
    let c = id_of(&m.ask("ben", Reason::Idle, "c", 0.0));
    m.cancel_npc("anna");
    assert_eq!(m.pending(), 1);
    assert!(
        m.deliver(
            Response {
                id: a,
                outcome: Ok(HELLO.to_owned())
            },
            1.0
        )
        .is_none()
    );
    let out = m.take_outbox();
    assert_eq!(out.iter().map(|r| r.id).collect::<Vec<_>>(), vec![c]);
}

fn session(provider: &mut ScriptedProvider, m: &mut Mind) -> Vec<String> {
    let mut seen = Vec::new();
    for (i, (npc, prompt)) in [
        ("anna", "the player says hello"),
        ("ben", "a wolf howls"),
        ("anna", "the player buys bread"),
    ]
    .iter()
    .enumerate()
    {
        m.ask(npc, Reason::PlayerSpoke, prompt, i as f64);
        for request in m.take_outbox() {
            let response = provider.respond(&request);
            let d = m.deliver(response, i as f64 + 0.5).expect("known");
            seen.push(format!(
                "{} {:?} {:?} {}",
                d.npc, d.intents, d.rejected, d.fallback
            ));
        }
    }
    seen
}

#[test]
fn a_scripted_provider_answers_by_what_the_prompt_contains_and_errors_otherwise() {
    let mut p = ScriptedProvider::new().reply_when("hello", HELLO);
    let mut m = mind(roomy());
    m.ask("anna", Reason::Idle, "the player says hello", 0.0);
    let request: Request = m.take_outbox().remove(0);
    assert_eq!(p.respond(&request).outcome, Ok(HELLO.to_owned()));
    m.ask("anna", Reason::Idle, "something else entirely", 1.0);
    let other = m.take_outbox().remove(0);
    assert!(p.respond(&other).outcome.is_err());
}

#[test]
fn a_recorded_session_replays_identically_without_the_original_provider() {
    let mut original = ScriptedProvider::new()
        .reply_when("hello", HELLO)
        .reply_when("wolf", r#"{"intents":[{"name":"say","args":{"text":"Wolves!"}},{"name":"emote","args":{"name":"afraid"}}]}"#)
        .reply_when("bread", "Sorry, I cannot help with that.");
    let mut first = mind(roomy());
    first.start_recording();
    let seen_first = session(&mut original, &mut first);
    assert_eq!(seen_first.len(), 3);
    let json = first.recording().to_json();
    let mut replay = ScriptedProvider::from_recording(&Recording::from_json(&json).expect("loads"));
    let mut second = mind(roomy());
    let seen_second = session(&mut replay, &mut second);
    assert_eq!(
        seen_first, seen_second,
        "the same session, from the recording alone"
    );
    assert!(Recording::from_json("{nope").is_err());
}
