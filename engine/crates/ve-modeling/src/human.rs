//! `body.human`: a person from the CC0 human pack (ADR 0055), evaluated from named phenotype sliders into a surface with UVs.
//!
//! The pack is about 9 MB, so it is not compiled in: the host hands it over once with [`set_human_pack`] (the browser worker fetches it the first time a
//! document needs it) and a node evaluated before that fails with `human.pack_missing`, which the host answers by loading it and trying again.
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::OnceLock;

use ve_graph::{Node, Value as Param};
use ve_human::{FitOptions, Human, Measure, Pack, Params, Target};
use ve_kernel::Mesh;

use crate::{Fail, cache::Value, fail, fail_suggest};

mod rig;
pub(crate) use rig::Rig;
use rig::{ball_for_bone, bone_source_indices};

static HUMAN: OnceLock<Human> = OnceLock::new();

/// Triangles `body.human` makes (two per quad of the 13,710-quad body), for the cost estimate.
pub(crate) const TRIANGLES: usize = 27_420;

/// Hands the evaluator the human pack (the bytes of `human.pack`, a zstd frame). Loading twice keeps the first.
///
/// # Errors
/// The pack is corrupt, truncated or of another version; the message says which.
pub fn set_human_pack(bytes: &[u8]) -> Result<(), String> {
    if HUMAN.get().is_some() {
        return Ok(());
    }
    let pack = Pack::from_zstd(bytes).map_err(|e| e.to_string())?;
    let _ = HUMAN.set(Human::new(pack));
    Ok(())
}

/// The loaded pack, for the nodes that read its skeleton and skin weights.
pub(crate) fn pack() -> Option<&'static Human> {
    HUMAN.get()
}

/// Whether [`set_human_pack`] has succeeded.
#[must_use]
pub fn human_pack_loaded() -> bool {
    HUMAN.get().is_some()
}

/// The skeleton and landmarks of an evaluated person, in the same space as its surface (standing on y = 0).
pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "body.human" => Some(build(node)),
        "body.eye" => Some(crate::human_eye::build(node, inputs)),
        "body.hair" => Some(crate::human_hair::build(node, inputs)),
        "body.bone" | "body.skin" | "body.skin_to" | "body.garment" | "body.outfit" => {
            crate::human_rig::run(node, inputs)
        }
        _ => None,
    }
}

/// The body as an editable polygon mesh: its 13,710 quads on the 13,718 shared vertices, with the UVs as corner attributes, so every `poly.*` node
/// (inset, extrude, subdivide, bevel, symmetrize, UV tools) takes the person as input like any other polygon mesh.
fn quad_mesh(human: &Human, positions: &[[f32; 3]]) -> Option<ve_geometry::PolyMesh> {
    use ve_geometry::{AttrData, Attribute, Domain, PolyMesh};
    let render = human.render_mesh();
    let quads = human.quads();
    let corners: Vec<u32> = quads.iter().flatten().copied().collect();
    // The render triangles are two per quad, (0, 1, 2) then (0, 2, 3), on UV-split vertices: they give each corner's UV.
    let (mut u, mut v) = (
        Vec::with_capacity(corners.len()),
        Vec::with_capacity(corners.len()),
    );
    for pair in render.triangles.chunks_exact(2) {
        let ids = [pair[0][0], pair[0][1], pair[0][2], pair[1][2]];
        for id in ids {
            let uv = render.uv.get(id as usize).copied().unwrap_or([0.0; 2]);
            u.push(uv[0]);
            v.push(uv[1]);
        }
    }
    let mut mesh = PolyMesh::new(positions.to_vec(), vec![4; quads.len()], corners).ok()?;
    // A body's quads are gently curved, not flat.
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    for (name, values) in [("u", u), ("v", v)] {
        mesh.set_attribute(
            name,
            Attribute {
                domain: Domain::Corner,
                data: AttrData::F32(values),
            },
        )
        .ok()?;
    }
    Some(mesh)
}

/// Turns a grounded point about the vertical axis through the origin, then moves it by `at`.
fn place(turn: f32, at: [f32; 3], p: [f32; 3]) -> [f32; 3] {
    let (sin, cos) = turn.sin_cos();
    [
        p[0] * cos + p[2] * sin + at[0],
        p[1] + at[1],
        -p[0] * sin + p[2] * cos + at[2],
    ]
}

fn slider(node: &Node, name: &str) -> Result<f32, Fail> {
    match node.params.get(name) {
        #[expect(clippy::cast_possible_truncation, reason = "sliders are 0 to 1")]
        Some(Param::Num(value)) => Ok(*value as f32),
        #[expect(clippy::cast_precision_loss, reason = "an integer slider is 0 or 1")]
        Some(Param::Int(value)) => Ok(*value as f32),
        _ => Err(fail(
            "param.type",
            format!("body.human {name} must be a number"),
        )),
    }
}

fn triple(node: &Node, name: &str) -> Result<[f32; 3], Fail> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return Ok([0.0; 3]);
    };
    let values: Vec<f64> = items.iter().filter_map(Param::as_f64).collect();
    #[expect(clippy::cast_possible_truncation, reason = "positions are metres")]
    match values.as_slice() {
        [a, b, c] => Ok([*a as f32, *b as f32, *c as f32]),
        _ => Err(fail(
            "param.type",
            format!("body.human {name} must be three numbers"),
        )),
    }
}

