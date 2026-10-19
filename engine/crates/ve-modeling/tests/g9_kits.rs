//! B7: the shipped kits parse, evaluate inside their stated footprint, stay deterministic and survive save/reload.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture failures include the model diagnostic rather than hiding it"
)]
use ve_graph::{
    CommandBatch, Document, Outcome, Session, TransactionId, builtin_registry, parse_text,
};
use ve_modeling::{Evaluator, KITS, Quality};

fn session(program: &str) -> Session {
    let parsed = parse_text(&builtin_registry(), program)
        .unwrap_or_else(|errors| panic!("parse errors: {errors:?}"));
    let mut session = Session::with_builtin(908);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_kits").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    session
}

fn kit(name: &str, call: &str) -> String {
    let kit = KITS.iter().find(|kit| kit.name == name).unwrap();
    format!("{}\n{call}\noutput item", kit.source)
}

fn evaluate(session: &Session) -> ve_modeling::Evaluation {
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("{}: {}", error.code, error.message))
}

/// Bounding box over every part, as `[min, max]` per axis.
fn bounds(result: &ve_modeling::Evaluation) -> [[f32; 2]; 3] {
    let mut out = [[f32::INFINITY, f32::NEG_INFINITY]; 3];
    for part in &result.parts {
        for point in &part.positions {
            for (axis, range) in out.iter_mut().enumerate() {
                range[0] = range[0].min(point[axis]);
                range[1] = range[1].max(point[axis]);
            }
        }
    }
    out
}

#[test]
fn every_kit_evaluates_inside_its_footprint() {
    let limits: [(&str, [f32; 3]); 3] = [
        ("desk", [1.4, 0.76, 0.7]),
        ("shelf", [0.85, 1.8, 0.3]),
        ("window", [1.0, 1.2, 0.12]),
    ];
    assert!(
        KITS.len() >= limits.len(),
        "the original kits must stay registered"
    );
    for (name, size) in limits {
        let result = evaluate(&session(&kit(name, &format!("item = {name}()"))));
        assert!(!result.parts.is_empty(), "{name} produced nothing");
        let found = bounds(&result);
        for axis in 0..3 {
            let extent = found[axis][1] - found[axis][0];
            assert!(
                (extent - size[axis]).abs() < 0.03,
                "{name} axis {axis}: {extent} vs {}",
                size[axis]
            );
        }
    }
}

#[test]
fn shelf_arrangement_seed_is_stable_and_changes_the_books() {
    let render = |seed: u32| {
        bounds(&evaluate(&session(&kit(
            "shelf",
            &format!("item = shelf(arrangement={seed})"),
        ))))
    };
    assert_eq!(render(4), render(4));
    let again = evaluate(&session(&kit("shelf", "item = shelf(arrangement=4)")));
    let other = evaluate(&session(&kit("shelf", "item = shelf(arrangement=5)")));
    let volume = |r: &ve_modeling::Evaluation| r.parts.iter().map(|p| p.volume).sum::<f64>();
    let first: Vec<_> = again.parts.iter().map(|p| p.positions.clone()).collect();
    let second: Vec<_> = other.parts.iter().map(|p| p.positions.clone()).collect();
    assert_ne!(first, second, "a different seed must move the books");
    assert!((volume(&again) - volume(&other)).abs() < 1.0e-3);
}

#[test]
fn kits_save_and_reload_with_identical_geometry() {
    for kit_info in KITS {
        let source = session(&kit(kit_info.name, &format!("item = {}()", kit_info.name)));
        let before = evaluate(&source);
        let saved = source.document().canonical_json();
        let restored: Document = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.canonical_json(), saved);
        let after = evaluate(&Session::new(builtin_registry(), restored));
        assert_eq!(before.parts.len(), after.parts.len());
        for (a, b) in before.parts.iter().zip(&after.parts) {
            assert_eq!(a.positions, b.positions, "{}", kit_info.name);
        }
    }
}
