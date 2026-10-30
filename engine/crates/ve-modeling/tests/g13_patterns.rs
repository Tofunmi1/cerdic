//! Patterns (ADR 0060): `field.expr` makes a function of position from the expression language, `field.mask_from` turns a terrain layer
//! into one, and `scatter.place` takes one as `where`. Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g13_patterns").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

/// Where the copies of the first part stand: the translation of each placement matrix.
fn planted(source: &str) -> Vec<[f32; 3]> {
    let result = evaluate(source).unwrap();
    let transforms = result.parts[0].transforms.as_ref().unwrap();
    transforms.iter().map(|m| [m[12], m[13], m[14]]).collect()
}

const GROUND: &str = "t = terrain.tile@1(size=[128, 128], cell=2, seed=4, height=60, scale=160)\nb = solid.box@1(size=[1, 3, 1])\n";

fn plain() -> String {
    format!("{GROUND}f = scatter.place@1(b, t, radius=8, seed=2)\noutput f = f\n")
}

/// The same forest with a pattern linked as `where`; `code` is the expression text (no double quotes inside).
fn with_where(code: &str) -> String {
    format!(
        "{GROUND}w = field.expr@1(code=\"{code}\")\nf = scatter.place@1(b, t, w, radius=8, seed=2)\noutput f = f\n"
    )
}

#[test]
fn the_pattern_operators_are_registered_and_ready() {
    let registry = builtin_registry();
    let ready: Vec<String> = registry.ready().map(|spec| spec.id.to_string()).collect();
    for id in ["field.expr@1", "field.mask_from@1"] {
        assert!(
            ready.iter().any(|r| r == id),
            "{id} is a ready operator: {ready:?}"
        );
    }
}

#[test]
fn a_where_field_confines_where_things_are_planted() {
    let everywhere = planted(&plain());
    let right_half = planted(&with_where("out = step(64.0, p.x)"));
    assert!(
        !right_half.is_empty() && right_half.len() < everywhere.len(),
        "{} against {}",
        right_half.len(),
        everywhere.len()
    );
    assert!(
        right_half.iter().all(|p| p[0] >= 56.0),
        "nothing grows left of the edge (to within a few cells of blending)"
    );
    let share = right_half.len() as f32 / everywhere.len() as f32;
    assert!(
        (0.3..0.7).contains(&share),
        "about half as many plants: {share}"
    );
}

#[test]
fn a_field_that_is_one_changes_nothing_and_one_that_is_zero_plants_nothing() {
    assert_eq!(
        planted(&with_where("out = 1.0")),
        planted(&plain()),
        "a constant 1 is the same forest"
    );
    let none = evaluate(&with_where("out = 0.0")).unwrap();
    assert!(
        none.warnings.iter().any(|w| w.code == "scatter.nothing"),
        "{:?}",
        none.warnings
    );
    assert!(
        none.parts
            .iter()
            .all(|p| p.transforms.as_ref().is_none_or(Vec::is_empty))
    );
}

#[test]
fn fields_see_the_height_and_the_slope_of_the_ground_they_are_sampled_on() {
    let all = planted(&plain());
    let high = planted(&with_where("out = select(p.y > 30.0, 1.0, 0.0)"));
    assert!(!high.is_empty() && high.len() < all.len());
    assert!(
        high.iter().all(|p| p[1] > 26.0),
        "only high ground is planted (to within a cell of slope): lowest {}",
        high.iter().map(|p| p[1]).fold(f32::MAX, f32::min)
    );
    let flat = planted(&with_where("out = select(n.y > 0.98, 1.0, 0.0)"));
    assert!(
        !flat.is_empty() && flat.len() < all.len(),
        "{} flat-ground plants of {}",
        flat.len(),
        all.len()
    );
}

#[test]
fn a_terrain_layer_as_a_field_matches_the_layer_parameter_exactly() {
    let mask =
        "m = terrain.mask@1(t, mask=\"slope\", name=\"flat\", from=0, to=0.25, invert=true)\n";
    let by_layer = format!(
        "{GROUND}{mask}f = scatter.place@1(b, m, radius=8, seed=2, layer=\"flat\")\noutput f = f\n"
    );
    let by_field = format!(
        "{GROUND}{mask}w = field.mask_from@1(m, layer=\"flat\")\nf = scatter.place@1(b, t, w, radius=8, seed=2)\noutput f = f\n"
    );
    let (a, b) = (planted(&by_layer), planted(&by_field));
    assert!(!a.is_empty());
    assert_eq!(a, b, "the same density, so the same plants");
    let unknown = evaluate(&format!("{GROUND}{mask}w = field.mask_from@1(m, layer=\"nope\")\nf = scatter.place@1(b, t, w)\noutput f = f\n")).unwrap_err();
    assert!(
        unknown.contains("nope") && unknown.contains("terrain.mask"),
        "{unknown}"
    );
}

#[test]
fn patterns_feed_other_patterns_through_their_named_inputs() {
    let source = |code: &str| {
        format!(
            "{GROUND}a = field.expr@1(code=\"out = step(64.0, p.x)\")\nw = field.expr@1(a, code=\"{code}\")\nf = scatter.place@1(b, t, w, radius=8, seed=2)\noutput f = f\n"
        )
    };
    let same = planted(&source("out = a"));
    assert_eq!(
        same,
        planted(&with_where("out = step(64.0, p.x)")),
        "a pattern read back through its input is itself"
    );
    let thinned = planted(&source("out = a * 0.4"));
    assert!(
        thinned.len() < same.len() && !thinned.is_empty(),
        "{} against {}",
        thinned.len(),
        same.len()
    );
    let unlinked = evaluate(&format!(
        "{GROUND}w = field.expr@1(code=\"out = a\")\nf = scatter.place@1(b, t, w)\noutput f = f\n"
    ))
    .unwrap_err();
    assert!(
        unlinked.contains("expr.error") && unlinked.contains('a'),
        "an input that is not linked is an unknown name: {unlinked}"
    );
}

#[test]
fn bad_expressions_are_refused_with_the_place_and_a_suggestion() {
    let error = evaluate(&with_where("out = nope")).unwrap_err();
    assert!(
        error.contains("expr.error") && error.contains("nope") && error.contains("line 1"),
        "{error}"
    );
    let typo = evaluate(&with_where("out = sinn(1.0)")).unwrap_err();
    assert!(
        typo.contains("sin"),
        "the suggestion names the near miss: {typo}"
    );
    let vector = evaluate(&with_where("out = p")).unwrap_err();
    assert!(
        vector.contains("pattern.not_float"),
        "density needs a float: {vector}"
    );
}

#[test]
fn a_pattern_is_not_geometry_and_cannot_be_a_scene_output() {
    let error = evaluate("f = field.expr@1(code=\"out = 1.0\")\noutput f = f\n").unwrap_err();
    assert!(error.to_lowercase().contains("pattern"), "{error}");
}
