//! The text parsers behind `anim.track`, `anim.wave`, `anim.parent` and `behavior.on` take model-written strings.
//! Whatever they are given they must return a result (never panic, hang or accept out-of-range numbers).
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality, flags};

/// A small deterministic generator, so a failure can be replayed.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

const PIECES: &[&str] = &[
    "0", "1", "-1", "2.5", "1e9", "-1e9", "nan", "inf", "0.0001", "999999", ":", ",", ";", ";;",
    " ", "=", "!", "on", "off", "toggle", "a", "key", "Power", "x_y", "9x", "é", "\"", "\\", "\n",
    "", "0:0,0,0", "1:1,1,1", "0:1", "name=on", "!alarm", "a=b=c",
];

fn random_text(rng: &mut Lcg) -> String {
    (0..(rng.next() % 12)).map(|_| rng.pick(PIECES)).collect()
}

#[test]
fn flag_parsers_never_panic_and_only_accept_valid_names() {
    let mut rng = Lcg(7);
    for _ in 0..5000 {
        let text = random_text(&mut rng);
        if let Ok(sets) = flags::parse_sets(&text) {
            assert!(sets.len() <= flags::MAX_PER_BEHAVIOR, "{text:?}");
            assert!(
                sets.iter().all(|s| flags::name(&s.name).is_ok()),
                "{text:?}"
            );
        }
        if let Ok(needs) = flags::parse_needs(&text) {
            assert!(needs.len() <= flags::MAX_PER_BEHAVIOR, "{text:?}");
            assert!(
                needs.iter().all(|n| flags::name(&n.name).is_ok()),
                "{text:?}"
            );
        }
        let _ = flags::name(&text);
    }
}

fn evaluate(text: &str) -> Result<usize, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_fuzz").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { .. } = outcome {
        return Err("rejected".to_owned());
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map(|evaluation| {
            evaluation
                .parts
                .iter()
                .map(|p| p.animation.iter().map(|t| t.keys.len()).sum::<usize>())
                .sum()
        })
        .map_err(|e| e.message)
}

#[test]
fn animation_nodes_survive_junk_params_and_keep_accepted_keys_in_range() {
    let mut rng = Lcg(11);
    let mut accepted = 0;
    for _ in 0..400 {
        let property = rng.pick(&["position", "rotation", "scale", "glow", "visible", "color"]);
        let keys = random_text(&mut rng).replace(['"', '\\', '\n'], "");
        let sets = random_text(&mut rng).replace(['"', '\\', '\n'], "");
        let program = format!(
            "b = solid.box@1(size=[1, 1, 1])\nt = anim.track@1(b, property=\"{property}\", keys=\"{keys}\")\n\
             r = behavior.on@1(t, sets=\"{sets}\")\noutput o = r\n"
        );
        if let Ok(count) = evaluate(&program) {
            accepted += 1;
            assert!(
                (2..=64).contains(&count),
                "accepted {keys:?} with {count} keys"
            );
        }
    }
    // Some random programs are valid; the point is that none of the others crash.
    assert!(accepted < 400);
}
