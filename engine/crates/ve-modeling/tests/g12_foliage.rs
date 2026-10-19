//! `solid.foliage`: cutout and wind settings on a surface, kept with its colour and texture, on every part of a fused solid.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helpers unwrap fixtures that are known to be valid and report failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Foliage, Quality};

fn run(text: &str) -> Result<Evaluation, (String, Option<String>)> {
    let batch = parse_text(&builtin_registry(), text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_foliage").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.suggestion.clone()))
}

fn ok(text: &str) -> Evaluation {
    run(text).unwrap_or_else(|(code, _)| panic!("{code}"))
}

const CARD: &str = "card = solid.box@1(size=[0.4, 0.01, 0.4])\n\
    paint = solid.material@1(card, color=[0.2, 0.5, 0.1], roughness=0.8)\n";

#[test]
fn the_settings_land_on_the_surface() {
    let result = ok(&format!(
        "{CARD}leaf = solid.foliage@1(paint, alpha_cutoff=0.4, sway=0.3, flutter=0.2)\noutput leaf"
    ));
    let material = result.parts[0].material.as_ref().unwrap();
    assert_eq!(
        material.foliage,
        Some(Foliage {
            alpha_cutoff: 0.4,
            sway: 0.3,
            flutter: 0.2
        })
    );
}

#[test]
fn defaults_cut_at_half_and_do_not_move() {
    let result = ok(&format!("{CARD}leaf = solid.foliage@1(paint)\noutput leaf"));
    let foliage = result.parts[0].material.as_ref().unwrap().foliage.unwrap();
    assert!((foliage.alpha_cutoff - 0.5).abs() < f32::EPSILON);
    assert!(foliage.sway.abs() < f32::EPSILON && foliage.flutter.abs() < f32::EPSILON);
}

#[test]
fn colour_roughness_and_texture_are_kept() {
    let result = ok(&format!(
        "{CARD}textured = solid.texture@1(paint, program=\"n = fbm(scale_x=4, scale_y=4); albedo = n\")\n\
         leaf = solid.foliage@1(textured, sway=0.5)\noutput leaf"
    ));
    let material = result.parts[0].material.as_ref().unwrap();
    assert!((material.color[1] - 0.5).abs() < 1e-6 && (material.roughness - 0.8).abs() < 1e-6);
    assert!(material.texture_program.is_some());
    assert!(material.foliage.is_some());
}

#[test]
fn everything_at_zero_is_an_ordinary_surface_again() {
    let result = ok(&format!(
        "{CARD}leaf = solid.foliage@1(paint, sway=0.4)\nplain = solid.foliage@1(leaf, alpha_cutoff=0, sway=0, flutter=0)\noutput plain"
    ));
    assert_eq!(result.parts[0].material.as_ref().unwrap().foliage, None);
}

#[test]
fn a_bare_shape_gets_a_default_surface_with_the_setting() {
    let result =
        ok("box = solid.box@1(size=[1, 1, 1])\nleaf = solid.foliage@1(box, sway=0.2)\noutput leaf");
    assert!(result.parts[0].material.as_ref().unwrap().foliage.is_some());
}

#[test]
fn every_colour_of_a_fused_solid_gets_it() {
    let result = ok("a = solid.box@1(size=[1, 1, 1])\n\
         red = solid.material@1(a, color=[0.8, 0.1, 0.1], roughness=0.5)\n\
         b = solid.box@1(size=[1, 1, 1])\n\
         b2 = xform.move@1(b, by=[0.5, 0, 0])\n\
         green = solid.material@1(b2, color=[0.1, 0.8, 0.1], roughness=0.5)\n\
         both = solid.union@1(red, green)\n\
         leaf = solid.foliage@1(both, alpha_cutoff=0.3)\noutput leaf");
    assert!(result.parts.len() >= 2, "{} parts", result.parts.len());
    for part in &result.parts {
        let foliage = part.material.as_ref().unwrap().foliage.unwrap();
        assert!((foliage.alpha_cutoff - 0.3).abs() < 1e-6);
    }
}

#[test]
fn a_placement_set_keeps_the_setting_on_its_prototype() {
    let result = ok(&format!(
        "{CARD}ring = inst.radial@1(paint, count=6, radius=1)\nleaf = solid.foliage@1(ring, sway=0.6)\noutput leaf"
    ));
    let part = &result.parts[0];
    let placements = part.transforms.as_ref().map_or(0, Vec::len);
    assert_eq!(placements, 6, "six placements share the prototype");
    assert!((part.material.as_ref().unwrap().foliage.unwrap().sway - 0.6).abs() < 1e-6);
}

#[test]
fn a_setting_survives_a_json_round_trip_and_old_results_load_without_it() {
    let result = ok(&format!(
        "{CARD}leaf = solid.foliage@1(paint, alpha_cutoff=0.35, sway=0.1, flutter=0.9)\noutput leaf"
    ));
    let material = result.parts[0].material.as_ref().unwrap();
    let json = serde_json::to_string(material).unwrap();
    let back: ve_modeling::Material = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, material);
    // A material saved before foliage existed has no such field and reads as an ordinary surface.
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value.as_object_mut().unwrap().remove("foliage");
    let old: ve_modeling::Material = serde_json::from_value(value).unwrap();
    assert_eq!(old.foliage, None);
}
