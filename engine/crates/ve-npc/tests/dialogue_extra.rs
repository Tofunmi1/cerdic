//! Additional dialogue session edge-case tests.
use std::collections::BTreeMap;
use ve_npc::{ArgValue, Delivery, DialogueSession, Intent, RequestId, SessionEvent, SessionState};

fn intent(name: &str, key: &str, value: &str) -> Intent {
    Intent {
        name: name.into(),
        args: BTreeMap::from([(key.into(), ArgValue::Text(value.into()))]),
    }
}

#[test]
fn delivery_joins_lines_and_drops_emotes_without_names() {
    let mut session = DialogueSession::new("anna", 0.0);
    let delivery = Delivery {
        request: RequestId(1),
        npc: "anna".into(),
        intents: vec![
            intent("say", "text", "one"),
            intent("say", "text", "two"),
            Intent {
                name: "emote".into(),
                args: BTreeMap::new(),
            },
        ],
        rejected: Vec::new(),
        fallback: false,
    };
    session.apply_delivery(&delivery, 1.0).expect("open");
    assert_eq!(session.transcript()[0].text, "one\ntwo");
    assert!(
        !session
            .drain_events()
            .iter()
            .any(|e| matches!(e, SessionEvent::Emote { .. }))
    );
}

#[test]
fn completed_turns_restart_stream_and_zero_capacity_is_safe() {
    let mut session = DialogueSession::with_capacity("anna", 0.0, 1);
    session.npc_complete("done", Vec::new(), 1.0).expect("open");
    session.npc_delta("next", 2.0).expect("open");
    assert_eq!(session.transcript().len(), 1);
    assert_eq!(session.transcript()[0].text, "next");
    assert_eq!(session.conversation(0), Vec::<(String, String)>::new());
    let mut zero = DialogueSession::with_capacity("anna", 0.0, 0);
    zero.npc_complete("not retained", Vec::new(), 1.0)
        .expect("open");
    assert_eq!(zero.trimmed(), 1);
}

#[test]
fn listening_does_not_interrupt_speaking_and_suggestions_can_clear() {
    let mut session = DialogueSession::new("anna", 0.0);
    session.npc_delta("hello", 1.0).expect("open");
    session.set_listening(true).expect("open");
    assert_eq!(session.state(), SessionState::Speaking);
    session
        .npc_complete("hello", vec!["reply".into()], 2.0)
        .expect("open");
    session.drain_events();
    session
        .npc_complete("hello", Vec::new(), 3.0)
        .expect("open");
    assert_eq!(
        session
            .drain_events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::SuggestionsChanged(_)))
            .count(),
        1
    );
}

#[test]
fn load_preserves_capacity_and_trimmed_count_and_idle_interrupt_adds_only_player() {
    let mut session = DialogueSession::with_capacity("anna", 0.0, 1);
    session.player_says("old", 1.0).expect("open");
    session.player_says("new", 2.0).expect("open");
    assert_eq!(session.trimmed(), 1);
    let mut loaded = DialogueSession::from_json(&session.to_json()).expect("valid save");
    assert_eq!(loaded.trimmed(), 1);
    loaded.player_says("latest", 3.0).expect("open");
    assert_eq!(loaded.trimmed(), 2);
    assert_eq!(loaded.transcript().len(), 1);
}

#[test]
fn random_operations_never_duplicate_ids_or_panic() {
    let mut seed = 0x7a31_93d5_u64;
    let mut session = DialogueSession::with_capacity("anna", 0.0, 13);
    for i in 0..5000 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        match seed % 6 {
            0 => {
                let _ = session.player_says("hello", f64::from(i));
            }
            1 => {
                let _ = session.npc_delta("x", f64::from(i));
            }
            2 => {
                let _ = session.npc_complete("final", Vec::new(), f64::from(i));
            }
            3 => {
                let _ = session.set_listening(seed & 0x100 != 0);
            }
            4 => {
                let _ = DialogueSession::from_json(&session.to_json());
            }
            _ => {
                if session.state() == SessionState::Closed {
                    session = DialogueSession::new("anna", 0.0);
                } else {
                    session.close(f64::from(i));
                }
            }
        }
        let mut ids: Vec<_> = session.transcript().iter().map(|turn| turn.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), session.transcript().len());
    }
}

#[test]
fn interrupt_without_streaming_is_only_a_player_turn() {
    let mut session = DialogueSession::new("anna", 0.0);
    session.player_says("first", 1.0).expect("open");
    session.drain_events();
    session.player_says("second", 2.0).expect("open");
    assert_eq!(session.transcript().len(), 2);
    assert!(!session.transcript()[0].interrupted);
    assert_eq!(
        session
            .drain_events()
            .iter()
            .filter(|event| matches!(event, SessionEvent::TurnCompleted { .. }))
            .count(),
        1
    );
}

#[test]
fn event_stream_can_be_replayed_from_an_initial_save() {
    let mut original = DialogueSession::new("anna", 0.0);
    let initial = original.to_json();
    original.player_says("hello", 1.0).expect("open");
    original.npc_delta("good ", 2.0).expect("open");
    original.npc_delta("day", 2.1).expect("open");
    original
        .npc_complete("good day!", vec!["more".into()], 3.0)
        .expect("open");
    let events = original.drain_events();
    let mut replay = DialogueSession::from_json(&initial).expect("initial save");
    for event in events {
        match event {
            SessionEvent::TurnStarted {
                speaker: ve_npc::Speaker::Player,
                ..
            } => replay.player_says("hello", 1.0).expect("open"),
            SessionEvent::TextDelta { delta, .. } => replay.npc_delta(&delta, 2.0).expect("open"),
            SessionEvent::TextReplaced { text, .. } => replay
                .npc_complete(&text, vec!["more".into()], 3.0)
                .expect("open"),
            _ => {}
        }
    }
    assert_eq!(replay.transcript(), original.transcript());
}

#[test]
fn empty_say_and_missing_emote_arguments_are_dropped_and_capacity_one_trims() {
    let mut session = DialogueSession::with_capacity("anna", 0.0, 1);
    let delivery = Delivery {
        request: RequestId(1),
        npc: "anna".into(),
        intents: vec![
            Intent {
                name: "say".into(),
                args: BTreeMap::new(),
            },
            Intent {
                name: "emote".into(),
                args: BTreeMap::new(),
            },
        ],
        rejected: Vec::new(),
        fallback: false,
    };
    session.apply_delivery(&delivery, 1.0).expect("open");
    assert!(session.transcript().is_empty());
    session.player_says("one", 2.0).expect("open");
    session.player_says("two", 3.0).expect("open");
    assert_eq!(session.transcript().len(), 1);
    assert_eq!(session.trimmed(), 1);
}
