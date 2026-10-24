//! Additional tests for the NPC host command and integration paths.
#![expect(clippy::expect_used, reason = "tests assert by failing")]

use serde_json::{Value, json};
use ve_npc::{ArgType, IntentSpec, MindBudget, NpcHost, NpcSpec, Response};

fn npc(name: &str) -> NpcSpec {
    NpcSpec {
        name: name.into(),
        persona: "A helpful NPC.".into(),
        rules: "Be kind.".into(),
        goals: Vec::new(),
        world_facts: Vec::new(),
    }
}
fn host() -> NpcHost {
    let mut h = NpcHost::new(MindBudget {
        calls_per_minute: 100,
        max_concurrent: 100,
        tokens_per_minute: 1_000_000,
        timeout_secs: 8.0,
    });
    h.register(npc("a")).expect("register");
    h
}
fn reply(intents: Value) -> String {
    json!({"intents": intents}).to_string()
}

#[test]
fn prompt_budget_retrieval_and_conversation_are_bounded_and_ordered() {
    let mut h = host();
    let mut last = None;
    for i in 0..200 {
        let id = h
            .observe(
                "a",
                &format!("Long memory {i}: {}", "bread memory ".repeat(80)),
                0.5,
                f64::from(i),
            )
            .expect("observe");
        last = Some(id);
    }
    let last = last.expect("one memory");
    let before = h
        .memory("a")
        .expect("memory")
        .get(last)
        .expect("memory")
        .access_count;
    h.player_says("a", "bread", 300.0).expect("says");
    let request = h.take_outbox().remove(0);
    assert!(request.prompt.chars().count() <= 12_000);
    assert_eq!(
        h.memory("a")
            .expect("memory")
            .get(last)
            .expect("memory")
            .access_count,
        before + 1
    );
    h.deliver(
        Response {
            id: request.id,
            outcome: Ok(reply(
                json!([{"name":"say","args":{"text":"Fresh bread!"}}]),
            )),
        },
        301.0,
    )
    .expect("deliver");
    h.player_says("a", "Thanks", 302.0).expect("says");
    assert!(h.take_outbox().remove(0).prompt.contains("a: Fresh bread!"));
}

#[test]
fn npc_memories_are_isolated_and_unknown_intents_do_not_hide_speech() {
    let mut h = host();
    h.register(npc("b")).expect("register");
    h.observe("a", "private memory", 0.8, 0.0).expect("observe");
    assert!(h.memory("b").expect("memory").is_empty());
    h.player_says("a", "hello", 1.0).expect("says");
    let request = h.take_outbox().remove(0);
    let raw = reply(json!([{"name":"say","args":{"text":"Visible"}},{"name":"mystery","args":{}}]));
    h.deliver(
        Response {
            id: request.id,
            outcome: Ok(raw),
        },
        2.0,
    )
    .expect("deliver");
    assert_eq!(
        h.session("a")
            .expect("session")
            .transcript()
            .last()
            .expect("turn")
            .text,
        "Visible"
    );
}

#[test]
fn custom_intents_reach_events_and_memory_ranking_does_not_touch() {
    let mut h = host();
    h.add_intent(IntentSpec::new("wave", "Wave.").arg("happy", ArgType::Bool))
        .expect("intent");
    h.observe("a", "waving", 0.4, 0.0).expect("observe");
    let access = h.memory("a").expect("memory").recent(1)[0].access_count;
    h.handle(&json!({"cmd":"memory","npc":"a","query":"wave","now":1,"k":1}).to_string());
    assert_eq!(
        h.memory("a").expect("memory").recent(1)[0].access_count,
        access
    );
    h.player_says("a", "hello", 1.0).expect("says");
    let request = h.take_outbox().remove(0);
    h.deliver(
        Response {
            id: request.id,
            outcome: Ok(reply(json!([{"name":"wave","args":{"happy":true}}]))),
        },
        2.0,
    )
    .expect("deliver");
    assert!(
        h.drain_events("a")
            .expect("events")
            .iter()
            .any(|e| matches!(e,ve_npc::SessionEvent::Intent(i) if i.name=="wave"))
    );
}

