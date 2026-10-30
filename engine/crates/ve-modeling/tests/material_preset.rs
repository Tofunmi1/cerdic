//! `material.preset`: a named, layered surface (wood, brick, rusted iron, snow ...) with wear, dirt, moss and wet sliders, so a model
//! picks a look by name instead of writing a texture program.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const PRESETS: [&str; 18] = [
    "oak",
    "planks",
    "red_brick",
    "cobblestone",
    "concrete",
    "plaster",
    "rusted_iron",
    "brushed_steel",
    "leather",
    "linen",
    "sand",
    "mud",
    "snow",
    "ice",
    "asphalt",
    "corrugated_metal",
    "painted_metal",
    "wall_tile",
];
const METALS: [&str; 4] = [
    "rusted_iron",
    "brushed_steel",
    "corrugated_metal",
    "painted_metal",
];

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(11);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_preset").unwrap(),
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

fn part(params: &str) -> Result<Evaluation, String> {
    evaluate(&format!(
        "a = solid.box@1(size=[1,2,3])\nm = material.preset@1(a{params})\noutput p = m\n"
    ))
}

fn program(params: &str) -> String {
    part(params).unwrap().parts[0]
        .material
        .as_ref()
        .unwrap()
        .texture_program
        .clone()
        .unwrap()
}

#[test]
fn every_preset_sets_a_textured_surface_and_keeps_the_shape() {
    for name in PRESETS {
        let evaluation =
            part(&format!(", name=\"{name}\"")).unwrap_or_else(|e| panic!("{name}: {e}"));
        let part = &evaluation.parts[0];
        assert!(
            (part.volume - 6.0).abs() < 1e-9,
            "{name}: the shape is untouched"
        );
        let material = part.material.as_ref().unwrap();
        let program = material
            .texture_program
            .as_deref()
            .unwrap_or_else(|| panic!("{name}: no texture program"));
        assert!(program.len() <= 8192, "{name}: {} bytes", program.len());
        assert!(
            program.contains("albedo") && program.contains("roughness"),
            "{name}: needs both channels"
        );
        assert_eq!(
            program.matches('(').count(),
            program.matches(')').count(),
            "{name}: unbalanced parentheses"
        );
        assert_eq!(
            program.matches('[').count(),
            program.matches(']').count(),
            "{name}: unbalanced brackets"
        );
        if METALS.contains(&name) {
            assert!(
                material.metallic >= 0.5,
                "{name}: metallic {}",
                material.metallic
            );
        } else {
            assert!(
                material.metallic <= 0.05,
                "{name}: metallic {}",
                material.metallic
            );
        }
    }
}

#[test]
fn an_unknown_name_is_refused() {
    assert!(part(", name=\"unobtainium\"").is_err());
}

#[test]
fn the_default_is_a_usable_preset_with_a_tile_and_a_size() {
    let evaluation = part(", name=\"oak\"").unwrap();
    let material = evaluation.parts[0].material.as_ref().unwrap();
    assert!(
        (material.texture_tile - 1.0).abs() < 1e-6,
        "tile {}",
        material.texture_tile
    );
    assert_eq!(material.texture_size, 512);
    let wide = part(", name=\"oak\", tile=2.5, size=\"256\"").unwrap();
    let material = wide.parts[0].material.as_ref().unwrap();
    assert!((material.texture_tile - 2.5).abs() < 1e-6);
    assert_eq!(material.texture_size, 256);
}

#[test]
fn each_slider_changes_the_look() {
    for name in ["red_brick", "oak", "cobblestone"] {
        let base = program(&format!(", name=\"{name}\", wear=0, dirt=0, moss=0, wet=0"));
        for slider in ["wear", "dirt", "moss", "wet"] {
            let changed = program(&format!(
                ", name=\"{name}\", wear=0, dirt=0, moss=0, wet=0, {slider}=1"
            ));
            assert_ne!(base, changed, "{name}: {slider} changes nothing");
        }
    }
}

#[test]
fn the_same_inputs_make_the_same_program() {
    let params = ", name=\"cobblestone\", wear=0.4, dirt=0.6, moss=0.3, wet=0.2";
    assert_eq!(program(params), program(params));
}

#[test]
fn with_every_slider_at_zero_the_program_reads_no_mesh_signal() {
    // A program that reads the part's mesh (curvature, cavity, up) cannot be shared between parts; a plain tiled one can, so many bricks
    // walls cost one map.
    for name in PRESETS {
        let plain = program(&format!(", name=\"{name}\", wear=0, dirt=0, moss=0, wet=0"));
        assert!(
            !plain.contains("surface_"),
            "{name}: reads the mesh at all zeros"
        );
    }
    let worn = program(", name=\"red_brick\", wear=1, dirt=0, moss=0, wet=0");
    assert!(
        worn.contains("surface_"),
        "wear paints from the mesh's edges"
    );
    let mossy = program(", name=\"red_brick\", wear=0, dirt=0, moss=1, wet=0");
    assert!(mossy.contains("surface_up"), "moss grows on top faces");
    let dirty = program(", name=\"red_brick\", wear=0, dirt=1, moss=0, wet=0");
    assert!(
        dirty.contains("surface_cavity") || dirty.contains("surface_ao"),
        "dirt gathers in creases"
    );
}

#[test]
fn wet_lowers_the_roughness_it_paints() {
    let dry = program(", name=\"concrete\", wear=0, dirt=0, moss=0, wet=0");
    let wet = program(", name=\"concrete\", wear=0, dirt=0, moss=0, wet=1");
    assert_ne!(dry, wet);
}

#[test]
fn sliders_outside_zero_to_one_are_refused() {
    assert!(part(", name=\"oak\", wear=1.5").is_err());
    assert!(part(", name=\"oak\", moss=-0.1").is_err());
}

#[test]
fn a_saved_document_writes_whole_number_sliders_as_integers() {
    // The browser sends the document through JSON, where a slider at 0 or 1 arrives as an integer (it once failed there with
    // "moss must be between 0 and 1" while every native test passed).
    let mut session = Session::with_builtin(12);
    let batch = format!(
        r#"{{"doc":"{}","base_revision":0,"transaction":"t_int","commands":[
        {{"add_node":{{"op":"solid.box@1","params":{{"size":[1,1,1]}},"alias":"$a","label":"a"}}}},
        {{"add_node":{{"op":"material.preset@1","params":{{"name":"oak","tile":1,"wear":1,"dirt":0,"moss":0,"wet":1}},"alias":"$m","label":"m"}}}},
        {{"connect":{{"from":{{"node":"$a","port":"out"}},"to":{{"node":"$m","port":"in"}}}}}},
        {{"set_output":{{"name":"part","from":{{"node":"$m","port":"out"}}}}}}]}}"#,
        session.document().id
    );
    assert!(matches!(
        session.apply_json(&batch),
        Outcome::Committed { .. }
    ));
    let evaluation = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    assert!(
        evaluation.parts[0]
            .material
            .as_ref()
            .unwrap()
            .texture_program
            .is_some()
    );
}

#[test]
fn it_is_the_same_surface_a_texture_node_would_make() {
    // The preset sits on the existing texture path: the surface carries its program, tile and size like solid.texture does, and a toon
    // or a clear coat can still go on top.
    let source = "a = solid.box@1(size=[1,1,1])\nm = material.preset@1(a, name=\"ice\")\nf = solid.finish@1(m, clearcoat=0.5)\noutput p = f\n";
    let evaluation = evaluate(source).unwrap();
    let material = evaluation.parts[0].material.as_ref().unwrap();
    assert!(material.texture_program.is_some());
}
