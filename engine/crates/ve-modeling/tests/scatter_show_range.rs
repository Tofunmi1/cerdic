//! Camera distance bands are carried as draw metadata and leave placement deterministic.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
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
        transaction: TransactionId::parse("t_show_range").unwrap(),
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

fn source(params: &str) -> String {
    format!(
        "t = terrain.tile@1(size=[64, 64], cell=2, seed=4, height=60, scale=160)\nb = solid.box@1(size=[0.5, 1, 0.5])\nf = scatter.place@1(b, t, radius=4, seed=1{params})\noutput f = f\n"
    )
}

fn transforms(params: &str) -> Vec<[f32; 16]> {
    evaluate(&source(params))
        .unwrap()
        .parts
        .into_iter()
        .find(|p| p.name == "f")
        .unwrap()
        .transforms
        .unwrap()
}

#[test]
fn inst_vary_keeps_the_range() {
    let text = "t = terrain.tile@1(size=[64, 64], cell=2, seed=4, height=60, scale=160)\nb = solid.box@1(size=[0.5, 1, 0.5])\nf = scatter.place@1(b, t, radius=4, seed=1, show_from=45, show_to=140)\nsc = field.expr@1(code=\"out = 0.8\")\nv = inst.vary@1(f, scale=sc)\noutput v = v\n";
    let range = evaluate(text)
        .unwrap()
        .parts
        .into_iter()
        .find(|p| p.name == "v")
        .unwrap()
        .visible_range;
    assert_eq!(range, Some((45.0, 140.0)));
}

#[test]
fn every_material_region_of_a_shape_keeps_the_range() {
    // A shape of two colours is drawn as two parts (regions) of one output; each must carry the band.
    let text = "t = terrain.tile@1(size=[64, 64], cell=2, seed=4, height=60, scale=160)\nu = flora.tree@1(kind=\"oak\", height=6, seed=1, lod=4, sides=4)\nf = scatter.place@1(u, t, radius=4, seed=1, show_from=45, show_to=140)\noutput f = f\n";
    let parts: Vec<_> = evaluate(text)
        .unwrap()
        .parts
        .into_iter()
        .filter(|p| p.name == "f")
        .collect();
    assert!(
        parts.len() >= 2,
        "expected two regions, got {}",
        parts.len()
    );
    assert!(parts.iter().all(|p| p.visible_range == Some((45.0, 140.0))));
}

#[test]
fn no_show_params_means_no_range() {
    let result = evaluate(&source("")).unwrap();
    assert_eq!(
        result
            .parts
            .iter()
            .find(|p| p.name == "f")
            .unwrap()
            .visible_range,
        None
    );
}

#[test]
fn show_params_are_carried_to_the_part() {
    let range = evaluate(&source(", show_from=45, show_to=140"))
        .unwrap()
        .parts
        .into_iter()
        .find(|p| p.name == "f")
        .unwrap()
        .visible_range;
    assert_eq!(range, Some((45.0, 140.0)));
    let range = evaluate(&source(", show_from=140"))
        .unwrap()
        .parts
        .into_iter()
        .find(|p| p.name == "f")
        .unwrap()
        .visible_range;
    assert_eq!(range, Some((140.0, f32::INFINITY)));
}

#[test]
fn show_range_does_not_move_the_plants() {
    assert_eq!(transforms(""), transforms(", show_from=45, show_to=140"));
}

#[test]
fn show_to_below_show_from_is_refused() {
    let error = evaluate(&source(", show_from=50, show_to=20")).unwrap_err();
    assert!(error.contains("show_to"), "{error}");
}
