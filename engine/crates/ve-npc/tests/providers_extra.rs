//! Additional composition, routing, and determinism coverage for providers.
#![expect(clippy::expect_used, reason = "tests assert by failing")]

use serde_json::json;
use std::cell::Cell;
use std::rc::Rc;
use ve_npc::{
    Chain, Metered, MindBudget, NpcHost, NpcSpec, Provider, ProviderError, ProviderRegistry,
    Reason, Request, RequestId, Response,
};

struct Reply {
    name: &'static str,
    value: &'static str,
    calls: Rc<Cell<u32>>,
    fails: bool,
}
impl Reply {
    fn new(name: &'static str, value: &'static str) -> (Self, Rc<Cell<u32>>) {
        let calls = Rc::new(Cell::new(0));
        (
            Self {
                name,
                value,
                calls: Rc::clone(&calls),
                fails: false,
            },
            calls,
        )
    }
    fn failing(name: &'static str, value: &'static str) -> Self {
        Self {
            name,
            value,
            calls: Rc::new(Cell::new(0)),
            fails: true,
        }
    }
}
impl Provider for Reply {
    fn respond(&mut self, req: &Request) -> Response {
        self.calls.set(self.calls.get() + 1);
        let outcome = if self.fails {
            Err(self.value.into())
        } else {
            Ok(json!({"intents":[{"name":"say","args":{"text":self.value}}]}).to_string())
        };
        Response {
            id: req.id,
            outcome,
        }
    }
    fn name(&self) -> &str {
        self.name
    }
}
fn req() -> Request {
    Request {
        id: RequestId(8),
        npc: "anna".into(),
        prompt: "p".into(),
        reason: Reason::PlayerSpoke,
        priority: 100,
        issued_at: 0.0,
    }
}
fn host() -> NpcHost {
    let mut host = NpcHost::new(MindBudget::default());
    host.register(NpcSpec {
        name: "anna".into(),
        persona: "baker".into(),
        rules: "kind".into(),
        goals: vec![],
        world_facts: vec![],
    })
    .expect("register");
    host
}
fn transcript(host: &NpcHost) -> Vec<String> {
    host.session("anna")
        .expect("session")
        .transcript()
        .iter()
        .map(|turn| turn.text.clone())
        .collect()
}

#[test]
fn chain_uses_middle_success_and_nests() {
    let (middle, count) = Reply::new("middle", "works");
    let mut chain = Chain::new()
        .then(Box::new(Reply::failing("first", "no")))
        .then(Box::new(middle))
        .then(Box::new(Reply::new("last", "unused").0));
    assert!(
        chain
            .respond(&req())
            .outcome
            .expect("middle succeeds")
            .contains("works")
    );
    assert_eq!(count.get(), 1);
    let mut nested = Chain::new().then(Box::new(
        Chain::new()
            .then(Box::new(Reply::failing("bad", "no")))
            .then(Box::new(Reply::new("inner", "nested").0)),
    ));
    assert!(
        nested
            .respond(&req())
            .outcome
            .expect("nested chain succeeds")
            .contains("nested")
    );
}

#[test]
fn registry_default_can_change_and_replacement_preserves_default_name() {
    let mut registry = ProviderRegistry::new();
    registry.register("one", Box::new(Reply::new("one", "ONE").0));
    registry.register("two", Box::new(Reply::new("two", "TWO").0));
    registry.set_default("two").expect("known");
    registry.register("two", Box::new(Reply::new("two", "NEW TWO").0));
    assert_eq!(registry.default_name(), Some("two"));
    let mut host = host();
    host.player_says("anna", "hi", 1.0).expect("asks");
    assert_eq!(host.pump(&mut registry, 2.0, 1), 1);
    assert_eq!(
        transcript(&host).last().map(String::as_str),
        Some("NEW TWO")
    );
    assert_eq!(
        registry.set_default("missing"),
        Err(ProviderError::Unknown("missing".into()))
    );
    assert_eq!(
        ProviderError::Unknown("x".into()).to_string(),
        "unknown provider: x"
    );
}

#[test]
fn meter_wraps_chain_and_routes_missing_named_provider_to_fallback() {
    let chain = Chain::new()
        .then(Box::new(Reply::failing("bad", "down")))
        .then(Box::new(Reply::new("good", "fine").0));
    let mut meter = Metered::new(chain);
    meter.respond(&req());
    assert_eq!((meter.stats().calls, meter.stats().failures), (1, 0));
    let mut host = host();
    host.set_fallback(
        "anna",
        ve_npc::FallbackTable::new().line(Reason::PlayerSpoke, "later"),
    );
    host.set_provider("anna", "removed")
        .expect("provider checked at use");
    let mut registry = ProviderRegistry::new();
    registry.register(
        "replacement",
        Box::new(Reply::new("replacement", "not used").0),
    );
    host.player_says("anna", "hello", 1.0).expect("asks");
    assert_eq!(host.pump(&mut registry, 2.0, 1), 1);
    assert_eq!(transcript(&host).last().map(String::as_str), Some("later"));
}

fn deterministic_run() -> Vec<String> {
    let mut host = host();
    let mut registry = ProviderRegistry::new();
    registry.register("fixed", Box::new(Reply::new("fixed", "same").0));
    for (text, time) in [("one", 1.0), ("two", 3.0)] {
        host.player_says("anna", text, time).expect("asks");
        host.pump(&mut registry, time + 1.0, 1);
    }
    transcript(&host)
}

#[test]
fn identical_host_and_registry_runs_have_identical_transcripts() {
    assert_eq!(deterministic_run(), deterministic_run());
}
