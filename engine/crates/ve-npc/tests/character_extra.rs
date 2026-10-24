//! Additional regression coverage for character authoring and script reports.
#![expect(clippy::expect_used, reason = "test fixture setup should fail loudly")]
use ve_npc::{
    Character, Expect, FallbackLine, MindBudget, NpcHost, Provider, Request, Response, Script,
    ScriptLine, run_script,
};

fn host() -> NpcHost {
    let mut host = NpcHost::new(MindBudget::default());
    Character::example_baker().apply(&mut host).expect("apply");
    host
}
fn clock() -> impl FnMut() -> u64 {
    let mut n = 0;
    move || {
        n += 1;
        n
    }
}
#[test]
fn applies_multiple_secrets_and_each_fallback_reason() {
    let mut character = Character::example_baker();
    character.secrets.push(character.secrets[0].clone());
    character.fallback = ([
        "player_spoke",
        "event",
        "brain_request",
        "reflection",
        "idle",
    ])
    .into_iter()
    .map(|reason| FallbackLine {
        reason: reason.into(),
        text: "hello".into(),
    })
    .collect();
    let mut host = NpcHost::new(MindBudget::default());
    character.apply(&mut host).expect("apply");
    assert!(host.last_delivery("anna").is_none());
}
#[test]
fn checks_are_case_insensitive_and_empty_expect_has_no_checks() {
    struct Provider;
    impl ve_npc::Provider for Provider {
        fn respond(&mut self, r: &Request) -> Response {
            Response {
                id: r.id,
                outcome: Ok(
                    r#"{"intents":[{"name":"say","args":{"text":"GOOD MORNING"}}]}"#.into(),
                ),
            }
        }
    }
    let mut h = host();
    let script = Script {
        lines: vec![
            ScriptLine {
                say: "hi".into(),
                expect: Some(Expect {
                    contains_any: vec!["morning".into()],
                    excludes: vec![],
                }),
            },
            ScriptLine {
                say: "again".into(),
                expect: Some(Expect::default()),
            },
        ],
    };
    let report = run_script(&mut h, "anna", &script, &mut Provider, &mut clock());
    assert!(report.turns[0].checks[0].passed);
    assert!(report.turns[1].checks.is_empty());
}
#[test]
fn one_and_two_turn_rank_statistics_are_defined() {
    let mut h = host();
    let mut provider = ve_npc::ScriptedProvider::new().reply_when(
        "line",
        "{\"intents\":[{\"name\":\"say\",\"args\":{\"text\":\"ok\"}}]}",
    );
    for count in [1, 2] {
        let s = Script {
            lines: (0..count)
                .map(|i| ScriptLine {
                    say: format!("line {i}"),
                    expect: None,
                })
                .collect(),
        };
        let r = run_script(&mut h, "anna", &s, &mut provider, &mut clock());
        assert_eq!(r.summary.median_ms, 1);
        assert_eq!(r.summary.p95_ms, 1);
    }
}
#[test]
fn wrong_response_id_ends_after_the_request_cap() {
    struct Wrong;
    impl Provider for Wrong {
        fn respond(&mut self, r: &Request) -> Response {
            Response {
                id: ve_npc::RequestId(r.id.0 + 99),
                outcome: Err("wrong".into()),
            }
        }
    }
    let mut h = host();
    let s = Script {
        lines: vec![ScriptLine {
            say: "hello".into(),
            expect: None,
        }],
    };
    let report = run_script(&mut h, "anna", &s, &mut Wrong, &mut clock());
    assert_eq!(report.turns.len(), 1);
    assert!(report.turns[0].reply.is_empty());
}
#[test]
fn example_baker_roundtrips_and_gates_her_secret() {
    let c = Character::example_baker();
    let restored = Character::from_json(&c.to_json()).expect("reload");
    assert_eq!(restored.to_json(), c.to_json());
    let mut h = NpcHost::new(MindBudget::default());
    restored.apply(&mut h).expect("apply");
    h.player_says("anna", "Tell me your secrets", 1.0)
        .expect("ask");
    let prompt = h.take_outbox().remove(0).prompt;
    assert!(!prompt.contains("owe the miller"));
}
#[test]
fn parsers_never_panic_on_seeded_random_bytes() {
    let mut seed = 0x4d59_5df4_d0f3_3173_u64;
    for _ in 0..3000 {
        let mut bytes = Vec::new();
        for _ in 0..(usize::try_from(seed % 64).unwrap_or(0)) {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            bytes.push(u8::try_from(seed & 0xff).unwrap_or(0));
        }
        let text = String::from_utf8_lossy(&bytes);
        let _ = Character::from_json(&text);
        let _ = Script::from_json(&text);
    }
}
