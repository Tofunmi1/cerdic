//! `body.garment` dresses a body.human: the part of its surface that a named garment covers (a shirt, trousers, shoes), lifted off the skin by a few
//! millimetres and skinned with the same weights, so clothes bend with the bones. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_g26_garment").unwrap(),
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

fn part<'a>(result: &'a Evaluation, name: &str) -> &'a ve_modeling::Part {
    result
        .parts
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("no part {name}"))
}

const WORN: &str = "\
body = body.human@1()
root_n = body.bone@1(body, bone=\"root\")
arm_n = body.bone@1(body, bone=\"upperarm01_l\")
arm_p = anim.parent@1(arm_n, parent=\"root\")
pants = body.garment@1(body, cover=\"pants\", offset=0.006, bones=\"root; upperarm01_l\")
shirt = body.garment@1(body, cover=\"shirt\", offset=0.008, bones=\"root; upperarm01_l\")
shoes = body.garment@1(body, cover=\"shoes\", offset=0.01)
output root = root_n
output upperarm01_l = arm_p
output body_part = body
output pants_part = pants
output shirt_part = shirt
output shoes_part = shoes
";

fn height_range(part: &ve_modeling::Part) -> (f32, f32) {
    part.positions
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        })
}

#[test]
fn each_garment_covers_its_own_part_of_the_body() {
    let result = evaluate(WORN).unwrap();
    let body = part(&result, "body_part");
    let (pants, shirt, shoes) = (
        part(&result, "pants_part"),
        part(&result, "shirt_part"),
        part(&result, "shoes_part"),
    );
    for garment in [pants, shirt, shoes] {
        assert!(!garment.indices.is_empty(), "a garment has surface");
        assert!(
            garment.indices.len() < body.indices.len(),
            "and covers less than the whole body"
        );
    }
    let (pants_lo, pants_hi) = height_range(pants);
    let (shirt_lo, shirt_hi) = height_range(shirt);
    let (shoes_lo, shoes_hi) = height_range(shoes);
    assert!(
        pants_lo < 0.15 && pants_hi > 0.85 && pants_hi < 1.15,
        "trousers run from the ankle to the waist: {pants_lo}..{pants_hi}"
    );
    assert!(
        shirt_lo > 0.8 && shirt_hi > 1.4,
        "a shirt runs from the hips to the neck: {shirt_lo}..{shirt_hi}"
    );
    assert!(
        shoes_hi < 0.2 && shoes_lo < 0.02,
        "shoes cover the feet: {shoes_lo}..{shoes_hi}"
    );
}

#[test]
fn a_garment_stands_off_the_skin_by_its_offset() {
    let result = evaluate(WORN).unwrap();
    let body = part(&result, "body_part");
    let shirt = part(&result, "shirt_part");
    // every garment vertex is above the skin (not inside it) and within a centimetre of it
    for p in shirt.positions.iter().step_by(37) {
        let nearest = body
            .positions
            .iter()
            .map(|q| ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt())
            .fold(f32::MAX, f32::min);
        assert!(
            nearest > 0.003 && nearest < 0.02,
            "garment vertex {p:?} is {nearest} m from the skin"
        );
    }
}

#[test]
fn garments_are_skinned_with_the_bodys_weights() {
    let result = evaluate(WORN).unwrap();
    let pants = part(&result, "pants_part");
    let skin = pants
        .skin
        .as_ref()
        .expect("a garment given bones is skinned");
    assert_eq!(skin.bones, vec!["root", "upperarm01_l"]);
    assert_eq!(skin.joints.len(), pants.positions.len());
    for w in &skin.weights {
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-4);
    }
    assert!(
        part(&result, "shoes_part").skin.is_none(),
        "without bones a garment is not skinned"
    );
}

#[test]
fn an_unknown_cover_names_the_choices() {
    let error =
        evaluate("body = body.human@1()\ng = body.garment@1(body, cover=\"hat\")\noutput g = g\n")
            .unwrap_err();
    assert!(
        error.contains("shirt") && error.contains("pants"),
        "{error}"
    );
}
