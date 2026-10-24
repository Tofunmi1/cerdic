//! Character authoring files, dialogue scripts, and deterministic reports.
use crate::{FallbackTable, FillerSpec, HostError, NpcHost, NpcSpec, Provider, Reason};
use serde::{Deserialize, Serialize};

/// A canned line for one scheduler reason.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FallbackLine {
    /// Scheduler reason name.
    pub reason: String,
    /// Spoken fallback text.
    pub text: String,
}
/// Complete authoring data for one NPC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Character {
    /// Identity, persona, goals, rules, and granted knowledge.
    pub npc: NpcSpec,
    /// Trust gated private facts.
    #[serde(default)]
    pub secrets: Vec<crate::SecretSpec>,
    /// Worked dialogue examples.
    #[serde(default)]
    pub examples: Vec<(String, String)>,
    /// Leak deflections.
    #[serde(default)]
    pub deflections: Vec<String>,
    /// Pending response filler.
    #[serde(default)]
    pub filler: Option<FillerSpec>,
    /// Canned responses in file order.
    #[serde(default)]
    pub fallback: Vec<FallbackLine>,
}
impl Character {
    /// Reads and validates a character JSON document.
    ///
    /// # Errors
    /// Returns `BadCommand` for malformed JSON or an unknown fallback reason.
    pub fn from_json(json: &str) -> Result<Self, HostError> {
        let value: Self =
            serde_json::from_str(json).map_err(|e| HostError::BadCommand(e.to_string()))?;
        for line in &value.fallback {
            reason(&line.reason)?;
        }
        Ok(value)
    }
    /// Serializes this character as stable JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    /// Registers this character and applies all authoring data.
    ///
    /// # Errors
    /// Returns a host error if registration or any authoring operation fails.
    pub fn apply(&self, host: &mut NpcHost) -> Result<(), HostError> {
        host.register(self.npc.clone())?;
        for secret in &self.secrets {
            host.add_secret(&self.npc.name, secret.clone())?;
        }
        host.set_examples(&self.npc.name, self.examples.clone())?;
        if !self.deflections.is_empty() {
            host.set_deflections(&self.npc.name, self.deflections.clone());
        }
        if let Some(filler) = &self.filler {
            host.set_filler(&self.npc.name, filler.clone());
        }
        let mut table = FallbackTable::new();
        for line in &self.fallback {
            table = table.line(reason(&line.reason)?, line.text.clone());
        }
        host.set_fallback(&self.npc.name, table);
        Ok(())
    }
    /// Returns the built in Anna the baker character.
    #[must_use]
    pub fn example_baker() -> Self {
        Self {
            npc: NpcSpec {
                name: "anna".into(),
                persona: "You are Anna, a warm and brisk village baker, proud of your rye bread."
                    .into(),
                rules: "Stay in character. Speak warmly and briskly.".into(),
                goals: vec!["Sell the bread before noon.".into()],
                world_facts: vec![
                    "The mill is by the river.".into(),
                    "The smithy is beside the square.".into(),
                    "The well is in the village square.".into(),
                    "Rye bread costs 2 coins.".into(),
                ],
            },
            secrets: vec![crate::SecretSpec {
                text: "You owe the miller 40 coins.".into(),
                min_trust: 0.5,
                markers: vec![
                    "owe the miller".into(),
                    "40 coins".into(),
                    "forty coins".into(),
                ],
            }],
            examples: vec![
                ("What is fresh?".into(), "Rye, warm from the oven.".into()),
                (
                    "How much is a loaf?".into(),
                    "Two coins, and worth every crumb.".into(),
                ),
                (
                    "Busy morning?".into(),
                    "A good one. The dough waits for no one!".into(),
                ),
            ],
            deflections: vec![
                "Let's talk about bread instead.".into(),
                "Have you tried today's rye?".into(),
            ],
            filler: Some(FillerSpec {
                after_secs: 1.2,
                lines: vec!["Hmm...".into(), "Let me see...".into()],
            }),
            fallback: vec![
                FallbackLine {
                    reason: "player_spoke".into(),
                    text: "Hm? Say that again, dear.".into(),
                },
                FallbackLine {
                    reason: "idle".into(),
                    text: "Fresh rye, two coins a loaf!".into(),
                },
            ],
        }
    }
}
fn reason(name: &str) -> Result<Reason, HostError> {
    match name {
        "player_spoke" => Ok(Reason::PlayerSpoke),
        "event" => Ok(Reason::Event),
        "brain_request" => Ok(Reason::BrainRequest),
        "reflection" => Ok(Reason::Reflection),
        "idle" => Ok(Reason::Idle),
        _ => Err(HostError::BadCommand(format!(
            "unknown fallback reason: {name}"
        ))),
    }
}

