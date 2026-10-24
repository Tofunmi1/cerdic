//! Additional native command-provider edge-case coverage.
#![cfg(not(target_arch = "wasm32"))]
#![expect(clippy::expect_used, reason = "tests assert by failing")]

use std::fs;
use std::time::Duration;
use ve_npc::{CommandProvider, MindBudget, NpcHost, NpcSpec, Reason, Request};

fn request(prompt: &str, id: u64) -> Request {
    Request {
        id: ve_npc::RequestId(id),
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
fn host() -> NpcHost {
    let mut host = NpcHost::new(MindBudget::default());
    host.register(NpcSpec {
        name: "anna".into(),
        persona: "Anna".into(),
        rules: String::new(),
        goals: vec![],
        world_facts: vec![],
    })
    .expect("register");
    host
}

#[test]
fn arguments_keep_spaces_and_quotes_intact() {
    let argument = "a value with 'quotes'";
    let script = "printf '%s' \"$1\"";
    let mut provider = CommandProvider::new("sh").args(["-c", script, "provider", argument]);
    assert_eq!(
        provider.respond(&request("", 1)).outcome,
        Ok(argument.into())
    );
}

#[test]
fn successful_stderr_does_not_enter_reply() {
    let mut provider = sh("echo diagnostic >&2; echo answer");
    assert_eq!(
        provider.respond(&request("", 1)).outcome,
        Ok("answer".into())
    );
}

#[test]
fn closing_stdin_early_still_returns_reply() {
    let mut provider = sh("exec 0<&-; printf reply");
    assert_eq!(
        provider.respond(&request(&"p".repeat(300_000), 1)).outcome,
        Ok("reply".into())
    );
}

#[test]
fn large_prompt_and_slow_stream_reader_do_not_deadlock() {
    let mut provider =
        sh("while IFS= read -r line; do :; done; printf first; sleep 0.05; printf second");
    assert_eq!(
        provider
            .respond(&request(&"prompt ".repeat(43_000), 1))
            .outcome,
        Ok("firstsecond".into())
    );
}

#[test]
fn timeout_kills_child_before_marker_write() {
    let marker = std::env::temp_dir().join(format!("ve-npc-timeout-{}", std::process::id()));
    let _ = fs::remove_file(&marker);
    let script = format!("sleep 1; touch '{}'", marker.display());
    let mut provider = sh(&script).with_timeout(Duration::from_millis(50));
    let outcome = provider.respond(&request("", 1)).outcome;
    assert!(outcome.expect_err("timeout").contains("timed out"));
    std::thread::sleep(Duration::from_millis(1100));
    assert!(
        !marker.exists(),
        "timed-out process must not reach its marker write"
    );
}

#[test]
fn one_provider_handles_sequential_requests() {
    let mut provider = sh("cat; printf ' reply'");
    assert_eq!(
        provider.respond(&request("one", 1)).outcome,
        Ok("one reply".into())
    );
    assert_eq!(
        provider.respond(&request("two", 2)).outcome,
        Ok("two reply".into())
    );
}

#[test]
fn runner_respects_request_limit_and_failing_provider_uses_fallback() {
    let mut host = host();
    host.set_fallback(
        "anna",
        ve_npc::FallbackTable::new().line(Reason::PlayerSpoke, "canned answer"),
    );
    host.player_says("anna", "hello", 1.0).expect("player says");
    let mut provider = sh(
        "cat >/dev/null; printf '%s' '{\"intents\":[{\"name\":\"say\",\"args\":{\"text\":\"hi\"}}]}'",
    );
    assert_eq!(ve_npc::run_until_idle(&mut host, &mut provider, 1.0, 0), 0);
    assert_eq!(ve_npc::run_until_idle(&mut host, &mut provider, 1.0, 1), 1);
    host.player_says("anna", "hello again", 2.0)
        .expect("second player line");
    let mut failing = sh("cat >/dev/null; exit 2");
    assert_eq!(ve_npc::run_until_idle(&mut host, &mut failing, 2.0, 1), 1);
    assert!(
        host.session("anna")
            .expect("session")
            .transcript()
            .last()
            .is_some_and(|turn| turn.fallback)
    );
}
