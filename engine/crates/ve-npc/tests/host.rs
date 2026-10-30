//! Contract tests for the NPC host (ADR 0064, step M4b): the glue that makes an NPC intelligent. A player speaks; the host remembers it,
//! retrieves the memories relevant to what was said, assembles the prompt, asks the mind (budgets, queue, fallback), and when the reply comes
//! back it validates it, shows the line through the dialogue session and applies the intents that change memory. One JSON command surface
//! sits on top, so the page (and later the language and the tools) drives all of it through a single call.
#![expect(clippy::expect_used, reason = "tests assert by failing")]

use serde_json::{Value as Json, json};
use ve_npc::{
    FallbackTable, HostError, MemoryKind, MindBudget, NpcHost, NpcSpec, Reason, Refusal, Response,
    ScriptedProvider, SessionEvent, Source,
};

fn baker() -> NpcSpec {
    NpcSpec {
        name: "anna".into(),
        persona: "You are Anna, the village baker. Warm, brisk, proud of your bread.".into(),
        rules: "Stay in character. Never mention being an AI.".into(),
        goals: vec!["Sell all the bread before noon.".into()],
        world_facts: vec!["The village has a mill, a smithy and a well.".into()],
    }
}

fn roomy() -> MindBudget {
    MindBudget {
        calls_per_minute: 100,
        max_concurrent: 100,
        tokens_per_minute: 1_000_000,
        timeout_secs: 8.0,
    }
}

fn host() -> NpcHost {
    let mut h = NpcHost::new(roomy());
    h.register(baker()).expect("registers");
    h
}

fn reply(text: &str) -> String {
    json!({"intents": [{"name": "say", "args": {"text": text}}]}).to_string()
}

#[test]
fn a_player_line_becomes_a_request_with_the_persona_and_the_line_in_the_prompt() {
    let mut h = host();
    h.player_says("anna", "Do you have any rye bread?", 10.0)
        .expect("asks");
    let out = h.take_outbox();
    assert_eq!(out.len(), 1);
    assert_eq!((out[0].npc.as_str(), out[0].priority), ("anna", 100));
    let p = &out[0].prompt;
    for part in [
        "You are Anna",
        "Stay in character",
        "mill, a smithy",
        "Sell all the bread",
        "Do you have any rye bread?",
        "## Reply format",
        "say(",
    ] {
        assert!(p.contains(part), "{part} missing from:\n{p}");
    }
    assert_eq!(
        h.session("anna").expect("session").transcript().len(),
        1,
        "the player's turn is already in the transcript"
    );
}

#[test]
fn memories_relevant_to_what_was_said_are_in_the_prompt_and_unrelated_ones_are_not() {
    let mut h = host();
    h.set_retrieval_count(1);
    h.observe(
        "anna",
        "The player gave me a gold coin for the bread.",
        0.8,
        0.0,
    )
    .expect("remembers");
    h.observe("anna", "Wolves howled on the hill at night.", 0.8, 0.0)
        .expect("remembers");
    h.player_says("anna", "Do you remember the gold coin?", 7_200.0)
        .expect("asks");
    let prompt = h.take_outbox().remove(0).prompt;
    assert!(prompt.contains("gold coin for the bread"), "{prompt}");
    assert!(
        !prompt.contains("Wolves howled"),
        "only the one retrieved memory was asked for: {prompt}"
    );
    assert!(
        prompt.contains("(2 hours ago)"),
        "ages are written for the model: {prompt}"
    );
}

#[test]
fn ages_are_written_the_same_way_every_time() {
    let mut h = host();
    h.set_retrieval_count(5);
    for (text, at) in [("a moment", 99.0), ("some seconds", 70.0), ("minutes", 0.0)] {
        h.observe("anna", &format!("It happened {text}."), 0.5, at)
            .expect("remembers");
    }
    h.player_says("anna", "It happened", 100.0).expect("asks");
    let prompt = h.take_outbox().remove(0).prompt;
    assert!(prompt.contains("(just now)"), "{prompt}");
    assert!(prompt.contains("(30 seconds ago)"), "{prompt}");
    assert!(prompt.contains("(1 minute ago)"), "{prompt}");
}

