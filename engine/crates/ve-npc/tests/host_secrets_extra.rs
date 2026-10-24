//! Edge coverage for secret scoping and leak protection.
#![expect(clippy::expect_used, reason = "tests assert by failing")]

use serde_json::json;
use ve_npc::{MindBudget, NpcHost, NpcSpec, Response, SecretSpec};

fn host() -> NpcHost {
    let mut host = NpcHost::new(MindBudget::default());
    host.register(NpcSpec {
        name: "n".into(),
        persona: "P".into(),
        rules: "R".into(),
        goals: vec![],
        world_facts: vec![],
    })
    .expect("register");
    host.add_secret(
        "n",
        SecretSpec {
            text: "alpha secret".into(),
            min_trust: 0.2,
            markers: vec!["alpha".into()],
        },
    )
    .expect("secret");
    host.add_secret(
        "n",
        SecretSpec {
            text: "beta secret".into(),
            min_trust: 0.8,
            markers: vec!["beta".into()],
        },
    )
    .expect("secret");
    host
}
fn ask(host: &mut NpcHost) -> ve_npc::Request {
    host.player_says("n", "hello", 1.0).expect("ask");
    host.take_outbox().remove(0)
}
fn respond(host: &mut NpcHost, line: &str) -> ve_npc::Delivery {
    let request = ask(host);
    host.deliver(
        Response {
            id: request.id,
            outcome: Ok(json!({"intents":[{"name":"say","args":{"text":line}}]}).to_string()),
        },
        2.0,
    )
    .expect("deliver")
}

#[test]
fn thresholds_examples_fallback_and_rotation_work() {
    let mut host = host();
    host.set_deflections("n", vec!["d1".into(), "d2".into(), "d3".into()]);
    let p = ask(&mut host).prompt;
    assert!(!p.contains("alpha secret") && !p.contains("beta secret"));
    assert!(p.contains("private matters"));
    for (index, expected) in ["d1", "d2", "d3"].into_iter().enumerate() {
        respond(&mut host, "alphabet soup");
        assert_eq!(
            host.session("n")
                .expect("session")
                .transcript()
                .last()
                .expect("turn")
                .text,
            expected
        );
        assert_eq!(
            host.leaks_blocked("n"),
            u32::try_from(index + 1).expect("small index")
        );
    }
    host.set_examples("n", vec![("q\nsecond".into(), "a\nsecond".into())])
        .expect("examples");
    let prompt = ask(&mut host).prompt;
    assert!(prompt.contains("player: q\nsecond") && prompt.contains("n: a\nsecond"));
    let request = ask(&mut host);
    let trust = json!({"intents":[{"name":"adjust_relationship","args":{"other":"player","affinity":0.0,"trust":0.9}}]}).to_string();
    host.deliver(
        Response {
            id: request.id,
            outcome: Ok(trust),
        },
        2.0,
    )
    .expect("trust");
    assert!(ask(&mut host).prompt.contains("beta secret"));
}

#[test]
fn fallback_and_save_guard() {
    use ve_npc::{FallbackTable, Reason};
    let mut host = host();
    host.set_fallback(
        "n",
        FallbackTable::new().line(Reason::PlayerSpoke, "alpha fallback"),
    );
    let request = ask(&mut host);
    let delivery = host
        .deliver(
            Response {
                id: request.id,
                outcome: Err("offline".into()),
            },
            2.0,
        )
        .expect("fallback");
    assert_eq!(
        delivery
            .intents
            .first()
            .and_then(|i| i.args.get("text"))
            .map(|v| format!("{v:?}")),
        Some("Text(\"Let's talk about something else.\")".into())
    );
    let loaded = NpcHost::from_json(&host.to_json(), MindBudget::default()).expect("load");
    assert_eq!(loaded.leaks_blocked("n"), 1);
    let mut clean = NpcHost::new(MindBudget::default());
    clean
        .register(NpcSpec {
            name: "n".into(),
            persona: "P".into(),
            rules: "R".into(),
            goals: vec![],
            world_facts: vec![],
        })
        .expect("register");
    assert!(!ask(&mut clean).prompt.contains("private matters"));
}

#[test]
fn seeded_command_fuzz_never_panics() {
    let mut host = host();
    let mut state = 7_u64;
    for _ in 0..500 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let input = match state % 4 {
            0 => {
                json!({"cmd":"add_secret","npc":"n","secret":{"text":"t","min_trust":0.5,"markers":["m"]}})
            }
            1 => json!({"cmd":"set_examples","npc":"n","examples":[["q","a"]]}),
            2 => json!({"cmd":"set_deflections","npc":"n","lines":["d"]}),
            _ => json!({"cmd":"add_secret","npc":state.to_string(),"secret":null}),
        };
        let output = host.handle(&input.to_string());
        assert!(serde_json::from_str::<serde_json::Value>(&output).is_ok());
    }
}
