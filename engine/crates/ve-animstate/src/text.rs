//! Text form for authoring and inspecting animation state machines.

use std::collections::BTreeMap;
use std::fmt;

use crate::{
    BlendPoint, Condition, Error, MAX_STATES, MAX_TRANSITIONS, Motion, State, StateMachine,
    Transition,
};

/// A text error, with a one-based source line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// One-based line on which the problem occurred.
    pub line: usize,
    /// Description of the problem.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

fn error(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: message.into(),
    }
}

impl StateMachine {
    /// Parses the line-oriented state machine format.
    ///
    /// # Errors
    /// Returns the source line and reason for malformed text or an invalid graph.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut states = Vec::new();
        let mut transitions = Vec::new();
        let mut entry: Option<(String, usize)> = None;
        let mut state_lines = BTreeMap::new();
        let mut transition_lines = Vec::new();
        let mut extras = crate::layers::Extras::default();
        let mut last_line = 0;
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            last_line = line;
            let body = raw.split('#').next().unwrap_or("").trim();
            if body.is_empty() {
                continue;
            }
            // Layers and events (a wave over a walk, footsteps) have their own statements.
            let words: Vec<&str> = body.split_whitespace().collect();
            match words.first().copied() {
                Some("layer") => {
                    crate::text_extras::parse_layer(&words[1..], &mut extras).map_err(|m| error(line, m))?;
                    continue;
                }
                Some("event") => {
                    crate::text_extras::parse_event(&words[1..], &mut extras).map_err(|m| error(line, m))?;
                    continue;
                }
                _ => {}
            }
            parse_statement(
                body,
                line,
                &mut entry,
                &mut states,
                &mut state_lines,
                &mut transitions,
                &mut transition_lines,
            )?;
        }
        if states.is_empty() {
            return Err(error(
                last_line.max(1),
                "no states; add a `state NAME ...` statement",
            ));
        }
        let Some((entry_name, entry_line)) = entry else {
            return Err(error(last_line.max(1), "no entry; add `entry NAME`"));
        };
        if !state_lines.contains_key(&entry_name) {
            return Err(error(
                entry_line,
                format!("entry state {entry_name:?} does not exist"),
            ));
        }
        for (i, transition) in transitions.iter().enumerate() {
            if let Some(from) = &transition.from
                && !state_lines.contains_key(from)
            {
                return Err(error(
                    transition_lines[i],
                    format!("unknown source state {from:?}"),
                ));
            }
            if !state_lines.contains_key(&transition.to) {
                return Err(error(
                    transition_lines[i],
                    format!("unknown target state {:?}", transition.to),
                ));
            }
        }
        let machine = Self {
            states,
            transitions,
            entry: entry_name,
            extras,
        };
        if let Err(problem) = machine.validate() {
            let line = validation_line(
                &problem,
                &machine,
                &state_lines,
                &transition_lines,
                entry_line,
            );
            return Err(error(line, problem.to_string()));
        }
        Ok(machine)
    }

    /// Writes a deterministic, round-trippable text representation.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = format!("entry {}\n", self.entry);
        for state in &self.states {
            out.push_str("state ");
            out.push_str(&state.name);
            match &state.motion {
                Motion::Clip {
                    clip,
                    speed,
                    looping,
                } => {
                    out.push_str(" clip=");
                    out.push_str(&clip_token(clip));
                    if speed.to_bits() != 1.0_f32.to_bits() {
                        out.push_str(" speed=");
                        out.push_str(&speed.to_string());
                    }
                    if !looping {
                        out.push_str(" once");
                    }
                }
                Motion::Blend1D { parameter, points } => {
                    out.push_str(" blend=");
                    out.push_str(parameter);
                    for point in points {
                        out.push(' ');
                        out.push_str(&clip_token(&point.clip));
                        out.push('@');
                        out.push_str(&point.value.to_string());
                    }
                }
            }
            out.push('\n');
        }
        for transition in &self.transitions {
            out.push_str("transition ");
            out.push_str(transition.from.as_deref().unwrap_or("any"));
            out.push_str(" -> ");
            out.push_str(&transition.to);
            if !transition.conditions.is_empty() {
                out.push_str(" when ");
                for (i, condition) in transition.conditions.iter().enumerate() {
                    if i > 0 {
                        out.push_str(" & ");
                    }
                    write_condition(&mut out, condition);
                }
            }
            if transition.duration.to_bits() != 0.15_f32.to_bits() {
                out.push_str(" fade=");
                out.push_str(&transition.duration.to_string());
            }
            if let Some(exit) = transition.exit_time {
                out.push_str(" exit=");
                out.push_str(&exit.to_string());
            }
            out.push('\n');
        }
        crate::text_extras::write(&self.extras, &mut out);
        out
    }
}

