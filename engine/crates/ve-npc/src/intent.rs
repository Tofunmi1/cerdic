//! Intent vocabulary and validation at the language-model boundary.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

/// Type and validation rule for one intent argument.
#[derive(Clone, Debug)]
pub enum ArgType {
    /// Finite number clamped to an inclusive range.
    Number {
        /// Inclusive minimum.
        min: f64,
        /// Inclusive maximum.
        max: f64,
    },
    /// UTF-8 text limited by Unicode scalar count.
    Text {
        /// Maximum number of characters.
        max_len: usize,
    },
    /// One of the listed exact strings.
    Choice(Vec<String>),
    /// Boolean value.
    Bool,
}

/// Typed value accepted for an intent argument.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ArgValue {
    /// Text value.
    Text(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Bool(bool),
}

/// One registered intent and its ordered argument schema.
#[derive(Clone, Debug)]
pub struct IntentSpec {
    name: String,
    description: String,
    args: Vec<(String, ArgType)>,
}
impl IntentSpec {
    /// Creates an intent specification.
    #[must_use]
    pub fn new(name: impl Into<String>, doc: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: doc.into(),
            args: Vec::new(),
        }
    }
    /// Adds a required argument, preserving its registration order.
    #[must_use]
    pub fn arg(mut self, name: impl Into<String>, kind: ArgType) -> Self {
        self.args.push((name.into(), kind));
        self
    }
}

/// Error returned when an intent specification cannot be registered.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum IntentError {
    /// Invalid intent name.
    #[error("bad intent name: {0}")]
    BadName(String),
    /// Duplicate intent name.
    #[error("duplicate intent: {0}")]
    Duplicate(String),
}

/// One validated intent returned by the parser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    /// Registered name.
    pub name: String,
    /// Validated arguments.
    pub args: BTreeMap<String, ArgValue>,
}
/// Explanation for one dropped intent or malformed response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedIntent {
    /// Original intent JSON, if available.
    pub raw: String,
    /// Reason for rejection.
    pub reason: String,
}
/// Valid intents and per-item rejection explanations.
#[derive(Clone, Debug, Default)]
pub struct ParsedReply {
    /// Valid intents in input order.
    pub intents: Vec<Intent>,
    /// Dropped items and malformed reply diagnostics.
    pub rejected: Vec<RejectedIntent>,
}

