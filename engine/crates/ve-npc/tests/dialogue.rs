//! Contract tests for the dialogue session model (ADR 0064, step M4a): everything a conversation screen needs to know, with **no UI in it**.
//! A future interface (the owner will design one in a Liquid Glass style) only binds to this model and its event stream: the transcript,
//! streaming text, whose turn it is, listening and thinking states, suggested replies, and the game-facing intents that come out of a reply.
#![expect(
    clippy::float_cmp,
    reason = "timestamps are deterministic contract values"
)]

use std::collections::BTreeMap;

use ve_npc::{
    ArgValue, Delivery, DialogueError, DialogueSession, Intent, RequestId, SessionEvent,
    SessionState, Speaker,
};

fn say(text: &str) -> Intent {
    Intent {
        name: "say".into(),
        args: BTreeMap::from([("text".to_owned(), ArgValue::Text(text.to_owned()))]),
    }
}

fn delivery(intents: Vec<Intent>, fallback: bool) -> Delivery {
    Delivery {
        request: RequestId(1),
        npc: "anna".into(),
        intents,
        rejected: Vec::new(),
        fallback,
    }
}

#[test]
fn a_new_session_is_idle_and_empty() {
    let s = DialogueSession::new("anna", 0.0);
    assert_eq!(s.npc(), "anna");
    assert_eq!(s.state(), SessionState::Idle);
    assert!(s.transcript().is_empty());
    assert!(s.suggestions().is_empty());
}

#[test]
fn the_player_speaking_adds_a_completed_turn_and_makes_the_npc_think() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.player_says("Hello there", 1.0).expect("open");
    assert_eq!(s.state(), SessionState::Thinking);
    let turn = &s.transcript()[0];
    assert_eq!(
        (
            turn.id,
            turn.speaker.clone(),
            turn.text.as_str(),
            turn.complete
        ),
        (1, Speaker::Player, "Hello there", true)
    );
    assert_eq!(turn.at, 1.0);
    let events = s.drain_events();
    assert_eq!(
        events,
        vec![
            SessionEvent::TurnStarted {
                id: 1,
                speaker: Speaker::Player
            },
            SessionEvent::TurnCompleted { id: 1 },
            SessionEvent::StateChanged(SessionState::Thinking),
        ]
    );
    assert!(s.drain_events().is_empty(), "draining empties the queue");
}

#[test]
fn streamed_text_builds_one_turn_and_the_final_text_wins() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.player_says("Hi", 1.0).expect("open");
    s.drain_events();
    s.npc_delta("Good ", 1.5).expect("open");
    s.npc_delta("morning", 1.6).expect("open");
    assert_eq!(s.state(), SessionState::Speaking);
    assert_eq!(s.transcript()[1].text, "Good morning");
    assert!(!s.transcript()[1].complete);
    s.npc_complete(
        "Good morning!",
        vec!["Tell me more".into(), "Goodbye".into()],
        2.0,
    )
    .expect("open");
    assert_eq!(
        s.transcript()[1].text,
        "Good morning!",
        "the validated final text replaces what streamed"
    );
    assert!(s.transcript()[1].complete);
    assert_eq!(s.state(), SessionState::Idle);
    assert_eq!(s.suggestions(), ["Tell me more", "Goodbye"]);
    let events = s.drain_events();
    assert_eq!(
        events,
        vec![
            SessionEvent::TurnStarted {
                id: 2,
                speaker: Speaker::Npc("anna".into())
            },
            SessionEvent::StateChanged(SessionState::Speaking),
            SessionEvent::TextDelta {
                id: 2,
                delta: "Good ".into()
            },
            SessionEvent::TextDelta {
                id: 2,
                delta: "morning".into()
            },
            SessionEvent::TextReplaced {
                id: 2,
                text: "Good morning!".into()
            },
            SessionEvent::TurnCompleted { id: 2 },
            SessionEvent::StateChanged(SessionState::Idle),
            SessionEvent::SuggestionsChanged(vec!["Tell me more".into(), "Goodbye".into()]),
        ]
    );
}

#[test]
fn completing_without_a_difference_sends_no_replacement() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.npc_delta("Hello", 0.1).expect("open");
    s.drain_events();
    s.npc_complete("Hello", Vec::new(), 0.2).expect("open");
    assert!(
        !s.drain_events()
            .iter()
            .any(|e| matches!(e, SessionEvent::TextReplaced { .. }))
    );
}

#[test]
fn the_conversation_for_a_prompt_is_the_completed_turns_with_names() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.player_says("Hello", 0.0).expect("open");
    s.npc_complete("Good morning!", Vec::new(), 1.0)
        .expect("open");
    s.player_says("What do you sell?", 2.0).expect("open");
    s.npc_delta("Bre", 2.5).expect("open");
    let all = s.conversation(10);
    assert_eq!(
        all,
        vec![
            ("player".to_owned(), "Hello".to_owned()),
            ("anna".to_owned(), "Good morning!".to_owned()),
            ("player".to_owned(), "What do you sell?".to_owned())
        ],
        "the unfinished reply is not history yet"
    );
    let tail = s.conversation(2);
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[1].1, "What do you sell?", "the newest turns, in order");
}

