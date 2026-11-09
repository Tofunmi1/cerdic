//! Ground-flora kits (`wildflower`, `fern`, `mushroom`, `log`, `stump`, `reed`, `flowering_bush`, `rock`): each evaluates cleanly, stands on the
//! ground, has two or more material regions where it should, and its exposed colours really change what is drawn.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture failures include the model diagnostic")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, PLANT_KITS, Quality};

fn run(name: &str, args: &str) -> Evaluation {
    let kit = PLANT_KITS.iter().find(|k| k.name == name).unwrap_or_else(|| panic!("no kit named {name}"));
    let source = format!("{}\nitem = {name}({args})\noutput item", kit.source);
    let parsed = parse_text(&builtin_registry(), &source).unwrap_or_else(|e| panic!("{name} parse errors: {e:?}"));
    let mut session = Session::with_builtin(1602);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g16_ground").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{name}: {outcome:?}");
    Evaluator::default().evaluate(session.document(), Quality::Final).unwrap_or_else(|e| panic!("{name}: {} {}", e.code, e.message))
}

fn colours(result: &Evaluation) -> Vec<[f32; 3]> {
    result.parts.iter().filter_map(|p| p.material.as_ref()).map(|m| m.color).collect()
}

const ALL: [&str; 8] = ["wildflower", "fern", "mushroom", "log", "stump", "reed", "flowering_bush", "rock"];

#[test]
fn every_ground_kit_evaluates_cleanly_and_stands_on_the_ground() {
    for name in ALL {
        let result = run(name, "");
        assert!(result.warnings.is_empty(), "{name}: {:?}", result.warnings);
        let low = result.parts.iter().flat_map(|p| &p.positions).map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let high = result.parts.iter().flat_map(|p| &p.positions).map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        assert!(low > -0.2 && low < 0.15, "{name} stands on y = 0, lowest point {low}");
        assert!(high > 0.05 && high < 2.0, "{name} is a ground plant, highest point {high}");
    }
}

#[test]
fn flowers_mushrooms_and_blossoming_bushes_have_a_region_for_each_colour() {
    for name in ["wildflower", "mushroom", "flowering_bush"] {
        let result = run(name, "");
        assert!(result.parts.len() >= 2, "{name} has at least a plant region and a flower or cap region, got {}", result.parts.len());
    }
}

#[test]
fn the_exposed_colours_change_what_is_drawn() {
    let pairs = [
        ("wildflower", "petal_color=[0.1, 0.2, 0.9]", [0.1, 0.2, 0.9]),
        ("mushroom", "cap_color=[0.9, 0.8, 0.1]", [0.9, 0.8, 0.1]),
        ("flowering_bush", "flower_color=[0.2, 0.3, 0.95]", [0.2, 0.3, 0.95]),
        ("reed", "head_color=[0.9, 0.1, 0.1]", [0.9, 0.1, 0.1]),
    ];
    for (name, args, want) in pairs {
        let used = colours(&run(name, args));
        assert!(used.iter().any(|c| c.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-3)), "{name}: {args} reaches a material, got {used:?}");
    }
}

#[test]
fn a_different_seed_gives_a_different_rock_and_log() {
    for (name, a, b) in [("rock", "seed=1", "seed=9"), ("log", "seed=1", "seed=9"), ("flowering_bush", "seed=1", "seed=9")] {
        let (one, two) = (run(name, a), run(name, b));
        let points = |r: &Evaluation| r.parts.iter().flat_map(|p| p.positions.iter().copied()).collect::<Vec<_>>();
        assert_ne!(points(&one), points(&two), "{name}: the seed changes the shape");
    }
}
