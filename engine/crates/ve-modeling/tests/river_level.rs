//! `river.carve level="descending"`: the river node's choice of the whole-path water profile (`ve_world::river::carve_river_descending`).
//! On stepped ground the default `level="local"` makes the water climb the steps; `descending` must not.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(15);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_river_level").unwrap(),
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

/// Terraced ridged ground (steps of several metres) with a river along the middle, west to east.
fn river(level: &str) -> String {
    format!(
        "t = terrain.tile@1(size=[256, 256], cell=2, seed=11, height=90, scale=110, octaves=4, roughness=0.55, style=\"ridged\", terraces=6, terrace_sharpness=0.9)\n\
         r = river.carve@1(t, path=\"0,128; 80,128; 170,128; 256,128\", width=10, depth=14, bank_slope=60, bank_width=16{level})\n\
         w = river.water@1(r)\noutput w = w\n"
    )
}

/// Mean water height in each 4 m column along x, over the vertices within 3 m of the river's centre line, and the spread inside each column.
fn columns(evaluation: &Evaluation) -> Vec<(f32, f32)> {
    let part = evaluation.parts.iter().find(|p| p.name == "w").unwrap();
    let mut bins: Vec<Vec<f32>> = vec![Vec::new(); 64];
    for p in &part.positions {
        if (p[2] - 128.0).abs() <= 3.0 {
            let bin = ((p[0] / 4.0) as usize).min(63);
            bins[bin].push(p[1]);
        }
    }
    bins.iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let mean = b.iter().sum::<f32>() / b.len() as f32;
            let spread = b.iter().copied().fold(f32::MIN, f32::max)
                - b.iter().copied().fold(f32::MAX, f32::min);
            (mean, spread)
        })
        .collect()
}

#[test]
fn the_default_level_is_unchanged() {
    let default = evaluate(&river("")).unwrap();
    let local = evaluate(&river(", level=\"local\"")).unwrap();
    let a = default.parts.iter().find(|p| p.name == "w").unwrap();
    let b = local.parts.iter().find(|p| p.name == "w").unwrap();
    assert_eq!(
        a.positions, b.positions,
        "an unset level means local, as before"
    );
}

#[test]
fn a_descending_river_never_climbs_the_terraces() {
    let columns = columns(&evaluate(&river(", level=\"descending\"")).unwrap());
    assert!(columns.len() > 20, "{} columns of water", columns.len());
    for pair in columns.windows(2) {
        assert!(
            pair[1].0 <= pair[0].0 + 0.15,
            "the water rose from {} to {}",
            pair[0].0,
            pair[1].0
        );
    }
}

#[test]
fn a_descending_river_is_level_across_its_width() {
    for (mean, spread) in columns(&evaluate(&river(", level=\"descending\"")).unwrap()) {
        assert!(
            spread < 0.15,
            "{spread} m of height difference across the channel at {mean}"
        );
    }
}

#[test]
fn on_terraced_ground_the_local_level_does_climb() {
    // The reason for the new mode: this fixture really does make the old water rise somewhere (so the test above means something).
    let columns = columns(&evaluate(&river(", level=\"local\"")).unwrap());
    assert!(
        columns.windows(2).any(|p| p[1].0 > p[0].0 + 0.5),
        "the fixture no longer shows the old bug; choose steeper terraces"
    );
}

#[test]
fn an_unknown_level_is_refused() {
    assert!(evaluate(&river(", level=\"uphill\"")).is_err());
}