#[test]
fn a_reply_is_shown_validated_and_its_intents_change_the_npcs_memory() {
    let mut h = host();
    h.player_says("anna", "I brought you flour.", 1.0)
        .expect("asks");
    let id = h.take_outbox().remove(0).id;
    let text = json!({"intents": [
        {"name": "say", "args": {"text": "Bless you, the mill is far!"}},
        {"name": "remember", "args": {"text": "The player brought me flour.", "importance": 0.7}},
        {"name": "set_fact", "args": {"key": "player.helpful", "value": "yes"}},
        {"name": "adjust_relationship", "args": {"other": "player", "affinity": 0.2, "trust": 0.1}},
    ]})
    .to_string();
    let delivery = h
        .deliver(
            Response {
                id,
                outcome: Ok(text),
            },
            2.0,
        )
        .expect("known request");
    assert!(!delivery.fallback);
    let s = h.session("anna").expect("session");
    assert_eq!(
        s.transcript().last().map(|t| t.text.as_str()),
        Some("Bless you, the mill is far!")
    );
    let m = h.memory("anna").expect("memory");
    assert!(
        m.recent(10)
            .iter()
            .any(|x| x.text == "The player brought me flour."
                && x.kind == MemoryKind::Event
                && x.importance > 0.69)
    );
    assert_eq!(
        m.fact("player.helpful").map(|f| f.value.as_str()),
        Some("yes")
    );
    let r = m.relationship("player");
    assert!(
        (r.affinity - 0.2).abs() < 1e-6 && (r.trust - 0.1).abs() < 1e-6,
        "{r:?}"
    );
}

#[test]
fn what_was_said_on_both_sides_is_remembered_as_dialogue() {
    let mut h = host();
    h.player_says("anna", "Good morning!", 1.0).expect("asks");
    let id = h.take_outbox().remove(0).id;
    h.deliver(
        Response {
            id,
            outcome: Ok(reply("Morning to you!")),
        },
        2.0,
    )
    .expect("known");
    let texts: Vec<String> = h
        .memory("anna")
        .expect("memory")
        .recent(10)
        .iter()
        .filter(|m| m.kind == MemoryKind::Dialogue)
        .map(|m| m.text.clone())
        .collect();
    assert!(
        texts
            .iter()
            .any(|t| t.contains("player") && t.contains("Good morning!")),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.contains("Morning to you!")),
        "{texts:?}"
    );
}

#[test]
fn a_remembered_gift_makes_the_scripted_model_answer_because_retrieval_put_it_in_the_prompt() {
    // The provider only answers when the prompt contains the memory: this proves the memory travelled all the way into what the model sees.
    let mut provider = ScriptedProvider::new().reply_when(
        "gold coin",
        &reply("Oh, the generous soul with the gold coin! Fresh loaf, on the house."),
    );
    let mut h = host();
    h.observe("anna", "The player gave me a gold coin.", 0.9, 0.0)
        .expect("remembers");
    h.player_says("anna", "Remember me?", 86_400.0)
        .expect("asks");
    for request in h.take_outbox() {
        let response = provider.respond(&request);
        h.deliver(response, 86_401.0).expect("known");
    }
    let last = h
        .session("anna")
        .expect("session")
        .transcript()
        .last()
        .expect("a turn")
        .clone();
    assert!(
        last.text.contains("gold coin") && !last.fallback,
        "{last:?}"
    );
}

#[test]
fn a_failed_model_call_gives_the_canned_line_and_marks_it() {
    let mut h = host();
    h.set_fallback(
        "anna",
        FallbackTable::new().line(Reason::PlayerSpoke, "Hm? Say that again."),
    );
    h.player_says("anna", "Hello", 1.0).expect("asks");
    let id = h.take_outbox().remove(0).id;
    let delivery = h
        .deliver(
            Response {
                id,
                outcome: Err("503".into()),
            },
            2.0,
        )
        .expect("known");
    assert!(delivery.fallback);
    let turn = h
        .session("anna")
        .expect("session")
        .transcript()
        .last()
        .expect("turn")
        .clone();
    assert_eq!(
        (turn.text.as_str(), turn.fallback),
        ("Hm? Say that again.", true)
    );
}