#[test]
fn a_mind_delivery_becomes_a_reply_emotes_and_game_intents() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.player_says("Hi", 0.0).expect("open");
    s.drain_events();
    let emote = Intent {
        name: "emote".into(),
        args: BTreeMap::from([("name".to_owned(), ArgValue::Text("smile".to_owned()))]),
    };
    let discount = Intent {
        name: "offer_discount".into(),
        args: BTreeMap::from([("percent".to_owned(), ArgValue::Number(10.0))]),
    };
    s.apply_delivery(
        &delivery(vec![say("Welcome!"), emote, discount.clone()], false),
        1.0,
    )
    .expect("open");
    let turn = &s.transcript()[1];
    assert_eq!(
        (turn.text.as_str(), turn.complete, turn.fallback),
        ("Welcome!", true, false)
    );
    let events = s.drain_events();
    assert!(events.contains(&SessionEvent::Emote {
        name: "smile".into()
    }));
    assert!(
        events.contains(&SessionEvent::Intent(discount)),
        "intents the dialogue does not own are passed on for the game"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SessionEvent::Intent(i) if i.name == "say" || i.name == "emote")),
        "say and emote are handled here"
    );
    assert_eq!(s.state(), SessionState::Idle);
}

#[test]
fn a_fallback_reply_is_marked_so_the_interface_can_tell() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.player_says("Hi", 0.0).expect("open");
    s.apply_delivery(&delivery(vec![say("Hm? Say that again.")], true), 1.0)
        .expect("open");
    assert!(s.transcript()[1].fallback);
    let mut empty = DialogueSession::new("anna", 0.0);
    empty.player_says("Hi", 0.0).expect("open");
    empty
        .apply_delivery(&delivery(Vec::new(), true), 1.0)
        .expect("open");
    assert_eq!(
        empty.transcript().len(),
        1,
        "no line to show, no turn made up"
    );
    assert_eq!(
        empty.state(),
        SessionState::Idle,
        "but the NPC is no longer thinking"
    );
}

#[test]
fn voice_input_has_a_listening_state() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.set_listening(true).expect("open");
    assert_eq!(s.state(), SessionState::Listening);
    assert_eq!(
        s.drain_events(),
        vec![SessionEvent::StateChanged(SessionState::Listening)]
    );
    s.set_listening(true).expect("open");
    assert!(s.drain_events().is_empty(), "no change, no event");
    s.player_says("Open the door", 1.0).expect("open");
    assert_eq!(s.state(), SessionState::Thinking);
    s.set_listening(false).expect("open");
    assert_eq!(
        s.state(),
        SessionState::Thinking,
        "stopping the microphone does not interrupt thinking"
    );
}

#[test]
fn old_turns_are_trimmed_to_a_cap_and_counted() {
    let mut s = DialogueSession::with_capacity("anna", 0.0, 4);
    for i in 0..6 {
        s.player_says(&format!("line {i}"), f64::from(i))
            .expect("open");
        s.npc_complete(&format!("answer {i}"), Vec::new(), f64::from(i) + 0.5)
            .expect("open");
    }
    assert_eq!(s.transcript().len(), 4);
    assert_eq!(s.trimmed(), 8);
    assert_eq!(s.transcript()[0].text, "line 4");
    assert_eq!(
        s.transcript()[0].id,
        9,
        "ids keep counting, they never repeat"
    );
}

#[test]
fn a_closed_session_refuses_everything() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.close(5.0);
    assert_eq!(s.state(), SessionState::Closed);
    assert!(s.drain_events().contains(&SessionEvent::Closed));
    assert!(matches!(
        s.player_says("Hi", 6.0),
        Err(DialogueError::Closed)
    ));
    assert!(matches!(s.npc_delta("x", 6.0), Err(DialogueError::Closed)));
    assert!(matches!(s.set_listening(true), Err(DialogueError::Closed)));
    s.close(7.0);
    assert!(
        !s.drain_events().contains(&SessionEvent::Closed),
        "closing twice is quiet"
    );
}

#[test]
fn a_session_saves_and_loads_its_transcript_and_state_but_not_its_pending_events() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.player_says("Hello", 1.0).expect("open");
    s.npc_complete("Good morning!", vec!["Tell me more".into()], 2.0)
        .expect("open");
    s.player_says("Go on", 3.0).expect("open");
    let json = s.to_json();
    let mut loaded = DialogueSession::from_json(&json).expect("loads");
    assert_eq!(loaded.to_json(), json, "save, load, save is stable");
    assert_eq!(loaded.transcript(), s.transcript());
    assert_eq!(loaded.state(), SessionState::Thinking);
    assert_eq!(loaded.suggestions(), s.suggestions());
    assert!(loaded.drain_events().is_empty());
    loaded
        .npc_complete("Well...", Vec::new(), 4.0)
        .expect("open");
    assert_eq!(
        loaded.transcript().last().map(|t| t.id),
        Some(4),
        "ids continue after a load"
    );
    assert!(DialogueSession::from_json("{nope").is_err());
}

#[test]
fn the_player_interrupting_a_streaming_reply_ends_it_and_starts_a_new_turn() {
    let mut s = DialogueSession::new("anna", 0.0);
    s.player_says("Hi", 0.0).expect("open");
    s.npc_delta("Let me tell you a very long", 1.0)
        .expect("open");
    s.drain_events();
    s.player_says("Stop", 2.0).expect("open");
    assert!(
        s.transcript()[1].complete,
        "the cut-off reply is closed where it was"
    );
    assert!(s.transcript()[1].interrupted);
    assert_eq!(s.transcript()[2].text, "Stop");
    assert_eq!(s.state(), SessionState::Thinking);
    let events = s.drain_events();
    assert!(events.contains(&SessionEvent::TurnCompleted { id: 2 }));
}