#[test]
fn commands_report_missing_fields_and_seeded_input_never_panics() {
    let mut h = host();
    for command in [
        "register",
        "observe",
        "player_says",
        "take_outbox",
        "deliver",
        "tick",
        "memory",
        "transcript",
        "events",
        "save",
    ] {
        let output: Value =
            serde_json::from_str(&h.handle(&json!({"cmd":command}).to_string())).expect("JSON");
        if !matches!(command, "take_outbox" | "save") {
            assert_eq!(output["ok"], false, "{command}: {output}");
        }
    }
    let out: Value = serde_json::from_str(
        &h.handle(&json!({"cmd":"player_says","npc":"a","text":"hello","now":1}).to_string()),
    )
    .expect("player command JSON");
    assert_eq!(out["ok"], true);
    let requests: Value =
        serde_json::from_str(&h.handle(r#"{"cmd":"take_outbox"}"#)).expect("outbox JSON");
    let id = requests["requests"][0]["id"].clone();
    let delivered: Value = serde_json::from_str(&h.handle(
        &json!({"cmd":"deliver","id":id,"ok":false,"text":"offline","now":20}).to_string(),
    ))
    .expect("delivery JSON");
    assert_eq!(delivered["ok"], true);
    let events: Value =
        serde_json::from_str(&h.handle(r#"{"cmd":"events","npc":"a"}"#)).expect("events JSON");
    assert_eq!(events["ok"], true);
    let tick: Value =
        serde_json::from_str(&h.handle(r#"{"cmd":"tick","now":50}"#)).expect("tick JSON");
    assert_eq!(tick["ok"], true);
    let save: Value = serde_json::from_str(&h.handle(r#"{"cmd":"save"}"#)).expect("save JSON");
    assert!(save["save"].as_str().is_some());
    let mut seed = 0x1234_5678_u32;
    for _ in 0..3000 {
        let mut bytes = Vec::new();
        for _ in 0..usize::try_from(seed % 80).expect("length fits usize") {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            bytes.push((seed >> 24) as u8);
        }
        let input = String::from_utf8_lossy(&bytes);
        let _ = h.handle(&input);
    }
    for _ in 0..3000 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let command = match seed % 8 {
            0 => {
                json!({"cmd":"observe","npc":"a","text":format!("fuzz {}",seed),"importance":0.3,"now":f64::from(seed)})
            }
            1 => {
                json!({"cmd":"player_says","npc":"a","text":format!("fuzz {}",seed),"now":f64::from(seed)})
            }
            2 => {
                json!({"cmd":"memory","npc":"a","query":format!("fuzz {}",seed),"now":f64::from(seed),"k":3})
            }
            3 => json!({"cmd":"transcript","npc":"a"}),
            4 => json!({"cmd":"events","npc":"a"}),
            5 => json!({"cmd":"take_outbox"}),
            6 => json!({"cmd":"tick","now":f64::from(seed)}),
            _ => json!({"cmd":"save"}),
        };
        let bytes = command.to_string();
        let _ = h.handle(&bytes);
    }
    for i in 0..3000 {
        let command = json!({"cmd":"observe","npc":"a","text":format!("random {i}"),"importance":0.3,"now":i});
        let out: Value = serde_json::from_str(&h.handle(&command.to_string())).expect("JSON");
        assert_eq!(out["ok"], true);
    }
}

#[test]
fn long_scripted_run_keeps_bounded_memory_and_transcript() {
    let mut h = host();
    for i in 0..50 {
        h.player_says("a", &format!("line {i}"), f64::from(i))
            .expect("says");
        let request = h.take_outbox().remove(0);
        h.deliver(
            Response {
                id: request.id,
                outcome: Ok(reply(
                    json!([{"name":"say","args":{"text":format!("answer {i}")}}]),
                )),
            },
            f64::from(i) + 0.5,
        )
        .expect("deliver");
    }
    assert!(h.memory("a").expect("memory").len() <= 512);
    assert!(h.session("a").expect("session").transcript().len() <= 200);
}