#[test]
fn a_slow_model_is_replaced_at_the_timeout() {
    let mut h = host();
    h.set_fallback(
        "anna",
        FallbackTable::new().line(Reason::PlayerSpoke, "One moment..."),
    );
    h.player_says("anna", "Hello", 0.0).expect("asks");
    h.take_outbox();
    assert!(h.tick(5.0).is_empty());
    let late = h.tick(9.0);
    assert_eq!(late.len(), 1);
    assert_eq!(
        h.session("anna")
            .expect("session")
            .transcript()
            .last()
            .map(|t| t.text.as_str()),
        Some("One moment...")
    );
}

#[test]
fn a_refused_ask_still_answers_the_player_at_once_with_a_fallback() {
    let mut h = NpcHost::new(MindBudget {
        calls_per_minute: 1,
        ..roomy()
    });
    h.register(baker()).expect("registers");
    h.set_fallback(
        "anna",
        FallbackTable::new().line(Reason::PlayerSpoke, "I need a moment."),
    );
    h.player_says("anna", "one", 0.0).expect("asks");
    h.take_outbox();
    let refusal = h
        .player_says("anna", "two", 5.0)
        .expect("the player is still answered");
    assert_eq!(refusal, Some(Refusal::RateLimited), "the caller learns why");
    assert!(h.take_outbox().is_empty(), "nothing was sent to the model");
    assert_eq!(
        h.session("anna")
            .expect("session")
            .transcript()
            .last()
            .map(|t| t.text.as_str()),
        Some("I need a moment.")
    );
}

#[test]
fn gossip_moves_a_memory_to_another_npc_with_provenance_and_it_shows_up_in_their_prompt() {
    let mut h = host();
    h.register(NpcSpec {
        name: "ben".into(),
        persona: "You are Ben, the smith.".into(),
        rules: "Stay in character.".into(),
        goals: vec![],
        world_facts: vec![],
    })
    .expect("registers");
    let id = h
        .observe("anna", "The miller owes the baker money.", 0.8, 0.0)
        .expect("remembers");
    let told = h.gossip("anna", "ben", id, 10.0, 2).expect("tells");
    let m = h
        .memory("ben")
        .expect("memory")
        .get(told)
        .expect("memory")
        .clone();
    assert_eq!(
        m.source,
        Source::Told {
            from: "anna".into(),
            hops: 1
        }
    );
    h.player_says("ben", "Do you know anything about the miller?", 20.0)
        .expect("asks");
    assert!(
        h.take_outbox()
            .remove(0)
            .prompt
            .contains("The miller owes the baker money.")
    );
    assert!(matches!(
        h.gossip("anna", "ghost", id, 1.0, 2),
        Err(HostError::UnknownNpc(_))
    ));
}

#[test]
fn unknown_and_duplicate_npcs_are_errors() {
    let mut h = host();
    assert!(matches!(
        h.player_says("nobody", "hi", 0.0),
        Err(HostError::UnknownNpc(_))
    ));
    assert!(matches!(h.register(baker()), Err(HostError::Duplicate(_))));
    assert!(h.memory("nobody").is_none() && h.session("nobody").is_none());
}