fn parse_statement(
    body: &str,
    line: usize,
    entry: &mut Option<(String, usize)>,
    states: &mut Vec<State>,
    state_lines: &mut BTreeMap<String, usize>,
    transitions: &mut Vec<Transition>,
    transition_lines: &mut Vec<usize>,
) -> Result<(), ParseError> {
    let words: Vec<&str> = body.split_whitespace().collect();
    match words.first().copied().unwrap_or("") {
        "entry" => {
            if words.len() != 2 || !valid_name(words[1]) {
                return Err(error(line, "write `entry NAME` using one name"));
            }
            if entry.is_some() {
                return Err(error(
                    line,
                    "more than one entry; keep exactly one `entry NAME`",
                ));
            }
            *entry = Some((words[1].to_owned(), line));
        }
        "state" => {
            if words.len() < 2 || !valid_name(words[1]) {
                return Err(error(line, "write `state NAME` using one name"));
            }
            let name = words[1];
            if state_lines.contains_key(name) {
                return Err(error(line, format!("repeated state {name:?}")));
            }
            if states.len() >= MAX_STATES {
                return Err(error(line, format!("more than {MAX_STATES} states")));
            }
            let motion = parse_motion(&words[2..]).map_err(|m| error(line, m))?;
            state_lines.insert(name.to_owned(), line);
            states.push(State {
                name: name.to_owned(),
                motion,
            });
        }
        "transition" => {
            if transitions.len() >= MAX_TRANSITIONS {
                return Err(error(
                    line,
                    format!("more than {MAX_TRANSITIONS} transitions"),
                ));
            }
            transitions.push(parse_transition(&words[1..]).map_err(|m| error(line, m))?);
            transition_lines.push(line);
        }
        keyword => {
            return Err(error(
                line,
                format!("unknown statement {keyword:?}; use entry, state or transition"),
            ));
        }
    }
    Ok(())
}

fn validation_line(
    error: &Error,
    machine: &StateMachine,
    states: &BTreeMap<String, usize>,
    transitions: &[usize],
    fallback: usize,
) -> usize {
    match error {
        Error::State { state, .. } => states.get(state).copied().unwrap_or(fallback),
        Error::Transition { index, .. } => transitions.get(*index).copied().unwrap_or(fallback),
        Error::Machine { reason } if reason.contains("entry") => {
            states.get(&machine.entry).copied().unwrap_or(fallback)
        }
        _ => fallback,
    }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(char::is_whitespace) && s != "->"
}
fn parse_float(s: &str) -> Result<f32, String> {
    let value: f32 = s.parse().map_err(|_| format!("bad number {s:?}"))?;
    if !value.is_finite() {
        return Err(format!("bad number {s:?}; use a finite float"));
    }
    Ok(value)
}
fn clip_token(s: &str) -> String {
    if s.is_empty() {
        "\"\"".to_owned()
    } else {
        s.to_owned()
    }
}

fn parse_motion(words: &[&str]) -> Result<Motion, String> {
    let Some(first) = words.first() else {
        return Err(
            "state plays nothing; add `clip=NAME` or `blend=PARAM POINT@VALUE...`".to_owned(),
        );
    };
    if let Some(parameter) = first.strip_prefix("blend=") {
        if !valid_name(parameter) {
            return Err("bad blend parameter; write `blend=PARAM`".to_owned());
        }
        if words.len() == 1 {
            return Err("blend has no points; add `CLIP@VALUE`".to_owned());
        }
        let mut points = Vec::new();
        for word in &words[1..] {
            let Some((clip, value)) = word.rsplit_once('@') else {
                return Err(format!(
                    "blend point {word:?} has no value; write `CLIP@VALUE`"
                ));
            };
            let clip = parse_clip(clip)?;
            points.push(BlendPoint {
                value: parse_float(value)?,
                clip,
            });
        }
        return Ok(Motion::Blend1D {
            parameter: parameter.to_owned(),
            points,
        });
    }
    let mut clip = None;
    let mut speed = 1.0;
    let mut looping = true;
    for word in words {
        if let Some(value) = word.strip_prefix("clip=") {
            if clip.is_some() {
                return Err("repeated clip= option".to_owned());
            }
            clip = Some(parse_clip(value)?);
        } else if let Some(value) = word.strip_prefix("speed=") {
            speed = parse_float(value)?;
        } else if *word == "once" {
            looping = false;
        } else if *word == "loop" {
            looping = true;
        } else {
            return Err(format!("unknown state option {word:?}"));
        }
    }
    Ok(Motion::Clip {
        clip: clip.ok_or_else(|| "state plays nothing; add `clip=NAME`".to_owned())?,
        speed,
        looping,
    })
}
fn parse_clip(s: &str) -> Result<String, String> {
    if s == "\"\"" {
        return Ok(String::new());
    }
    if !valid_name(s) {
        return Err(format!("bad clip name {s:?}; use a name or `\"\"`"));
    }
    Ok(s.to_owned())
}

