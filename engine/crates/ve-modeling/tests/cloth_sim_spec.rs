//! `cloth.sim`: the description of live cloth that reaches the runtime. The simulation itself runs in the viewer; here is what the evaluation
//! hands it: pins, the bones it follows and collides with, and refusals for what cannot be simulated. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_cloth_sim").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A 1 m by 1 m garment of 12 by 12 quads hanging in the XY plane, its top row selected as pins, and an arm bone part named `arm`.
const SCENE: &str = "grid = poly.grid@1(width=1, height=1, columns=12, rows=12)\nhung = xform.move@1(grid, by=[0, 1.5, 0])\ntop = poly.select@1(hung, element=\"vertex\", by=\"box\", box_min=[-1, 1.99, -1], box_max=[1, 2.01, 1])\nbox = solid.box@1(size=[0.1, 0.1, 0.1])\narm_shape = xform.move@1(box, by=[0.8, 1.2, 0])\narm = rig.bone@1(arm_shape, head=[0.8, 1.2, 0], tail=[0.8, 0.7, 0])\noutput arm = arm";

fn cloth_part(result: &Evaluation, name: &str) -> Option<ve_modeling::cloth_sim::ClothSpec> {
    result
        .parts
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.cloth.clone())
}

#[test]
fn the_garment_part_carries_its_pins_and_settings() {
    let source = format!(
        "{SCENE}\ncape = cloth.sim@1(hung, top, pin_bone=\"arm\", wind=[2, 0, 0], drag=0.5, bend=0.1)\noutput cape = cape\n"
    );
    let result = evaluate(&source).unwrap();
    let spec = cloth_part(&result, "cape").unwrap();
    assert_eq!(
        spec.pins.len(),
        13,
        "the top row of a 12 by 12 quad garment has 13 points"
    );
    assert_eq!(spec.pin_bone, "arm");
    assert_eq!(spec.wind, [2.0, 0.0, 0.0]);
    assert!((spec.bend - 0.1).abs() < 1e-6 && (spec.drag - 0.5).abs() < 1e-6);
    assert_eq!(spec.floor, None, "no floor unless asked for");
}

#[test]
fn collider_bones_are_resolved_to_their_rest_segments() {
    let source = format!(
        "{SCENE}\ncape = cloth.sim@1(hung, top, collide_bones=\"arm\", collider_radius=0.09)\noutput cape = cape\n"
    );
    let result = evaluate(&source).unwrap();
    let spec = cloth_part(&result, "cape").unwrap();
    assert_eq!(spec.collide_bones, vec!["arm".to_owned()]);
    assert_eq!(spec.segments.len(), 1);
    assert_eq!(spec.segments[0].head, [0.8, 1.2, 0.0]);
    assert_eq!(spec.segments[0].tail, [0.8, 0.7, 0.0]);
    assert!((spec.radius - 0.09).abs() < 1e-6);
}

#[test]
fn a_pin_bone_that_is_not_a_part_warns_and_the_cloth_is_dropped() {
    let source = format!(
        "{SCENE}\ncape = cloth.sim@1(hung, top, pin_bone=\"nobody\")\noutput cape = cape\n"
    );
    let result = evaluate(&source).unwrap();
    assert!(cloth_part(&result, "cape").is_none());
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.code == "cloth.pin_bone" && w.message.contains("nobody")),
        "{:?}",
        result.warnings
    );
}

#[test]
fn a_collider_that_is_not_a_bone_is_ignored_with_a_warning() {
    let source = format!(
        "{SCENE}\ncape = cloth.sim@1(hung, top, collide_bones=\"arm, cape\")\noutput cape = cape\n"
    );
    let result = evaluate(&source).unwrap();
    let spec = cloth_part(&result, "cape").unwrap();
    assert_eq!(
        spec.collide_bones,
        vec!["arm".to_owned()],
        "the real bone stays"
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.code == "cloth.collide_bone" && w.message.contains("cape")),
        "{:?}",
        result.warnings
    );
}

#[test]
fn a_garment_with_its_own_animation_is_refused_with_the_fix() {
    let source = format!(
        "{SCENE}\nmoving = anim.track@1(hung, property=\"position\", keys=\"0:0,0,0; 1:0,0.5,0\")\ncape = cloth.sim@1(moving, top)\noutput cape = cape\n"
    );
    let error = evaluate(&source).err().unwrap();
    assert_eq!(error.0, "cloth.animated");
}

#[test]
fn a_surface_put_on_a_garment_keeps_it_cloth() {
    let source = format!(
        "{SCENE}\ncape = cloth.sim@1(hung, top, pin_bone=\"arm\")\nred = solid.material@1(cape, color=[0.8, 0.1, 0.1], roughness=0.6)\noutput cape = red\n"
    );
    let result = evaluate(&source).unwrap();
    assert!(cloth_part(&result, "cape").is_some());
}

#[test]
fn the_spec_is_in_the_part_json_the_page_sends_to_the_runtime() {
    let source = format!(
        "{SCENE}\ncape = cloth.sim@1(hung, top, pin_bone=\"arm\", collide_bones=\"arm\")\noutput cape = cape\n"
    );
    let result = evaluate(&source).unwrap();
    let part = result.parts.iter().find(|p| p.name == "cape").unwrap();
    let json = serde_json::to_value(part).unwrap();
    assert_eq!(json["cloth"]["pin_bone"], "arm");
    assert_eq!(json["cloth"]["pins"].as_array().unwrap().len(), 13);
    assert_eq!(json["cloth"]["segments"].as_array().unwrap().len(), 1);
}

#[test]
fn pin_numbers_name_the_top_row_of_the_part_the_runtime_receives() {
    let source =
        format!("{SCENE}\ncape = cloth.sim@1(hung, top, pin_bone=\"arm\")\noutput cape = cape\n");
    let result = evaluate(&source).unwrap();
    let part = result.parts.iter().find(|p| p.name == "cape").unwrap();
    let spec = part.cloth.as_ref().unwrap();
    assert_eq!(
        part.positions.len(),
        13 * 13,
        "welded: one point per grid point, not one per corner"
    );
    for &pin in &spec.pins {
        let y = part.positions[pin as usize][1];
        assert!(
            (y - 2.0).abs() < 1e-4,
            "pin {pin} is at y {y}, not on the top edge"
        );
    }
    assert!(
        part.indices
            .iter()
            .all(|&i| (i as usize) < part.positions.len())
    );
}

#[test]
fn a_garment_with_too_many_points_is_refused_with_the_limit() {
    let source = "grid = poly.grid@1(width=1, height=1, columns=100, rows=100)\ncape = cloth.sim@1(grid)\noutput cape = cape\n";
    let error = evaluate(source).err().unwrap();
    assert_eq!(error.0, "budget.cloth");
    assert!(error.1.contains("6000"), "{}", error.1);
}

#[test]
fn a_selection_of_faces_cannot_be_the_pins() {
    let source = format!(
        "{SCENE}\nfaces = poly.select@1(hung, element=\"face\", by=\"all\")\ncape = cloth.sim@1(hung, faces)\noutput cape = cape\n"
    );
    let error = evaluate(&source).err().unwrap();
    assert_eq!(error.0, "poly.selection_kind");
}
