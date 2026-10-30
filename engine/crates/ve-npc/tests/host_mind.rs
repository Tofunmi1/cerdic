//! Contract tests for making the NPC's mind sturdier (ADR 0064, step M5b), from the research and the first real-model run: **one repair retry**
//! when a reply has no valid intent (a weaker model's usual failure), a **filler line** when the model is slow (the voice-agent trick of
//! covering latency with an immediate acknowledgement), **reflection** (when enough has happened the NPC is asked for lasting conclusions, which
//! become pinned memories), and **guidance in the prompt** about when to use the memory intents.
#![expect(clippy::expect_used, reason = "tests assert by failing")]

use serde_json::json;
use ve_npc::{
    FallbackTable, FillerSpec, MemoryKind, MindBudget, NpcHost, NpcSpec, Reason, Response,
    SessionEvent,
};

fn host() -> NpcHost {
    let mut h = NpcHost::new(MindBudget::default());
    h.register(NpcSpec {
        name: "anna".into(),
        persona: "You are Anna, the baker.".into(),
        rules: "Stay in character.".into(),
        goals: vec![],
        world_facts: vec![],
    })
    .expect("registers");
    h
}

fn say(text: &str) -> String {
    json!({"intents": [{"name": "say", "args": {"text": text}}]}).to_string()
}

fn last(h: &NpcHost) -> (String, bool) {
    let t = h
        .session("anna")
        .expect("session")
        .transcript()
        .last()
        .expect("turn")
        .clone();
    (t.text, t.fallback)
}

// ---- repair retry ----

#[test]
fn a_reply_with_no_valid_intent_is_asked_for_again_once_before_giving_up() {
    let mut h = host();
    h.player_says("anna", "Hello", 1.0).expect("asks");
    let first = h.take_outbox().remove(0);
    let delivered = h.deliver(
        Response {
            id: first.id,
            outcome: Ok("Sure! Here is my answer: hello.".into()),
        },
        2.0,
    );
    assert!(
        delivered.is_none() || delivered.as_ref().is_some_and(|d| d.intents.is_empty()),
        "nothing is shown yet"
    );
    assert_eq!(
        h.session("anna").expect("session").transcript().len(),
        1,
        "no reply turn yet, only the player's line"
    );
    let again = h.take_outbox();
    assert_eq!(again.len(), 1, "one repair request went out");
    assert!(
        again[0].prompt.contains("not valid") && again[0].prompt.contains("ONE JSON object"),
        "{}",
        again[0].prompt
    );
    assert!(
        again[0].prompt.contains("Hello"),
        "the repair keeps the original prompt"
    );
    h.deliver(
        Response {
            id: again[0].id,
            outcome: Ok(say("Hello, friend!")),
        },
        3.0,
    )
    .expect("known");
    assert_eq!(
        last(&h),
        ("Hello, friend!".to_owned(), false),
        "the retry's answer is shown as a normal reply"
    );
    assert_eq!(h.retries_used("anna"), 1);
}

#[test]
fn a_second_bad_reply_falls_back_and_a_provider_error_is_not_retried() {
    let mut h = host();
    h.set_fallback(
        "anna",
        FallbackTable::new().line(Reason::PlayerSpoke, "Hm? Say that again."),
    );
    h.player_says("anna", "Hello", 1.0).expect("asks");
    let first = h.take_outbox().remove(0);
    h.deliver(
        Response {
            id: first.id,
            outcome: Ok("nonsense".into()),
        },
        2.0,
    );
    let second = h.take_outbox().remove(0);
    h.deliver(
        Response {
            id: second.id,
            outcome: Ok("more nonsense".into()),
        },
        3.0,
    )
    .expect("known");
    assert_eq!(last(&h), ("Hm? Say that again.".to_owned(), true));
    assert!(h.take_outbox().is_empty(), "never a third attempt");
    h.player_says("anna", "Again", 10.0).expect("asks");
    let third = h.take_outbox().remove(0);
    h.deliver(
        Response {
            id: third.id,
            outcome: Err("503".into()),
        },
        11.0,
    )
    .expect("known");
    assert!(
        h.take_outbox().is_empty(),
        "a provider error goes straight to the fallback: no doubled waiting"
    );
    assert_eq!(last(&h), ("Hm? Say that again.".to_owned(), true));
}

// ---- filler ----