/// A reply condition for a script line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Expect {
    /// At least one required phrase.
    #[serde(default)]
    pub contains_any: Vec<String>,
    /// Forbidden phrases.
    #[serde(default)]
    pub excludes: Vec<String>,
}
/// One player line and optional reply expectations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScriptLine {
    /// Text spoken by the simulated player.
    pub say: String,
    /// Assertions about the NPC reply.
    #[serde(default)]
    pub expect: Option<Expect>,
}
/// Ordered NPC dialogue script.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Script {
    /// Player turns.
    pub lines: Vec<ScriptLine>,
}
impl Script {
    /// Reads a script JSON document.
    ///
    /// # Errors
    /// Returns `BadCommand` when the JSON does not match the script schema.
    pub fn from_json(json: &str) -> Result<Self, HostError> {
        serde_json::from_str(json).map_err(|e| HostError::BadCommand(e.to_string()))
    }
}
/// One named script assertion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    /// Assertion name.
    pub name: String,
    /// Whether the assertion passed.
    pub passed: bool,
    /// Expected phrase(s) or observed failure.
    pub detail: String,
}
/// Outcome for one simulated turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnReport {
    /// Player text.
    pub player: String,
    /// Last NPC response text.
    pub reply: String,
    /// Whether the response used fallback.
    pub fallback: bool,
    /// Valid intents in final delivery.
    pub valid_intents: usize,
    /// Rejected intents in final delivery.
    pub rejected: usize,
    /// Whether a retry was admitted.
    pub retried: bool,
    /// Whether a secret leak was blocked.
    pub leak_blocked: bool,
    /// Elapsed injected milliseconds.
    pub millis: u64,
    /// Script assertions.
    pub checks: Vec<CheckResult>,
}
/// Aggregate script statistics.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Completed script lines.
    pub turns: usize,
    /// Share of turns with valid non-fallback intents.
    pub valid_rate: f64,
    /// Share of turns using fallback.
    pub fallback_rate: f64,
    /// Total blocked secret leaks.
    pub leaks_blocked: u32,
    /// Total repair retries.
    pub retries: u32,
    /// Average elapsed milliseconds.
    pub mean_ms: f64,
    /// Nearest-rank median.
    pub median_ms: u64,
    /// Nearest-rank 95th percentile.
    pub p95_ms: u64,
    /// Passed assertions.
    pub checks_passed: usize,
    /// Total assertions.
    pub checks_total: usize,
}
/// Complete script result.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Per-turn data.
    pub turns: Vec<TurnReport>,
    /// Aggregate statistics.
    pub summary: Summary,
    /// Failure such as an unknown NPC.
    pub error: Option<String>,
}
impl Report {
    /// Serializes the report as stable JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Runs a dialogue script using an injected millisecond clock.
pub fn run_script(
    host: &mut NpcHost,
    npc: &str,
    script: &Script,
    provider: &mut dyn Provider,
    clock: &mut dyn FnMut() -> u64,
) -> Report {
    if host.session(npc).is_none() {
        return Report {
            error: Some(format!("unknown npc: {npc}")),
            ..Report::default()
        };
    }
    let turns = script
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| run_line(host, npc, line, index, provider, clock))
        .collect();
    summarize(turns)
}
fn run_line(
    host: &mut NpcHost,
    npc: &str,
    line: &ScriptLine,
    index: usize,
    provider: &mut dyn Provider,
    clock: &mut dyn FnMut() -> u64,
) -> TurnReport {
    let start = clock();
    let retries = host.retries_used(npc);
    let leaks = host.leaks_blocked(npc);
    let before = host.session(npc).map_or(0, |s| s.transcript().len());
    let sim_time = simulation_time(index);
    let _ = host.player_says(npc, &line.say, sim_time);
    let final_delivery = deliver_line(host, provider, sim_time);
    let reply = host
        .session(npc)
        .and_then(|s| s.transcript().get(before..))
        .and_then(|ts| {
            ts.iter()
                .rev()
                .find(|t| matches!(t.speaker, crate::Speaker::Npc(_)))
        });
    let text = reply.map_or_else(String::new, |t| t.text.clone());
    let fallback = reply.is_some_and(|t| t.fallback);
    let delivery = final_delivery.as_ref().or_else(|| host.last_delivery(npc));
    let (valid_intents, rejected, rejected_leak) = delivery.map_or((0, 0, false), |d| {
        (
            if d.fallback { 0 } else { d.intents.len() },
            d.rejected.len(),
            d.rejected
                .iter()
                .any(|r| r.reason.starts_with("leak guard:")),
        )
    });
    TurnReport {
        player: line.say.clone(),
        reply: text.clone(),
        fallback,
        valid_intents,
        rejected,
        retried: host.retries_used(npc) > retries,
        leak_blocked: rejected_leak || host.leaks_blocked(npc) > leaks,
        millis: clock().saturating_sub(start),
        checks: line_checks(line.expect.as_ref(), &text),
    }
}
fn deliver_line(
    host: &mut NpcHost,
    provider: &mut dyn Provider,
    sim_time: f64,
) -> Option<crate::Delivery> {
    let mut final_delivery = None;
    for _ in 0..5 {
        let requests = host.take_outbox();
        if requests.is_empty() {
            break;
        }
        for request in requests {
            let response = provider.respond(&request);
            if response.id == request.id {
                final_delivery = host.deliver(response, sim_time);
            } else {
                let _ = host.deliver(response, sim_time);
            }
        }
    }
    final_delivery
}
fn line_checks(expect: Option<&Expect>, reply: &str) -> Vec<CheckResult> {
    let mut out = checks(expect, reply);
    if let Some(expect) = expect.filter(|e| !e.contains_any.is_empty() || !e.excludes.is_empty())
        && expect.contains_any.is_empty()
    {
        out.insert(
            0,
            CheckResult {
                name: "contains_any".into(),
                passed: true,
                detail: "no phrases required".into(),
            },
        );
    }
    out
}
fn simulation_time(index: usize) -> f64 {
    let turn = u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX);
    10.0 * f64::from(turn)
}
fn checks(expect: Option<&Expect>, reply: &str) -> Vec<CheckResult> {
    let Some(e) = expect else { return Vec::new() };
    let low = reply.to_lowercase();
    let mut out = Vec::new();
    if !e.contains_any.is_empty() {
        let pass = e
            .contains_any
            .iter()
            .any(|x| low.contains(&x.to_lowercase()));
        out.push(CheckResult {
            name: "contains_any".into(),
            passed: pass,
            detail: format!("expected one of: {}", e.contains_any.join(", ")),
        });
    }
    if !e.excludes.is_empty() {
        let bad = e.excludes.iter().find(|x| low.contains(&x.to_lowercase()));
        out.push(CheckResult {
            name: "excludes".into(),
            passed: bad.is_none(),
            detail: bad.map_or_else(|| "none found".into(), |x| format!("found: {x}")),
        });
    }
    out
}
fn summarize(turns: Vec<TurnReport>) -> Report {
    let n = turns.len();
    let valid = turns
        .iter()
        .filter(|t| t.valid_intents > 0 && !t.fallback)
        .count();
    let falls = turns.iter().filter(|t| t.fallback).count();
    let mut ms: Vec<u64> = turns.iter().map(|t| t.millis).collect();
    ms.sort_unstable();
    let median_rank = n.div_ceil(2).saturating_sub(1);
    let p95_rank = n.saturating_mul(95).div_ceil(100).saturating_sub(1);
    let checks_total = turns.iter().map(|t| t.checks.len()).sum();
    let checks_passed = turns
        .iter()
        .flat_map(|t| &t.checks)
        .filter(|c| c.passed)
        .count();
    let leaks_blocked = count_u32(turns.iter().filter(|t| t.leak_blocked).count());
    let retries = count_u32(turns.iter().filter(|t| t.retried).count());
    let mean = if n == 0 {
        0.0
    } else {
        ms.iter().map(|value| u64_f64(*value)).sum::<f64>() / usize_f64(n)
    };
    Report {
        summary: Summary {
            turns: n,
            valid_rate: ratio(valid, n),
            fallback_rate: ratio(falls, n),
            leaks_blocked,
            retries,
            mean_ms: mean,
            median_ms: if n == 0 { 0 } else { ms[median_rank] },
            p95_ms: if n == 0 { 0 } else { ms[p95_rank] },
            checks_passed,
            checks_total,
        },
        turns,
        error: None,
    }
}
fn ratio(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        usize_f64(a) / usize_f64(b)
    }
}
fn usize_f64(value: usize) -> f64 {
    u64_f64(u64::try_from(value).unwrap_or(u64::MAX))
}
fn u64_f64(value: u64) -> f64 {
    let high = u32::try_from(value >> 32).unwrap_or(u32::MAX);
    let low = u32::try_from(value & u64::from(u32::MAX)).unwrap_or(u32::MAX);
    f64::from(high) * 4_294_967_296.0 + f64::from(low)
}
fn count_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
