//! UI-independent dialogue session state and its ordered event stream.
use crate::{ArgValue, Delivery, Intent};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The participant who owns a dialogue turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Speaker {
    /// The human player.
    Player,
    /// The session's non-player character.
    Npc(String),
}

/// Current dialogue activity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// No active operation.
    Idle,
    /// Microphone input is active.
    Listening,
    /// Waiting for an NPC response.
    Thinking,
    /// The NPC is producing a reply.
    Speaking,
    /// The session has ended.
    Closed,
}

/// One transcript entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    /// Monotonically allocated turn identifier.
    pub id: u64,
    /// Turn author.
    pub speaker: Speaker,
    /// Current turn text.
    pub text: String,
    /// Whether the turn is complete.
    pub complete: bool,
    /// Caller supplied timestamp.
    pub at: f64,
    /// Whether the mind used a fallback.
    pub fallback: bool,
    /// Whether a player interrupted the turn.
    pub interrupted: bool,
}

/// Failure while operating or restoring a dialogue session.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DialogueError {
    /// The session is closed.
    #[error("dialogue session is closed")]
    Closed,
    /// Saved session data is invalid.
    #[error("bad dialogue save: {0}")]
    BadSave(String),
}

/// A stable notification emitted by a dialogue session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SessionEvent {
    /// A new transcript turn began.
    TurnStarted {
        /// Identifier of the new turn.
        id: u64,
        /// Author of the new turn.
        speaker: Speaker,
    },
    /// Text was appended to a streaming turn.
    TextDelta {
        /// Identifier of the streaming turn.
        id: u64,
        /// Newly appended text.
        delta: String,
    },
    /// Final text replaced streamed text.
    TextReplaced {
        /// Identifier of the completed turn.
        id: u64,
        /// Authoritative final text.
        text: String,
    },
    /// A transcript turn ended.
    TurnCompleted {
        /// Identifier of the completed turn.
        id: u64,
    },
    /// Session activity changed.
    StateChanged(SessionState),
    /// Suggested player replies changed.
    SuggestionsChanged(Vec<String>),
    /// An emote was requested.
    Emote {
        /// Requested emote name.
        name: String,
    },
    /// An intent should be handled by the game.
    Intent(Intent),
    /// A short latency-covering line that is not part of the transcript.
    Filler {
        /// The temporary line to show while the NPC is thinking.
        text: String,
    },
    /// The session closed.
    Closed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Save {
    npc: String,
    state: SessionState,
    transcript: Vec<Turn>,
    suggestions: Vec<String>,
    next_id: u64,
    trimmed: usize,
    capacity: usize,
}

/// Deterministic dialogue model with bounded transcript and ordered events.
pub struct DialogueSession {
    npc: String,
    state: SessionState,
    transcript: Vec<Turn>,
    suggestions: Vec<String>,
    next_id: u64,
    trimmed: usize,
    capacity: usize,
    events: Vec<SessionEvent>,
}

impl DialogueSession {
    /// Creates an empty session with a 200-turn transcript capacity.
    #[must_use]
    pub fn new(npc: &str, now: f64) -> Self {
        Self::with_capacity(npc, now, 200)
    }

