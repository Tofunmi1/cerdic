//! Non-repeating sampling parameters travel from language nodes to material results.
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Material, Quality};

fn evaluate(op: &str, params: &str) -> Result<Material, String> {
    let shape = if op == "terrain.material" {
        "terrain.tile@1(size=[8,8], cell=2)"
    } else {
        "solid.box@1(size=[2,2,2])"
    };
    let source = format!(
        "b = {shape}\nm = {op}@1(b{params})\nr = xform.move@1(m, by=[0,1,0])\noutput r = r"
    );
    let commands = parse_text(&builtin_registry(), &source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(31);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_stochastic").map_err(|e| format!("{e:?}"))?,
        label: None,
        commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{outcome:?}"));
    }
    let evaluation = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))?;
    evaluation
        .parts
        .first()
        .and_then(|p| p.material.clone())
        .ok_or_else(|| "missing material".to_owned())
}

const TEXTURE: &str = ", program=\"n = noise(scale=4); albedo = n\", tile=2";

#[test]
fn texture_defaults_off_terrain_defaults_on() -> Result<(), String> {
    let texture = evaluate("solid.texture", TEXTURE)?;
    let terrain = evaluate("terrain.material", "")?;
    assert!(!texture.no_repeat);
    assert!(terrain.no_repeat);
    assert_eq!(texture.no_repeat_scale, 1.0);
    assert_eq!(terrain.no_repeat_scale, 1.0);
    Ok(())
}

#[test]
fn both_nodes_carry_explicit_flags_and_scales_through_transforms() -> Result<(), String> {
    let texture = evaluate(
        "solid.texture",
        &format!("{TEXTURE}, no_repeat=true, no_repeat_scale=4"),
    )?;
    let terrain = evaluate(
        "terrain.material",
        ", no_repeat=false, no_repeat_scale=0.25",
    )?;
    assert!(texture.no_repeat);
    assert_eq!(texture.no_repeat_scale, 4.0);
    assert!(!terrain.no_repeat);
    assert_eq!(terrain.no_repeat_scale, 0.25);
    assert!(terrain.finish.and_then(|f| f.terrain).is_some());
    Ok(())
}

#[test]
fn scales_outside_the_documented_range_and_non_boolean_flags_are_rejected() {
    for (op, base) in [("solid.texture", TEXTURE), ("terrain.material", "")] {
        for bad in [
            "no_repeat_scale=0.24",
            "no_repeat_scale=8.01",
            "no_repeat=2",
        ] {
            assert!(
                evaluate(op, &format!("{base}, {bad}")).is_err(),
                "{op}: {bad}"
            );
        }
    }
}

#[test]
fn saved_results_keep_the_settings_and_older_results_load() -> Result<(), String> {
    let material = evaluate(
        "solid.texture",
        &format!("{TEXTURE}, no_repeat=true, no_repeat_scale=8"),
    )?;
    let mut json = serde_json::to_value(&material).map_err(|e| e.to_string())?;
    let restored: Material = serde_json::from_value(json.clone()).map_err(|e| e.to_string())?;
    assert_eq!(restored, material);
    let object = json.as_object_mut().ok_or("expected object")?;
    object.remove("no_repeat");
    object.remove("no_repeat_scale");
    let legacy: Material = serde_json::from_value(json).map_err(|e| e.to_string())?;
    assert!(!legacy.no_repeat);
    assert_eq!(legacy.no_repeat_scale, 1.0);
    Ok(())
}
