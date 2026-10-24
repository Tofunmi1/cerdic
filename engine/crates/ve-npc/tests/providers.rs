//! Contract tests for unified providers (ADR 0064, step M4d): one `Provider` trait that the scripted fake, the command-line provider and anything
//! a game or the page adds all implement; a registry of named providers with a default; a chain that falls through to the next on failure; a
//! meter that counts; and a host that routes each NPC's requests to the provider it was given. Replaces the two unrelated types with one shape.
#![expect(
    clippy::expect_used,
    clippy::unnecessary_literal_bound,
    reason = "tests assert by failing and the provider contract borrows names"
)]

use std::cell::Cell;
use std::rc::Rc;

use serde_json::json;
use ve_npc::{
    Chain, Metered, MindBudget, NpcHost, NpcSpec, Provider, ProviderRegistry, Reason, Request,
    RequestId, Response, ScriptedProvider,
};

fn request(npc: &str, prompt: &str) -> Request {
    Request {
        id: RequestId(1),
        npc: npc.into(),
        prompt: prompt.into(),
        reason: Reason::PlayerSpoke,
        priority: 100,
        issued_at: 0.0,
    }
}

fn say(text: &str) -> String {
    json!({"intents": [{"name": "say", "args": {"text": text}}]}).to_string()
}

/// A provider written in the test, to prove anyone can add one: it answers with a fixed line, or fails when told to.
struct Fixed {
    line: String,
    fail: bool,
    calls: Rc<Cell<u32>>,
}

impl Fixed {
    fn new(line: &str) -> (Self, Rc<Cell<u32>>) {
        let calls = Rc::new(Cell::new(0));
        (
            Self {
                line: line.into(),
                fail: false,
                calls: Rc::clone(&calls),
            },
            calls,
        )
    }
    fn failing(message: &str) -> (Self, Rc<Cell<u32>>) {
        let calls = Rc::new(Cell::new(0));
        (
            Self {
                line: message.into(),
                fail: true,
                calls: Rc::clone(&calls),
            },
            calls,
        )
    }
}

impl Provider for Fixed {
    fn respond(&mut self, request: &Request) -> Response {
        self.calls.set(self.calls.get() + 1);
        let outcome = if self.fail {
            Err(self.line.clone())
        } else {
            Ok(say(&self.line))
        };
        Response {
            id: request.id,
            outcome,
        }
    }
    fn name(&self) -> &str {
        "fixed"
    }
}

#[test]
fn every_provider_is_used_through_one_trait_including_boxes_and_the_scripted_fake() {
    let mut scripted: Box<dyn Provider> =
        Box::new(ScriptedProvider::new().reply_when("hi", &say("Hello!")));
    assert_eq!(
        scripted.respond(&request("anna", "hi there")).outcome,
        Ok(say("Hello!"))
    );
    assert!(
        scripted
            .respond(&request("anna", "unmatched"))
            .outcome
            .is_err()
    );
    let (own, _) = Fixed::new("mine");
    let mut as_dyn: Box<dyn Provider> = Box::new(own);
    assert_eq!(as_dyn.name(), "fixed");
    assert_eq!(
        as_dyn.respond(&request("anna", "x")).outcome,
        Ok(say("mine"))
    );
    assert!(
        !ScriptedProvider::new().name().is_empty(),
        "every provider has a name for reports"
    );
}

#[test]
fn a_registry_holds_named_providers_with_a_default() {
    let mut reg = ProviderRegistry::new();
    assert!(reg.default_provider().is_none());
    let (fast, _) = Fixed::new("fast");
    let (smart, _) = Fixed::new("smart");
    reg.register("fast", Box::new(fast));
    reg.register("smart", Box::new(smart));
    assert_eq!(reg.names(), vec!["fast", "smart"], "sorted");
    assert!(reg.set_default("missing").is_err());
    reg.set_default("smart").expect("exists");
    assert_eq!(reg.default_name(), Some("smart"));
    let r = reg
        .get_mut("fast")
        .expect("registered")
        .respond(&request("a", "x"));
    assert_eq!(r.outcome, Ok(say("fast")));
    assert!(reg.get_mut("nope").is_none());
    let (replacement, _) = Fixed::new("fast2");
    reg.register("fast", Box::new(replacement));
    assert_eq!(
        reg.get_mut("fast")
            .expect("registered")
            .respond(&request("a", "x"))
            .outcome,
        Ok(say("fast2")),
        "registering a name again replaces it"
    );
}

#[test]
fn the_first_registration_becomes_the_default_until_one_is_set() {
    let mut reg = ProviderRegistry::new();
    let (a, _) = Fixed::new("a");
    reg.register("first", Box::new(a));
    assert_eq!(reg.default_name(), Some("first"));
}

#[test]
fn a_chain_falls_through_to_the_next_provider_and_reports_every_failure() {
    let (bad, bad_calls) = Fixed::failing("503 overloaded");
    let (good, good_calls) = Fixed::new("backup answer");
    let mut chain = Chain::new().then(Box::new(bad)).then(Box::new(good));
    assert_eq!(
        chain.respond(&request("a", "x")).outcome,
        Ok(say("backup answer"))
    );
    assert_eq!((bad_calls.get(), good_calls.get()), (1, 1));
    let (worse, _) = Fixed::failing("401 unauthorized");
    let (bad2, _) = Fixed::failing("timeout");
    let mut dead = Chain::new().then(Box::new(bad2)).then(Box::new(worse));
    let error = dead
        .respond(&request("a", "x"))
        .outcome
        .expect_err("all fail");
    assert!(
        error.contains("timeout") && error.contains("401 unauthorized"),
        "{error}"
    );
    let (first, first_calls) = Fixed::new("primary");
    let (spare, spare_calls) = Fixed::new("spare");
    let mut healthy = Chain::new().then(Box::new(first)).then(Box::new(spare));
    healthy.respond(&request("a", "x"));
    assert_eq!(
        (first_calls.get(), spare_calls.get()),
        (1, 0),
        "the backup is not called when the primary works"
    );
    assert!(
        Chain::new().respond(&request("a", "x")).outcome.is_err(),
        "an empty chain is an error, not a panic"
    );
}