#[test]
fn a_slow_model_gets_one_filler_line_after_the_delay_and_it_is_not_remembered() {
    let mut h = host();
    h.set_filler(
        "anna",
        FillerSpec {
            after_secs: 1.2,
            lines: vec!["Hmm...".into(), "Let me see...".into()],
        },
    );
    h.player_says("anna", "What is the price of rye?", 0.0)
        .expect("asks");
    let request = h.take_outbox().remove(0);
    h.drain_events("anna").expect("npc");
    h.tick(1.0);
    assert!(
        !h.drain_events("anna")
            .expect("npc")
            .iter()
            .any(|e| matches!(e, SessionEvent::Filler { .. })),
        "too early"
    );
    h.tick(1.3);
    let events = h.drain_events("anna").expect("npc");
    let fillers: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let SessionEvent::Filler { text } = e {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(fillers.len(), 1);
    assert!(["Hmm...", "Let me see..."].contains(&fillers[0].as_str()));
    h.tick(3.0);
    assert!(
        !h.drain_events("anna")
            .expect("npc")
            .iter()
            .any(|e| matches!(e, SessionEvent::Filler { .. })),
        "once per request"
    );
    h.deliver(
        Response {
            id: request.id,
            outcome: Ok(say("Two coins.")),
        },
        4.0,
    )
    .expect("known");
    assert_eq!(
        h.session("anna").expect("session").transcript().len(),
        2,
        "the filler is not a turn"
    );
    assert!(
        !h.memory("anna")
            .expect("memory")
            .recent(20)
            .iter()
            .any(|m| m.text.contains("Hmm...")),
        "and not a memory"
    );
}

#[test]
fn a_quick_answer_gets_no_filler() {
    let mut h = host();
    h.set_filler(
        "anna",
        FillerSpec {
            after_secs: 1.2,
            lines: vec!["Hmm...".into()],
        },
    );
    h.player_says("anna", "Hi", 0.0).expect("asks");
    let request = h.take_outbox().remove(0);
    h.deliver(
        Response {
            id: request.id,
            outcome: Ok(say("Hello!")),
        },
        0.5,
    )
    .expect("known");
    h.drain_events("anna").expect("npc");
    h.tick(2.0);
    assert!(
        !h.drain_events("anna")
            .expect("npc")
            .iter()
            .any(|e| matches!(e, SessionEvent::Filler { .. }))
    );
}

// ---- reflection ----

fn busy(h: &mut NpcHost) {
    for (text, importance) in [
        ("The player gave me a gold coin.", 0.8),
        ("The player asked about the mill.", 0.4),
        ("The player helped carry flour.", 0.7),
    ] {
        h.observe("anna", text, importance, 100.0)
            .expect("remembers");
    }
}

#[test]
fn enough_has_happened_so_the_npc_is_asked_for_lasting_conclusions() {
    let mut h = host();
    assert!(
        !h.maybe_reflect("anna", 200.0).expect("npc"),
        "nothing has happened yet"
    );
    busy(&mut h);
    assert!(
        h.maybe_reflect("anna", 200.0).expect("npc"),
        "importance 1.9 passes the default threshold"
    );
    let out = h.take_outbox();
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].reason, out[0].priority), (Reason::Reflection, 40));
    for part in ["You are Anna", "gold coin", "carry flour", "reflect("] {
        assert!(
            out[0].prompt.contains(part),
            "{part} missing:\n{}",
            out[0].prompt
        );
    }
    assert!(
        !h.maybe_reflect("anna", 201.0).expect("npc"),
        "one is already out"
    );
    assert!(h.take_outbox().is_empty());
}

#[test]
fn a_reflection_becomes_a_pinned_memory_and_resets_the_clock() {
    let mut h = host();
    busy(&mut h);
    h.maybe_reflect("anna", 200.0).expect("npc");
    let request = h.take_outbox().remove(0);
    let reply = json!({"intents": [{"name": "reflect", "args": {"text": "The player is generous and helpful; I can trust them."}}]}).to_string();
    h.deliver(
        Response {
            id: request.id,
            outcome: Ok(reply),
        },
        205.0,
    )
    .expect("known");
    let m = h.memory("anna").expect("memory");
    let r = m
        .recent(20)
        .into_iter()
        .find(|x| x.kind == MemoryKind::Reflection)
        .expect("a reflection")
        .clone();
    assert!(
        r.text.contains("generous") && r.pinned && r.importance > 0.7,
        "{r:?}"
    );
    assert!(!m.reflection_due(1.5), "the clock restarted");
    assert_eq!(
        h.session("anna").expect("session").transcript().len(),
        0,
        "a reflection is not a conversation turn"
    );
    assert!(
        !h.maybe_reflect("anna", 300.0).expect("npc"),
        "not due again"
    );
    h.player_says("anna", "Do you trust me?", 400.0)
        .expect("asks");
    assert!(
        h.take_outbox()
            .remove(0)
            .prompt
            .contains("I can trust them"),
        "the reflection is retrieved into later prompts"
    );
}

#[test]
fn an_unparseable_reflection_is_dropped_without_a_retry_and_the_npc_may_try_again_later() {
    let mut h = host();
    busy(&mut h);
    h.maybe_reflect("anna", 200.0).expect("npc");
    let request = h.take_outbox().remove(0);
    h.deliver(
        Response {
            id: request.id,
            outcome: Ok("I cannot do that.".into()),
        },
        205.0,
    );
    assert!(
        h.take_outbox().is_empty(),
        "reflections are not worth a repair attempt"
    );
    assert!(
        h.maybe_reflect("anna", 210.0).expect("npc"),
        "still due, so it can be asked again"
    );
}

#[test]
fn the_threshold_can_be_changed_and_unknown_npcs_are_errors() {
    let mut h = host();
    h.observe("anna", "A small thing.", 0.3, 1.0)
        .expect("remembers");
    h.set_reflection_threshold(0.2);
    assert!(h.maybe_reflect("anna", 2.0).expect("npc"));
    assert!(h.maybe_reflect("ghost", 2.0).is_err());
}

// ---- guidance in the prompt ----

#[test]
fn the_prompt_says_when_to_remember_and_when_to_adjust_feelings() {
    let mut h = host();
    h.player_says("anna", "Hello", 1.0).expect("asks");
    let prompt = h.take_outbox().remove(0).prompt;
    for part in [
        "remember",
        "gift",
        "promise",
        "Do not remember small talk",
        "adjust_relationship",
        "importance",
    ] {
        assert!(
            prompt.contains(part),
            "{part} missing from the guidance:\n{prompt}"
        );
    }
    let reply_format = prompt
        .find("## Reply format")
        .unwrap_or_else(|| panic!("no reply format"));
    assert!(
        prompt[reply_format..].contains("Do not remember small talk"),
        "the guidance sits with the reply format, the last thing the model reads"
    );
}
