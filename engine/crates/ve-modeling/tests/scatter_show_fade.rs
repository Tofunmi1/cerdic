//! Camera distance cross-fades are carried as draw metadata and leave placement deterministic.
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
        transaction: TransactionId::parse("t_show_fade").unwrap(),
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
fn fade_is_carried_to_every_material_region_of_a_shape() {
    let text = "t = terrain.tile@1(size=[64, 64], cell=2, seed=4, height=60, scale=160)\nu = flora.tree@1(kind=\"oak\", height=6, seed=1, lod=4, sides=4)\nf = scatter.place@1(u, t, radius=4, seed=1, show_from=40, show_to=130, show_fade=10)\noutput f = f\n";
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
    assert!(parts.iter().all(|p| p.visible_fade == 10.0));
}

#[test]
fn fade_defaults_to_zero_and_survives_inst_vary() {
    assert_eq!(
        evaluate(&source(""))
            .unwrap()
            .parts
            .into_iter()
            .find(|p| p.name == "f")
            .unwrap()
            .visible_fade,
        0.0
    );
    let text = "t = terrain.tile@1(size=[64, 64], cell=2, seed=4, height=60, scale=160)\nb = solid.box@1(size=[0.5, 1, 0.5])\nf = scatter.place@1(b, t, radius=4, seed=1, show_from=40, show_to=130, show_fade=10)\nsc = field.expr@1(code=\"out = 0.8\")\nv = inst.vary@1(f, scale=sc)\noutput v = v\n";
    assert_eq!(
        evaluate(text)
            .unwrap()
            .parts
            .into_iter()
            .find(|p| p.name == "v")
            .unwrap()
            .visible_fade,
        10.0
    );
}

#[test]
fn fade_cannot_exceed_half_of_a_bounded_band() {
    let error = evaluate(&source(", show_from=40, show_to=130, show_fade=60")).unwrap_err();
    assert!(error.contains("show_fade"), "{error}");
}

#[test]
fn fade_does_not_move_the_plants() {
    assert_eq!(
        transforms(", show_from=40, show_to=130"),
        transforms(", show_from=40, show_to=130, show_fade=10")
    );
}