    /// Creates an empty session with the requested transcript capacity.
    #[must_use]
    pub fn with_capacity(npc: &str, _now: f64, capacity_turns: usize) -> Self {
        Self {
            npc: npc.into(),
            state: SessionState::Idle,
            transcript: Vec::new(),
            suggestions: Vec::new(),
            next_id: 1,
            trimmed: 0,
            capacity: capacity_turns,
            events: Vec::new(),
        }
    }
    /// Returns the NPC name.
    #[must_use]
    pub fn npc(&self) -> &str {
        &self.npc
    }
    /// Returns current activity.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state
    }
    /// Returns retained turns in chronological order.
    #[must_use]
    pub fn transcript(&self) -> &[Turn] {
        &self.transcript
    }
    /// Returns current suggested replies.
    #[must_use]
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }
    /// Returns the number of turns dropped due to capacity.
    #[must_use]
    pub fn trimmed(&self) -> usize {
        self.trimmed
    }
    /// Removes and returns pending events in emission order.
    pub fn drain_events(&mut self) -> Vec<SessionEvent> {
        std::mem::take(&mut self.events)
    }

    /// Returns newest complete turns, oldest first, in provider conversation form.
    #[must_use]
    pub fn conversation(&self, max_turns: usize) -> Vec<(String, String)> {
        self.transcript
            .iter()
            .filter(|t| t.complete)
            .rev()
            .take(max_turns)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|t| {
                let speaker = match &t.speaker {
                    Speaker::Player => "player".to_owned(),
                    Speaker::Npc(name) => name.clone(),
                };
                (speaker, t.text.clone())
            })
            .collect()
    }
    /// Serializes stable session data, excluding pending events.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&Save {
            npc: self.npc.clone(),
            state: self.state,
            transcript: self.transcript.clone(),
            suggestions: self.suggestions.clone(),
            next_id: self.next_id,
            trimmed: self.trimmed,
            capacity: self.capacity,
        })
        .unwrap_or_default()
    }
    /// Restores session data from JSON with an empty event queue.
    ///
    /// # Errors
    /// Returns [`DialogueError::BadSave`] when JSON or its counters are invalid.
    pub fn from_json(json: &str) -> Result<Self, DialogueError> {
        let save: Save =
            serde_json::from_str(json).map_err(|e| DialogueError::BadSave(e.to_string()))?;
        if save.transcript.len() > save.capacity
            || save.transcript.iter().any(|t| t.id >= save.next_id)
        {
            return Err(DialogueError::BadSave(
                "inconsistent transcript counters".into(),
            ));
        }
        Ok(Self {
            npc: save.npc,
            state: save.state,
            transcript: save.transcript,
            suggestions: save.suggestions,
            next_id: save.next_id,
            trimmed: save.trimmed,
            capacity: save.capacity,
            events: Vec::new(),
        })
    }

    /// Adds a completed player turn and transitions to thinking.
    ///
    /// # Errors
    /// Returns [`DialogueError::Closed`] if the session has ended.
    pub fn player_says(&mut self, text: &str, now: f64) -> Result<(), DialogueError> {
        self.ensure_open()?;
        if let Some(i) = self.streaming_index() {
            self.transcript[i].complete = true;
            self.transcript[i].interrupted = true;
            self.events.push(SessionEvent::TurnCompleted {
                id: self.transcript[i].id,
            });
        }
        let id = self.add_turn(Speaker::Player, text, true, now, false);
        self.events.push(SessionEvent::TurnStarted {
            id,
            speaker: Speaker::Player,
        });
        self.events.push(SessionEvent::TurnCompleted { id });
        self.set_state(SessionState::Thinking);
        Ok(())
    }
    /// Appends a chunk to the active NPC stream, starting a stream when needed.
    ///
    /// # Errors
    /// Returns [`DialogueError::Closed`] if the session has ended.
    pub fn npc_delta(&mut self, delta: &str, now: f64) -> Result<(), DialogueError> {
        self.ensure_open()?;
        if self.streaming_index().is_none() {
            self.start_stream(now);
        }
        if let Some(i) = self.streaming_index() {
            self.transcript[i].text.push_str(delta);
            let id = self.transcript[i].id;
            self.events.push(SessionEvent::TextDelta {
                id,
                delta: delta.into(),
            });
        }
        Ok(())
    }
    /// Completes the active or a new NPC turn with validated final text.
    ///
    /// # Errors
    /// Returns [`DialogueError::Closed`] if the session has ended.
    pub fn npc_complete(
        &mut self,
        final_text: &str,
        suggestions: Vec<String>,
        now: f64,
    ) -> Result<(), DialogueError> {
        self.ensure_open()?;
        let (id, index) = if let Some(i) = self.streaming_index() {
            (self.transcript[i].id, Some(i))
        } else {
            let id = self.add_turn(
                Speaker::Npc(self.npc.clone()),
                final_text,
                false,
                now,
                false,
            );
            self.events.push(SessionEvent::TurnStarted {
                id,
                speaker: Speaker::Npc(self.npc.clone()),
            });
            self.set_state(SessionState::Speaking);
            (id, self.transcript.iter().position(|turn| turn.id == id))
        };
        if let Some(i) = index {
            if self.transcript[i].text != final_text {
                self.transcript[i].text = final_text.into();
                self.events.push(SessionEvent::TextReplaced {
                    id,
                    text: final_text.into(),
                });
            }
            self.transcript[i].complete = true;
        }
        self.events.push(SessionEvent::TurnCompleted { id });
        self.set_state(SessionState::Idle);
        if self.suggestions != suggestions {
            self.suggestions.clone_from(&suggestions);
            self.events
                .push(SessionEvent::SuggestionsChanged(suggestions));
        }
        Ok(())
    }
    /// Applies mind intents as dialogue, emote, and game-facing events.
    ///
    /// # Errors
    /// Returns [`DialogueError::Closed`] if the session has ended.
    pub fn apply_delivery(&mut self, delivery: &Delivery, now: f64) -> Result<(), DialogueError> {
        self.ensure_open()?;
        let mut lines = Vec::new();
        for intent in &delivery.intents {
            if intent.name == "say"
                && let Some(ArgValue::Text(text)) = intent.args.get("text")
            {
                lines.push(text.clone());
            }
        }
        if lines.is_empty() {
            self.set_state(SessionState::Idle);
        } else {
            self.npc_complete(&lines.join("\n"), Vec::new(), now)?;
            if let Some(turn) = self.transcript.last_mut() {
                turn.fallback = delivery.fallback;
            }
        }
        for intent in &delivery.intents {
            match intent.name.as_str() {
                "say" => {}
                "emote" => {
                    if let Some(ArgValue::Text(name)) = intent.args.get("name") {
                        self.events.push(SessionEvent::Emote { name: name.clone() });
                    }
                }
                _ => self.events.push(SessionEvent::Intent(intent.clone())),
            }
        }
        Ok(())
    }
    /// Starts or stops listening without interrupting thinking or speaking.
    ///
    /// # Errors
    /// Returns [`DialogueError::Closed`] if the session has ended.
    pub fn set_listening(&mut self, on: bool) -> Result<(), DialogueError> {
        self.ensure_open()?;
        match (on, self.state) {
            (true, SessionState::Idle) => self.set_state(SessionState::Listening),
            (false, SessionState::Listening) => self.set_state(SessionState::Idle),
            _ => {}
        }
        Ok(())
    }
    /// Closes the session once.
    pub fn close(&mut self, _now: f64) {
        if self.state != SessionState::Closed {
            self.set_state(SessionState::Closed);
            self.events.push(SessionEvent::Closed);
        }
    }

    fn ensure_open(&self) -> Result<(), DialogueError> {
        if self.state == SessionState::Closed {
            Err(DialogueError::Closed)
        } else {
            Ok(())
        }
    }
    fn set_state(&mut self, state: SessionState) {
        if self.state != state {
            self.state = state;
            self.events.push(SessionEvent::StateChanged(state));
        }
    }
    fn streaming_index(&self) -> Option<usize> {
        self.transcript
            .iter()
            .rposition(|t| matches!(t.speaker, Speaker::Npc(_)) && !t.complete)
    }
    fn start_stream(&mut self, now: f64) {
        let id = self.add_turn(Speaker::Npc(self.npc.clone()), "", false, now, false);
        self.events.push(SessionEvent::TurnStarted {
            id,
            speaker: Speaker::Npc(self.npc.clone()),
        });
        self.set_state(SessionState::Speaking);
    }
    fn add_turn(
        &mut self,
        speaker: Speaker,
        text: &str,
        complete: bool,
        now: f64,
        fallback: bool,
    ) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.transcript.push(Turn {
            id,
            speaker,
            text: text.into(),
            complete,
            at: now,
            fallback,
            interrupted: false,
        });
        while self.transcript.len() > self.capacity {
            self.transcript.remove(0);
            self.trimmed = self.trimmed.saturating_add(1);
        }
        id
    }
}