#[test]
fn the_whole_host_saves_and_loads() {
    let mut h = host();
    h.observe("anna", "The player gave me a gold coin.", 0.9, 0.0)
        .expect("remembers");
    h.player_says("anna", "Hi", 1.0).expect("asks");
    let id = h.take_outbox().remove(0).id;
    h.deliver(
        Response {
            id,
            outcome: Ok(reply("Hello!")),
        },
        2.0,
    )
    .expect("known");
    let saved = h.to_json();
    let mut loaded = NpcHost::from_json(&saved, roomy()).expect("loads");
    assert_eq!(loaded.to_json(), saved, "save, load, save is stable");
    assert_eq!(
        loaded.session("anna").expect("session").transcript().len(),
        2
    );
    assert!(
        loaded
            .memory("anna")
            .expect("memory")
            .recent(10)
            .iter()
            .any(|m| m.text.contains("gold coin"))
    );
    loaded
        .player_says("anna", "Again", 3.0)
        .expect("a loaded host keeps working");
    assert!(NpcHost::from_json("{nope", roomy()).is_err());
}

#[test]
fn events_are_available_per_npc_in_order() {
    let mut h = host();
    h.player_says("anna", "Hi", 1.0).expect("asks");
    let id = h.take_outbox().remove(0).id;
    h.deliver(
        Response {
            id,
            outcome: Ok(reply("Hello!")),
        },
        2.0,
    )
    .expect("known");
    let events = h.drain_events("anna").expect("npc exists");
    assert!(matches!(
        events.first(),
        Some(SessionEvent::TurnStarted { .. })
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::TurnCompleted { .. }))
    );
    assert!(h.drain_events("anna").expect("npc exists").is_empty());
}

// ---- the single JSON command surface ----

fn call(h: &mut NpcHost, command: &Json) -> Json {
    serde_json::from_str(&h.handle(&command.to_string())).expect("the reply is JSON")
}

#[test]
fn one_json_call_drives_everything() {
    let mut h = NpcHost::new(roomy());
    let r = call(
        &mut h,
        &json!({"cmd": "register", "npc": {"name": "anna", "persona": "You are Anna.", "rules": "Stay in character.", "goals": [], "world_facts": []}}),
    );
    assert_eq!(r["ok"], json!(true), "{r}");
    let r = call(
        &mut h,
        &json!({"cmd": "player_says", "npc": "anna", "text": "Hi", "now": 1.0}),
    );
    assert_eq!(r["ok"], json!(true), "{r}");
    let r = call(&mut h, &json!({"cmd": "take_outbox"}));
    let requests = r["requests"].as_array().expect("a list");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["npc"], json!("anna"));
    assert!(requests[0]["prompt"].as_str().expect("text").contains("Hi"));
    let id = requests[0]["id"].clone();
    let r = call(
        &mut h,
        &json!({"cmd": "deliver", "id": id, "ok": true, "text": reply("Hello, friend!"), "now": 2.0}),
    );
    assert_eq!(r["ok"], json!(true), "{r}");
    assert_eq!(r["fallback"], json!(false), "{r}");
    assert!(
        r["events"].as_array().is_some_and(|e| !e.is_empty()),
        "the session events come back with the delivery: {r}"
    );
    let r = call(
        &mut h,
        &json!({"cmd": "memory", "npc": "anna", "query": "Hi", "now": 3.0, "k": 3}),
    );
    assert!(
        r["memories"]
            .as_array()
            .expect("memories")
            .iter()
            .any(|m| m["text"].as_str().unwrap_or("").contains("Hello, friend!")),
        "{r}"
    );
    let r = call(&mut h, &json!({"cmd": "transcript", "npc": "anna"}));
    assert_eq!(r["turns"].as_array().expect("turns").len(), 2);
}

#[test]
fn bad_commands_answer_with_an_error_and_never_panic() {
    let mut h = host();
    for bad in [
        "{nope",
        "[]",
        r#"{"cmd": 7}"#,
        r#"{"cmd": "teleport"}"#,
        r#"{"cmd": "player_says"}"#,
        r#"{"cmd": "player_says", "npc": "ghost", "text": "x", "now": 0}"#,
    ] {
        let r: Json = serde_json::from_str(&h.handle(bad)).expect("always JSON");
        assert_eq!(r["ok"], json!(false), "{bad} -> {r}");
        assert!(r["error"].as_str().is_some_and(|e| !e.is_empty()), "{r}");
    }
}