fn parse_transition(words: &[&str]) -> Result<Transition, String> {
    if words.len() < 3 || words[1] != "->" {
        return Err("write `transition FROM -> TO`".to_owned());
    }
    let from = if words[0] == "any" {
        None
    } else if valid_name(words[0]) {
        Some(words[0].to_owned())
    } else {
        return Err(format!("bad source state {:?}", words[0]));
    };
    if !valid_name(words[2]) {
        return Err(format!("bad target state {:?}", words[2]));
    }
    let mut duration = 0.15;
    let mut exit_time = None;
    let mut conditions = Vec::new();
    let mut i = 3;
    while i < words.len() {
        if let Some(v) = words[i].strip_prefix("fade=") {
            duration = parse_float(v)?;
            i += 1;
        } else if let Some(v) = words[i].strip_prefix("exit=") {
            exit_time = Some(parse_float(v)?);
            i += 1;
        } else if words[i] == "when" {
            i += 1;
            let mut cond_tokens = Vec::new();
            while i < words.len()
                && !words[i].starts_with("fade=")
                && !words[i].starts_with("exit=")
            {
                cond_tokens.push(words[i]);
                i += 1;
            }
            let joined = cond_tokens.join("");
            for part in joined.split('&') {
                if part.is_empty() {
                    return Err("bad condition; write `when PARAM>VALUE`, `PARAM<VALUE`, `PARAM=true` or `trigger:NAME`".to_owned());
                }
                conditions.push(parse_condition(part)?);
            }
        } else {
            return Err(format!("unknown transition token {:?}", words[i]));
        }
    }
    Ok(Transition {
        from,
        to: words[2].to_owned(),
        conditions,
        duration,
        exit_time,
    })
}

fn parse_condition(s: &str) -> Result<Condition, String> {
    if let Some(name) = s.strip_prefix("trigger:")
        && valid_name(name)
    {
        return Ok(Condition::Trigger {
            parameter: name.to_owned(),
        });
    }
    if let Some((name, value)) = s.split_once('>')
        && valid_name(name)
        && !value.contains('>')
    {
        return Ok(Condition::Greater {
            parameter: name.to_owned(),
            value: parse_float(value)?,
        });
    }
    if let Some((name, value)) = s.split_once('<')
        && valid_name(name)
        && !value.contains('<')
    {
        return Ok(Condition::Less {
            parameter: name.to_owned(),
            value: parse_float(value)?,
        });
    }
    if let Some((name, value)) = s.split_once('=')
        && valid_name(name)
        && !value.contains('=')
    {
        return match value {
            "true" => Ok(Condition::Equals {
                parameter: name.to_owned(),
                value: true,
            }),
            "false" => Ok(Condition::Equals {
                parameter: name.to_owned(),
                value: false,
            }),
            _ => Err(format!(
                "bad condition {s:?}; bool values are true or false"
            )),
        };
    }
    Err(format!(
        "bad condition {s:?}; use PARAM>VALUE, PARAM<VALUE, PARAM=true or trigger:NAME"
    ))
}

fn write_condition(out: &mut String, condition: &Condition) {
    match condition {
        Condition::Greater { parameter, value } => {
            out.push_str(parameter);
            out.push('>');
            out.push_str(&value.to_string());
        }
        Condition::Less { parameter, value } => {
            out.push_str(parameter);
            out.push('<');
            out.push_str(&value.to_string());
        }
        Condition::Equals { parameter, value } => {
            out.push_str(parameter);
            out.push('=');
            out.push_str(if *value { "true" } else { "false" });
        }
        Condition::Trigger { parameter } => {
            out.push_str("trigger:");
            out.push_str(parameter);
        }
        Condition::And(inner) => {
            for (i, item) in inner.iter().enumerate() {
                if i > 0 {
                    out.push('&');
                }
                write_condition(out, item);
            }
        }
    }
}