/// `name=value; name=value` into a map; a malformed pair names itself.
fn pairs(node: &Node, name: &str) -> Result<BTreeMap<String, f32>, Fail> {
    let text = match node.params.get(name) {
        Some(Param::Str(text)) => text.as_str(),
        _ => "",
    };
    let mut map = BTreeMap::new();
    for part in text
        .split([';', ','])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let (key, value) = part.split_once('=').ok_or_else(|| {
            fail_suggest(
                "param.format",
                format!("body.human {name}: `{part}` is not name=value"),
                format!("write {name} as `name=value; name=value`"),
            )
        })?;
        let value: f32 = value.trim().parse().map_err(|_| {
            fail(
                "param.format",
                format!("body.human {name}: `{}` is not a number", value.trim()),
            )
        })?;
        map.insert(key.trim().to_owned(), value);
    }
    Ok(map)
}

fn build(node: &Node) -> Result<Value, Fail> {
    let Some(human) = HUMAN.get() else {
        return Err(fail_suggest(
            "human.pack_missing",
            "the human body pack is not loaded",
            "load assets/human/human.pack into the evaluator (set_human_pack) and evaluate again",
        ));
    };
    let mut params = Params::default();
    for name in [
        "gender",
        "age",
        "height",
        "weight",
        "muscle",
        "proportions",
        "cupsize",
        "firmness",
        "african",
        "asian",
        "caucasian",
    ] {
        params
            .phenotype
            .insert(name.to_owned(), slider(node, name)?);
    }
    params.local = pairs(node, "local")?;
    params.face = pairs(node, "face")?;
    let mut warnings = Vec::new();
    let mut targets = Vec::new();
    for (name, measure) in [
        ("stature", Measure::Stature),
        ("waist", Measure::WaistCircumference),
        ("hip", Measure::HipCircumference),
        ("chest", Measure::ChestCircumference),
        ("mass", Measure::Mass),
    ] {
        let wanted = f64::from(slider(node, name)?);
        if wanted > 0.0 {
            targets.push(Target::new(measure, wanted));
        }
    }
    if !targets.is_empty() {
        let fitted = ve_human::fit(human, &params, &targets, &FitOptions::default())
            .map_err(|e| fail("human.fit", e.to_string()))?;
        // The sliders stay as authored in the document; the fit only decides what the body is evaluated with.
        let missed: Vec<String> = fitted
            .achieved
            .iter()
            .filter(|a| {
                (a.value - a.target).abs()
                    > if matches!(a.measure, Measure::Mass) {
                        2.0
                    } else {
                        0.02
                    }
            })
            .map(|a| {
                format!(
                    "{} asked {:.3}, reached {:.3}",
                    a.measure.name(),
                    a.target,
                    a.value
                )
            })
            .collect();
        if !missed.is_empty() {
            warnings.push(crate::EvaluationWarning {
                code: "human.fit_partial".to_owned(),
                node: node.id.to_string(),
                related: Vec::new(),
                message: format!(
                    "the body could not match every measurement ({}); relax them or change age, gender or the slider named by the nearest one",
                    missed.join("; ")
                ),
            });
        }
        params = fitted.params;
    }
    let body = human
        .evaluate(&params)
        .map_err(|e| fail("human.parameter", e.to_string()))?;
    let render = human.render_mesh();
    // Stand the person on y = 0: the lowest point rests on the ground.
    let floor = body
        .positions
        .iter()
        .fold(f32::MAX, |lowest, p| lowest.min(p[1]));
    // Then turn about the vertical axis through the origin and move to where the person stands (`turn` and `at`).
    let ground = |p: &[f32; 3]| [p[0], p[1] - floor, p[2]];
    let grounded: Vec<[f32; 3]> = body.positions.iter().map(ground).collect();
    let (turn, at) = (slider(node, "turn")?.to_radians(), triple(node, "at")?);
    let lower = |p: &[f32; 3]| place(turn, at, ground(p));
    let surface: Vec<[f32; 3]> = body.positions.iter().map(lower).collect();
    let head_source_indices = bone_source_indices(human, "head");
    let eye_source_indices = [
        bone_source_indices(human, "eye.L"),
        bone_source_indices(human, "eye.R"),
    ];
    let rig = Rig {
        turn,
        at,
        head_points: head_source_indices
            .iter()
            .filter_map(|index| grounded.get(*index).copied())
            .collect(),
        head_source_indices,
        bone_names: human.bone_names().to_vec(),
        bone_parents: human.bone_parents(),
        bone_heads: body.bone_heads.iter().map(lower).collect(),
        eyes: [
            ball_for_bone(human, &surface, "eye.L"),
            ball_for_bone(human, &surface, "eye.R"),
        ],
        eye_source_indices,
    };
    let positions: Vec<[f32; 3]> = body.render_positions.iter().map(lower).collect();
    let indices: Vec<u32> = render.triangles.iter().flatten().copied().collect();
    let uvs: Vec<[f32; 2]> = indices
        .iter()
        .filter_map(|&i| render.uv.get(i as usize).copied())
        .collect();
    let mut value = Value::solid(Mesh { positions, indices }, 0.0);
    value.uvs = uvs;
    value.poly = quad_mesh(human, &surface).map(Rc::new);
    value.human_source_indices = render.source.clone();
    value.poly_human_source_indices = (0..surface.len())
        .map(|index| u32::try_from(index).unwrap_or(u32::MAX))
        .collect();
    value.human = Some(Rc::new(rig));
    value.warnings = warnings;
    Ok(value)
}