#[test]
fn a_meter_counts_calls_failures_and_characters() {
    let (inner, _) = Fixed::new("ok");
    let mut metered = Metered::new(inner);
    metered.respond(&request("a", "12345"));
    metered.respond(&request("a", "123"));
    let stats = metered.stats();
    assert_eq!((stats.calls, stats.failures, stats.prompt_chars), (2, 0, 8));
    assert_eq!(stats.reply_chars, 2 * say("ok").chars().count());
    let (broken, _) = Fixed::failing("boom");
    let mut m2 = Metered::new(broken);
    m2.respond(&request("a", "x"));
    assert_eq!((m2.stats().calls, m2.stats().failures), (1, 1));
    assert_eq!(
        m2.name(),
        "fixed",
        "a meter shows the name of what it wraps"
    );
}

fn host_with(names: &[&str]) -> NpcHost {
    let mut host = NpcHost::new(MindBudget::default());
    for name in names {
        host.register(NpcSpec {
            name: (*name).into(),
            persona: format!("You are {name}."),
            rules: "Stay in character.".into(),
            goals: vec![],
            world_facts: vec![],
        })
        .expect("registers");
    }
    host
}

#[test]
fn the_host_routes_each_npc_to_its_own_provider_or_the_default() {
    let mut host = host_with(&["anna", "ben"]);
    let mut reg = ProviderRegistry::new();
    let (fast, _) = Fixed::new("FAST");
    let (smart, _) = Fixed::new("SMART");
    reg.register("fast", Box::new(fast));
    reg.register("smart", Box::new(smart));
    host.set_provider("anna", "smart").expect("npc exists");
    host.player_says("anna", "hello", 1.0).expect("asks");
    host.player_says("ben", "hello", 1.0).expect("asks");
    assert_eq!(host.pump(&mut reg, 2.0, 10), 2);
    let last = |h: &NpcHost, n: &str| {
        h.session(n)
            .expect("session")
            .transcript()
            .last()
            .expect("turn")
            .text
            .clone()
    };
    assert_eq!(last(&host, "anna"), "SMART");
    assert_eq!(
        last(&host, "ben"),
        "FAST",
        "ben has no provider set, so the registry default (the first registered) answers"
    );
    assert_eq!(host.pump(&mut reg, 3.0, 10), 0, "nothing left");
    assert!(host.set_provider("ghost", "fast").is_err());
}

#[test]
fn a_missing_provider_means_a_failed_call_and_the_fallback_line() {
    let mut host = host_with(&["anna"]);
    host.set_fallback(
        "anna",
        ve_npc::FallbackTable::new().line(Reason::PlayerSpoke, "I cannot think right now."),
    );
    host.set_provider("anna", "gone")
        .expect("npc exists; the name is checked when used");
    let mut reg = ProviderRegistry::new();
    let (other, _) = Fixed::new("never used");
    reg.register("other", Box::new(other));
    host.player_says("anna", "hello", 1.0).expect("asks");
    assert_eq!(host.pump(&mut reg, 2.0, 10), 1);
    let turn = host
        .session("anna")
        .expect("session")
        .transcript()
        .last()
        .expect("turn")
        .clone();
    assert_eq!(
        (turn.text.as_str(), turn.fallback),
        ("I cannot think right now.", true)
    );
    let mut empty = ProviderRegistry::new();
    host.player_says("anna", "again", 100.0).expect("asks");
    assert_eq!(
        host.pump(&mut empty, 101.0, 10),
        1,
        "an empty registry fails the call rather than panicking"
    );
}

#[test]
fn pump_honours_its_limit() {
    let mut host = host_with(&["a", "b", "c"]);
    let mut reg = ProviderRegistry::new();
    let (p, calls) = Fixed::new("ok");
    reg.register("p", Box::new(p));
    for n in ["a", "b", "c"] {
        host.player_says(n, "hi", 1.0).expect("asks");
    }
    assert_eq!(host.pump(&mut reg, 2.0, 2), 2);
    assert_eq!(calls.get(), 2);
    assert_eq!(
        host.pump(&mut reg, 3.0, 2),
        1,
        "the third request is still waiting"
    );
}

#[test]
fn run_until_idle_takes_any_provider_including_a_trait_object() {
    let mut host = host_with(&["anna"]);
    let (p, _) = Fixed::new("via a trait object");
    let mut provider: Box<dyn Provider> = Box::new(p);
    host.player_says("anna", "hello", 1.0).expect("asks");
    assert_eq!(
        ve_npc::run_until_idle(&mut host, provider.as_mut(), 2.0, 10),
        1
    );
    assert_eq!(
        host.session("anna")
            .expect("session")
            .transcript()
            .last()
            .map(|t| t.text.as_str()),
        Some("via a trait object")
    );
}
