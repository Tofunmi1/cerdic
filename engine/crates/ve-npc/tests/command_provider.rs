//! Contract tests for the command provider (ADR 0064, step M4c): a provider that hands each prompt to an external program on standard input
//! and reads the reply from standard output. It lets the NPC host talk to a real model through any command-line client (the Claude CLI, a
//! local server's client, a script) with no network code, no keys in our code and no web page, on a development machine. Native only.
#![cfg(not(target_arch = "wasm32"))]

use std::time::{Duration, Instant};

use serde_json::json;
use ve_npc::{CommandProvider, MindBudget, NpcHost, NpcSpec, Reason, Request};

fn request(prompt: &str) -> Request {
    Request {
        id: ve_npc::RequestId(1),
        npc: "anna".into(),
        prompt: prompt.into(),
        reason: Reason::PlayerSpoke,
        priority: 100,
        issued_at: 0.0,
    }
}

fn sh(script: &str) -> CommandProvider {
    CommandProvider::new("sh").args(["-c", script])
}

fn say(text: &str) -> String {
    json!({"intents": [{"name": "say", "args": {"text": text}}]}).to_string()
}

#[test]
fn the_prompt_goes_in_on_stdin_and_the_trimmed_reply_comes_out() {
    let mut p = sh("cat > /dev/null; printf '  hello from the script  \\n\\n'");
    let r = p.respond(&request("anything"));
    assert_eq!(r.id, ve_npc::RequestId(1));
    assert_eq!(r.outcome, Ok("hello from the script".to_owned()));
    let mut echo = sh("cat");
    assert_eq!(
        echo.respond(&request("round trip")).outcome,
        Ok("round trip".to_owned()),
        "what goes in is what the program sees"
    );
}

#[test]
fn a_big_prompt_does_not_deadlock() {
    let big = "word ".repeat(40_000);
    let mut p = sh("cat");
    let started = Instant::now();
    let r = p.respond(&request(&big));
    assert_eq!(r.outcome.as_ref().map(String::len), Ok(big.trim().len()));
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_failing_program_is_an_error_with_its_message() {
    let mut p = sh("cat > /dev/null; echo 'rate limit exceeded' >&2; exit 3");
    let outcome = p.respond(&request("x")).outcome;
    let error = outcome.expect_err("exit code 3");
    assert!(
        error.contains("rate limit exceeded") && error.contains('3'),
        "{error}"
    );
    let mut missing = CommandProvider::new("definitely-not-a-real-program-xyz");
    assert!(
        missing.respond(&request("x")).outcome.is_err(),
        "a program that cannot start is an error, not a panic"
    );
}

#[test]
fn a_slow_program_is_killed_at_the_timeout() {
    let mut p = sh("cat > /dev/null; sleep 5; echo late").with_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let outcome = p.respond(&request("x")).outcome;
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "it did not wait the full five seconds"
    );
    let error = outcome.expect_err("timed out");
    assert!(error.to_lowercase().contains("timed out"), "{error}");
}

#[test]
fn an_empty_reply_is_an_error_so_the_fallback_line_is_used() {
    let mut p = sh("cat > /dev/null; true");
    assert!(p.respond(&request("x")).outcome.is_err());
}

#[test]
fn the_host_talks_to_a_real_program_end_to_end() {
    // A program that answers with a valid reply only when the prompt contains the persona and the player's line.
    let reply = say("Welcome to my bakery!");
    let script = format!(
        "p=$(cat); case \"$p\" in *'You are Anna'*'Good morning'*) printf '%s' '{reply}';; *) echo nope >&2; exit 1;; esac"
    );
    let mut provider = sh(&script);
    let mut host = NpcHost::new(MindBudget::default());
    host.register(NpcSpec {
        name: "anna".into(),
        persona: "You are Anna, the baker.".into(),
        rules: "Stay in character.".into(),
        goals: vec![],
        world_facts: vec![],
    })
    .expect("registers");
    host.player_says("anna", "Good morning", 1.0).expect("asks");
    for r in host.take_outbox() {
        let response = provider.respond(&r);
        host.deliver(response, 2.0).expect("known");
    }
    let turn = host
        .session("anna")
        .expect("session")
        .transcript()
        .last()
        .expect("a reply")
        .clone();
    assert_eq!(
        (turn.text.as_str(), turn.fallback),
        ("Welcome to my bakery!", false)
    );
}

#[test]
fn run_until_idle_drives_the_host_and_the_provider_together() {
    let reply = say("At your service.");
    let mut provider = sh(&format!("cat > /dev/null; printf '%s' '{reply}'"));
    let mut host = NpcHost::new(MindBudget::default());
    host.register(NpcSpec {
        name: "anna".into(),
        persona: "You are Anna.".into(),
        rules: "Stay in character.".into(),
        goals: vec![],
        world_facts: vec![],
    })
    .expect("registers");
    host.player_says("anna", "Hello", 1.0).expect("asks");
    let handled = ve_npc::run_until_idle(&mut host, &mut provider, 1.0, 10);
    assert_eq!(handled, 1, "one request was sent and answered");
    assert_eq!(
        host.session("anna")
            .expect("session")
            .transcript()
            .last()
            .map(|t| t.text.as_str()),
        Some("At your service.")
    );
    assert_eq!(
        ve_npc::run_until_idle(&mut host, &mut provider, 2.0, 10),
        0,
        "nothing left to do"
    );
}
