//! `body.outfit`: real clothes and hair from the imported CC0 MakeHuman library, bound to the body so they follow its sliders, skinned with its weights, and never sunk into it.
//! (Written after the node, not before: the node was found by trying the assets on the body; these tests pin what was seen.)
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality, set_human_pack};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g32_outfit").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| {
            format!(
                "{} {} {}",
                e.code,
                e.message,
                e.suggestion.clone().unwrap_or_default()
            )
        })
}

fn dressed(body_params: &str, outfit_params: &str) -> (ve_modeling::Part, ve_modeling::Part) {
    let source = format!(
        "body = body.human@1({body_params})\nroot_n = body.bone@1(body, bone=\"root\")\nsuit = body.outfit@1(body, asset=\"female_casualsuit01\"{outfit_params})\noutput root = root_n\noutput body_part = body\noutput suit_part = suit\n"
    );
    let result = evaluate(&source).unwrap();
    let take = |name: &str| {
        result
            .parts
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .clone()
    };
    (take("body_part"), take("suit_part"))
}

fn nearest(body: &ve_modeling::Part, p: [f32; 3]) -> f32 {
    body.positions
        .iter()
        .map(|q| ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt())
        .fold(f32::MAX, f32::min)
}

#[test]
fn the_garment_is_the_real_mesh_with_its_own_uvs() {
    let (_, suit) = dressed("", "");
    assert_eq!(suit.indices.len() / 3, 4236, "the artist's triangles");
    assert_eq!(
        suit.uvs.len(),
        suit.indices.len(),
        "one UV per triangle corner"
    );
    assert!(
        suit.positions
            .iter()
            .all(|p| p.iter().all(|c| c.is_finite()))
    );
}

#[test]
fn the_garment_never_sinks_into_the_skin() {
    let (body, suit) = dressed("", ", standoff=0.012");
    for p in suit.positions.iter().step_by(17) {
        assert!(
            nearest(&body, *p) > 0.003,
            "garment vertex {p:?} is {} m from the skin",
            nearest(&body, *p)
        );
    }
}

#[test]
fn the_garment_follows_the_height_slider() {
    let top =
        |part: &ve_modeling::Part| part.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    let (_, tall) = dressed("height=0.9", "");
    let (_, short) = dressed("height=0.1", "");
    assert!(
        top(&tall) > top(&short) + 0.08,
        "{} against {}",
        top(&tall),
        top(&short)
    );
}

#[test]
fn a_skinned_garment_takes_the_bodys_weights() {
    let source = "body = body.human@1()\nroot_n = body.bone@1(body, bone=\"root\")\nsuit = body.outfit@1(body, asset=\"female_casualsuit01\", bones=\"root; spine05; spine04; spine03; spine02; spine01; head\")\noutput root = root_n\noutput suit = suit\n";
    let result = evaluate(source).unwrap();
    let suit = result.parts.iter().find(|p| p.name == "suit").unwrap();
    let skin = suit.skin.as_ref().unwrap();
    assert_eq!(skin.weights.len(), suit.positions.len());
    assert!(
        skin.weights
            .iter()
            .all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 1e-3)
    );
}

#[test]
fn an_unknown_asset_is_refused_with_the_names() {
    let error = evaluate(
        "body = body.human@1()\ns = body.outfit@1(body, asset=\"top_hat\")\noutput s = s\n",
    )
    .err()
    .unwrap();
    assert!(
        error.contains("female_casualsuit01") && error.contains("long01"),
        "{error}"
    );
}
