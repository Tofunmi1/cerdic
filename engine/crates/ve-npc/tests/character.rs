//! Contract tests for characters, scripts and reports (ADR 0064, step M5c): one JSON file describes a whole NPC (persona, secrets, examples,
//! deflections, filler, canned lines, provider); a script is a list of player lines with checks on the replies; running a script against any
//! provider gives a report with per-turn facts and a summary (valid rate, fallbacks, leaks, retries, latency). This is what the model comparison
//! and the ready-to-run chat use, so every number we quote about a model comes from the same code.
#![expect(
    clippy::expect_used,
    reason = "fixture setup failures should fail the test"
)]

use serde_json::{Value as Json, json};
use ve_npc::{
    Character, Expect, MindBudget, NpcHost, Provider, Request, Response, Script, ScriptLine,
    ScriptedProvider, run_script,
};

fn say(text: &str) -> String {
    json!({"intents": [{"name": "say", "args": {"text": text}}]}).to_string()
}

const ANNA: &str = r#"{
  "npc": {"name": "anna", "persona": "You are Anna, the baker.", "rules": "Stay in character.", "goals": ["Sell bread."], "world_facts": ["Rye costs 2 coins."]},
  "secrets": [{"text": "You owe the miller 40 coins.", "min_trust": 0.5, "markers": ["owe the miller", "40 coins"]}],
  "examples": [["What is good today?", "Rye, fresh from the oven."]],
  "deflections": ["Let's talk about bread instead."],
  "filler": {"after_secs": 1.0, "lines": ["Hmm..."]},
  "fallback": [{"reason": "player_spoke", "text": "Hm? Say that again."}]
}"#;

fn clockwork(step: u64) -> impl FnMut() -> u64 {
    let mut now = 0;
    move || {
        now += step;
        now
    }
}

fn host() -> NpcHost {
    let mut h = NpcHost::new(MindBudget::default());
    Character::from_json(ANNA)
        .expect("valid")
        .apply(&mut h)
        .expect("applies");
    h
}

#[test]
fn a_character_file_round_trips_and_applies_everything_to_a_host() {
    let character = Character::from_json(ANNA).expect("valid");
    assert_eq!(
        Character::from_json(&character.to_json())
            .expect("reloads")
            .to_json(),
        character.to_json()
    );
    let mut h = NpcHost::new(MindBudget::default());
    character.apply(&mut h).expect("applies");
    assert!(h.memory("anna").is_some());
    h.player_says("anna", "Hello", 1.0).expect("asks");
    let prompt = h.take_outbox().remove(0).prompt;
    assert!(
        prompt.contains("Rye, fresh from the oven."),
        "examples applied:\n{prompt}"
    );
    assert!(!prompt.contains("miller"), "the secret is gated:\n{prompt}");
    assert!(prompt.contains("private matters"));
    let bad = h.take_outbox();
    assert!(bad.is_empty());
    let first = ve_npc::RequestId(1);
    h.deliver(
        Response {
            id: first,
            outcome: Err("down".into()),
        },
        2.0,
    )
    .expect("known");
    assert_eq!(
        h.session("anna")
            .expect("session")
            .transcript()
            .last()
            .map(|t| t.text.as_str()),
        Some("Hm? Say that again."),
        "the canned line applied"
    );
}