/// Closed, registration-ordered set of allowed intents.
#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    specs: Vec<IntentSpec>,
}
impl Vocabulary {
    /// Creates an empty vocabulary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Creates a vocabulary containing standard dialogue and memory intents.
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut v = Self::new();
        for s in [
            IntentSpec::new("say", "Say something to the player.")
                .arg("text", ArgType::Text { max_len: 600 }),
            IntentSpec::new("remember", "Add a memory.")
                .arg("text", ArgType::Text { max_len: 500 })
                .arg("importance", ArgType::Number { min: 0.0, max: 1.0 }),
            IntentSpec::new("reflect", "Record a lasting conclusion.")
                .arg("text", ArgType::Text { max_len: 300 }),
            IntentSpec::new("set_fact", "Set a fact.")
                .arg("key", ArgType::Text { max_len: 80 })
                .arg("value", ArgType::Text { max_len: 200 }),
            IntentSpec::new("adjust_relationship", "Adjust a relationship.")
                .arg("other", ArgType::Text { max_len: 80 })
                .arg(
                    "affinity",
                    ArgType::Number {
                        min: -1.0,
                        max: 1.0,
                    },
                )
                .arg(
                    "trust",
                    ArgType::Number {
                        min: -1.0,
                        max: 1.0,
                    },
                ),
            IntentSpec::new("emote", "Show an emotion.").arg("name", ArgType::Text { max_len: 40 }),
        ] {
            let _ = v.register(s);
        }
        v
    }
    /// Registers a specification.
    ///
    /// # Errors
    /// Returns [`IntentError::BadName`] for invalid names and [`IntentError::Duplicate`] when
    /// the name is already registered.
    pub fn register(&mut self, spec: IntentSpec) -> Result<(), IntentError> {
        let bytes = spec.name.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 40
            || !bytes[0].is_ascii_lowercase()
            || !bytes
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
        {
            return Err(IntentError::BadName(spec.name));
        }
        if self.get(&spec.name).is_some() {
            return Err(IntentError::Duplicate(spec.name));
        }
        self.specs.push(spec);
        Ok(())
    }
    /// Looks up an intent by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&IntentSpec> {
        self.specs.iter().find(|s| s.name == name)
    }
    /// Returns deterministic one-line-per-intent schema text.
    #[must_use]
    pub fn describe(&self) -> String {
        self.specs
            .iter()
            .map(|s| {
                let args = s
                    .args
                    .iter()
                    .map(|(n, t)| format!("{n}: {}", describe_type(t)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({}) - {}", s.name, args, s.description)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
fn describe_type(t: &ArgType) -> String {
    match t {
        ArgType::Number { min, max } => format!("number {min}..{max}"),
        ArgType::Text { max_len } => format!("text (max {max_len} chars)"),
        ArgType::Choice(v) => format!("one of: {}", v.join(", ")),
        ArgType::Bool => "true or false".into(),
    }
}

/// Finds the first balanced JSON object and validates its intents.
#[must_use]
pub fn parse_reply(reply: &str, vocab: &Vocabulary) -> ParsedReply {
    let Some(raw) = extract_json(reply) else {
        return malformed("no JSON object found");
    };
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return malformed("invalid JSON object");
    };
    let Some(items) = value.get("intents").and_then(Value::as_array) else {
        return malformed("JSON has no intents array");
    };
    let mut out = ParsedReply::default();
    for (i, item) in items.iter().enumerate() {
        if i >= 8 {
            out.rejected.push(RejectedIntent {
                raw: item.to_string(),
                reason: "intent limit exceeded".into(),
            });
            continue;
        }
        match validate(item, vocab) {
            Ok(v) => out.intents.push(v),
            Err(reason) => out.rejected.push(RejectedIntent {
                raw: item.to_string(),
                reason,
            }),
        }
    }
    out
}
fn malformed(reason: &str) -> ParsedReply {
    ParsedReply {
        intents: vec![],
        rejected: vec![RejectedIntent {
            raw: String::new(),
            reason: reason.into(),
        }],
    }
}
fn validate(v: &Value, vocab: &Vocabulary) -> Result<Intent, String> {
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .ok_or("intent name is missing")?;
    let spec = vocab
        .get(name)
        .ok_or_else(|| format!("unknown intent {name}"))?;
    let args = v.get("args").and_then(Value::as_object);
    let mut result = BTreeMap::new();
    for (n, t) in &spec.args {
        let value = args
            .and_then(|a| a.get(n))
            .ok_or_else(|| format!("missing argument {n}"))?;
        let parsed = match t {
            ArgType::Text { max_len } => ArgValue::Text(
                value
                    .as_str()
                    .ok_or_else(|| format!("argument {n} must be text"))?
                    .chars()
                    .take(*max_len)
                    .collect(),
            ),
            ArgType::Number { min, max } => ArgValue::Number(
                value
                    .as_f64()
                    .filter(|x| x.is_finite())
                    .ok_or_else(|| format!("argument {n} must be a number"))?
                    .clamp(*min, *max),
            ),
            ArgType::Choice(choices) => {
                let s = value
                    .as_str()
                    .ok_or_else(|| format!("argument {n} must be a choice"))?;
                if !choices.iter().any(|x| x == s) {
                    return Err(format!("argument {n} is not an allowed choice"));
                }
                ArgValue::Text(s.into())
            }
            ArgType::Bool => ArgValue::Bool(
                value
                    .as_bool()
                    .ok_or_else(|| format!("argument {n} must be a boolean"))?,
            ),
        };
        result.insert(n.clone(), parsed);
    }
    Ok(Intent {
        name: name.into(),
        args: result,
    })
}
fn extract_json(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let mut depth = 0usize;
    let mut quote = false;
    let mut escape = false;
    for (offset, c) in s[start..].char_indices() {
        if quote {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quote = false;
            }
        } else if c == '"' {
            quote = true;
        } else if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(&s[start..=start + offset]);
            }
        }
    }
    None
}