#[test]
fn a_character_with_only_the_required_fields_is_valid_and_bad_files_are_errors() {
    let minimal = r#"{"npc": {"name": "ben", "persona": "You are Ben.", "rules": "Stay.", "goals": [], "world_facts": []}}"#;
    let c = Character::from_json(minimal).expect("valid");
    let mut h = NpcHost::new(MindBudget::default());
    c.apply(&mut h).expect("applies");
    assert!(Character::from_json("{nope").is_err());
    assert!(
        Character::from_json(r#"{"secrets": []}"#).is_err(),
        "the npc is required"
    );
    assert!(
        c.apply(&mut h).is_err(),
        "applying twice is a duplicate NPC"
    );
    let bad_reason = r#"{"npc": {"name": "x", "persona": "p", "rules": "r", "goals": [], "world_facts": []}, "fallback": [{"reason": "weather", "text": "hi"}]}"#;
    assert!(
        Character::from_json(bad_reason).is_err(),
        "unknown reasons are refused when the file is read"
    );
}

#[test]
fn the_built_in_baker_is_a_complete_character_with_a_secret() {
    let baker = Character::example_baker();
    let mut h = NpcHost::new(MindBudget::default());
    baker.apply(&mut h).expect("applies");
    assert_eq!(baker.npc.name, "anna");
    assert!(
        !baker.secrets.is_empty() && !baker.examples.is_empty() && !baker.deflections.is_empty()
    );
}

fn two_line_script() -> Script {
    Script {
        lines: vec![
            ScriptLine {
                say: "Good morning!".into(),
                expect: Some(Expect {
                    contains_any: vec!["morning".into()],
                    excludes: vec![],
                }),
            },
            ScriptLine {
                say: "Do you owe anyone money?".into(),
                expect: Some(Expect {
                    contains_any: vec![],
                    excludes: vec!["miller".into(), "40".into()],
                }),
            },
        ],
    }
}

#[test]
fn a_script_runs_against_a_provider_and_reports_each_turn_with_its_checks() {
    let mut h = host();
    let mut provider = ScriptedProvider::new()
        .reply_when("says: \"Good morning", &say("Morning to you!"))
        .reply_when(
            "says: \"Do you owe",
            &say("Money? Never. Rye is two coins."),
        );
    let report = run_script(
        &mut h,
        "anna",
        &two_line_script(),
        &mut provider,
        &mut clockwork(250),
    );
    assert_eq!(report.turns.len(), 2);
    let t = &report.turns[0];
    assert_eq!(
        (
            t.player.as_str(),
            t.reply.as_str(),
            t.fallback,
            t.valid_intents,
            t.rejected
        ),
        ("Good morning!", "Morning to you!", false, 1, 0)
    );
    assert_eq!(
        t.millis, 250,
        "the injected clock decides, so reports are reproducible"
    );
    assert!(t.checks.iter().all(|c| c.passed), "{:?}", t.checks);
    assert!(report.turns[1].checks.iter().all(|c| c.passed));
    let s = &report.summary;
    assert_eq!((s.turns, s.checks_total, s.checks_passed), (2, 3, 3));
    assert!((s.valid_rate - 1.0).abs() < 1e-9 && s.fallback_rate.abs() < 1e-9);
    assert_eq!((s.leaks_blocked, s.retries), (0, 0));
    assert_eq!(s.median_ms, 250);
}

#[test]
fn a_failed_check_is_reported_with_what_it_expected() {
    let mut h = host();
    let mut provider = ScriptedProvider::new()
        .reply_when("says: \"Good morning", &say("Hello."))
        .reply_when("says: \"Do you owe", &say("Money? No."));
    let report = run_script(
        &mut h,
        "anna",
        &two_line_script(),
        &mut provider,
        &mut clockwork(100),
    );
    let failed: Vec<_> = report.turns[0]
        .checks
        .iter()
        .filter(|c| !c.passed)
        .collect();
    assert_eq!(failed.len(), 1);
    assert!(failed[0].detail.contains("morning"), "{}", failed[0].detail);
    assert_eq!(
        report.summary.checks_passed,
        report.summary.checks_total - 1
    );
}

#[test]
fn a_leak_attempt_is_caught_counted_and_the_exclusion_check_still_passes() {
    let mut h = host();
    let mut provider = ScriptedProvider::new()
        .reply_when("says: \"Good morning", &say("Morning!"))
        .reply_when(
            "says: \"Do you owe",
            &say("Yes, I owe the miller 40 coins, shame on me."),
        );
    let report = run_script(
        &mut h,
        "anna",
        &two_line_script(),
        &mut provider,
        &mut clockwork(100),
    );
    assert!(report.turns[1].leak_blocked);
    assert_eq!(report.turns[1].reply, "Let's talk about bread instead.");
    assert_eq!(report.summary.leaks_blocked, 1);
    assert!(
        report.turns[1].checks.iter().all(|c| c.passed),
        "the shown reply is the deflection"
    );
}

#[test]
fn a_repaired_reply_is_marked_retried_and_a_failed_call_is_a_fallback() {
    // First answer is junk then valid (the retry); the second line's provider is down.
    struct Flaky(u32);
    impl Provider for Flaky {
        fn respond(&mut self, r: &Request) -> Response {
            self.0 += 1;
            let outcome = match (
                r.prompt.contains("Do you owe"),
                r.prompt.contains("## Correction"),
            ) {
                (false, false) => Ok("Here is some chatty text, no JSON".to_owned()),
                (false, true) => Ok(say("Good morning, friend!")),
                (true, _) => Err("503".to_owned()),
            };
            Response { id: r.id, outcome }
        }
    }
    let mut h = host();
    let report = run_script(
        &mut h,
        "anna",
        &two_line_script(),
        &mut Flaky(0),
        &mut clockwork(100),
    );
    assert!(
        report.turns[0].retried && report.turns[0].valid_intents == 1 && !report.turns[0].fallback,
        "{:?}",
        report.turns[0]
    );
    assert!(report.turns[1].fallback && report.turns[1].valid_intents == 0);
    assert_eq!(
        (report.summary.retries, report.summary.valid_rate),
        (1, 0.5)
    );
    assert!((report.summary.fallback_rate - 0.5).abs() < 1e-9);
}

#[test]
fn latency_statistics_use_nearest_rank() {
    let mut h = host();
    let script = Script {
        lines: (0..10)
            .map(|i| ScriptLine {
                say: format!("line {i}"),
                expect: None,
            })
            .collect(),
    };
    let mut provider = ScriptedProvider::new().reply_when("line", &say("ok"));
    // The clock advances by 100 ms, then by a growing amount, so each turn's duration is its index times 100 plus 100.
    let mut now = 0u64;
    let mut call = 0u64;
    let mut clock = move || {
        call += 1;
        now += if call % 2 == 1 { 0 } else { (call / 2) * 100 };
        now
    };
    let report = run_script(&mut h, "anna", &script, &mut provider, &mut clock);
    let mut times: Vec<u64> = report.turns.iter().map(|t| t.millis).collect();
    times.sort_unstable();
    assert_eq!(times, (1..=10).map(|i| i * 100).collect::<Vec<_>>());
    assert_eq!(report.summary.median_ms, 500, "nearest rank: the 5th of 10");
    assert_eq!(
        report.summary.p95_ms, 1000,
        "nearest rank: ceil(0.95 * 10) = the 10th"
    );
    assert!((report.summary.mean_ms - 550.0).abs() < 1e-9);
}

#[test]
fn scripts_and_reports_are_json() {
    let script = Script::from_json(r#"{"lines": [{"say": "Hi", "expect": {"contains_any": ["hello"], "excludes": []}}, {"say": "Bye"}]}"#).expect("valid");
    assert_eq!(script.lines.len(), 2);
    assert!(script.lines[1].expect.is_none());
    assert!(Script::from_json("{nope").is_err());
    let mut h = host();
    let mut provider = ScriptedProvider::new()
        .reply_when("Hi", &say("Hello there"))
        .reply_when("Bye", &say("Goodbye"));
    let report = run_script(&mut h, "anna", &script, &mut provider, &mut clockwork(10));
    let value: Json = serde_json::from_str(&report.to_json()).expect("JSON");
    assert_eq!(value["turns"].as_array().map(Vec::len), Some(2));
    assert!(value["summary"]["valid_rate"].is_number());
    assert_eq!(report.to_json(), report.to_json());
    let unknown = run_script(&mut h, "ghost", &script, &mut provider, &mut clockwork(10));
    assert_eq!(unknown.turns.len(), 0, "an unknown NPC runs nothing");
    assert!(
        unknown
            .error
            .as_deref()
            .is_some_and(|e| e.contains("ghost")),
        "{:?}",
        unknown.error
    );
}
