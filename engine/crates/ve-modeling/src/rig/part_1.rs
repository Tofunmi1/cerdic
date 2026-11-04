use std::{collections::HashMap, rc::Rc};

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use crate::{EvaluationWarning, Fail, Part, cache::Value, fail, fail_suggest};

mod dual;
pub mod paint;

pub use dual::{Blend, DualPose, deform_dual, deform_prepared, prepare as prepare_dual};
use paint::Stamp;

/// Most bones one skin may use.
pub const MAX_BONES: usize = 64;
/// Most bones that may move one vertex.
pub const MAX_INFLUENCES: usize = 4;
/// Most skinned parts in a scene.
pub const MAX_SKINNED_PARTS: usize = 32;
/// Most skinned vertices (triangle corners) in a scene. The Bevy renderer skins on the GPU, so this is the scene triangle budget
/// (200,000 triangles) with room to spare, not a CPU cost; it was 131,072 while every frame deformed the meshes on the CPU.
pub const MAX_SKINNED_VERTICES: usize = 1_200_000;

/// The rest segment of a bone, in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    /// Where the bone starts (its joint).
    pub head: [f32; 3],
    /// Where the bone ends.
    pub tail: [f32; 3],
}

/// A skinned part: the bones that move it and, after evaluation, each vertex's weights.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skinning {
    /// Output names of the bone parts, in joint-index order (valid bones only once weighed).
    pub bones: Vec<String>,
    /// How sharply influence falls with distance (a whole number from 1 to 8).
    pub falloff: u32,
    /// How many bones may move one vertex (1 to 4).
    pub influences: u32,
    /// How the bones' motions are combined.
    #[serde(default)]
    pub blend: Blend,
    /// Brush stamps applied to the distance weights.
    #[serde(default)]
    pub paint: Vec<Stamp>,
    /// Per vertex, the indices into `bones` of its influences (unused slots repeat index 0 with weight 0).
    #[serde(skip)]
    pub joints: Vec<[u16; 4]>,
    /// Per vertex, the influence weights, summing to 1.
    #[serde(skip)]
    pub weights: Vec<[f32; 4]>,
}

/// A chain of bone parts, root to tip, that should reach the part carrying this value (the target), solved each frame by
/// [`crate::ik`]. The bones' rest segments are filled in after evaluation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IkChain {
    /// Output names of the bone parts, root first; each is the parent of the next.
    pub bones: Vec<String>,
    /// Solver passes per frame.
    pub iterations: u32,
    /// The rest segment of each bone (empty until the chain is resolved against the parts).
    #[serde(default)]
    pub segments: Vec<Segment>,
    /// Set by `rig.secondary`: the chain does not reach for a target but follows its root loosely (a tail, ear or hair), by springs.
    #[serde(default)]
    pub secondary: Option<Secondary>,
    /// Set by `rig.gait`: the target is not authored by hand but walks, planted on the ground and swinging forward as its body moves.
    #[serde(default)]
    pub gait: Option<GaitSettings>,
    /// Set when `solver` is `two_bone`: the chain (exactly two bones) is solved analytically with a pole, soften and twist.
    #[serde(default)]
    pub two_bone: Option<TwoBoneSettings>,
    /// Set by `rig.aim`: a single bone that turns to look at the target.
    #[serde(default)]
    pub aim: Option<AimSettings>,
    /// The most any one bone of a `ccd` chain may turn from its rest pose, in degrees (180 is no limit): a knee that cannot bend backwards, a tentacle
    /// that cannot fold double.
    #[serde(default = "full_turn")]
    pub limit_degrees: f32,
}

fn full_turn() -> f32 {
    180.0
}

/// How a `rig.gait` foot target walks (see `ve-gait`): the part it belongs to, where in the cycle it starts, and the stride.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GaitSettings {
    /// Output name of the body part whose motion the foot follows.
    pub body: String,
    /// Where in the gait cycle this foot starts, 0 to 1 (alternate legs 0 and 0.5).
    pub phase: f32,
    /// Metres the body travels per full cycle of this foot.
    pub stride_length: f32,
    /// How high the foot lifts in its swing, in metres.
    pub step_height: f32,
    /// The share of the cycle the foot is planted, 0.1 to 1.
    pub duty: f32,
    /// The farthest the foot may be from its hip, in metres; 0 means the length of its leg.
    pub reach: f32,
    /// The ground height, in metres.
    pub ground: f32,
}

/// How a `rig.secondary` chain moves: stiffness 0 to 1 (how hard it springs back to its rest pose), damping per second, and gravity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Secondary {
    /// 0 hangs free, 1 returns quickly to the rest pose.
    pub stiffness: f32,
    /// Velocity lost to drag, per second.
    pub damping: f32,
    /// Gravity in m/s^2 (world).
    pub gravity: [f32; 3],
}

/// Most chains in a scene.
pub const MAX_IK_CHAINS: usize = 24;

fn triple(node: &Node, name: &str) -> [f32; 3] {
    let Some(Param::List(items)) = node.params.get(name) else {
        return [0.0; 3];
    };
    let mut out = [0.0_f32; 3];
    for (slot, item) in out.iter_mut().zip(items) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "bone coordinates are bounded metres"
        )]
        {
            *slot = item.as_f64().unwrap_or(0.0) as f32;
        }
    }
    out
}

fn int(node: &Node, name: &str, default: u32) -> u32 {
    match node.params.get(name) {
        Some(Param::Int(value)) => u32::try_from(*value).unwrap_or(default),
        Some(Param::Num(value)) if *value >= 0.0 => {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "bounded by the spec"
            )]
            let whole = *value as u32;
            whole
        }
        _ => default,
    }
}

/// Records the bone segment on the input.
pub(crate) fn bone(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "rig.bone needs a shape"))?;
    let (head, tail) = (triple(node, "head"), triple(node, "tail"));
    if head.iter().zip(&tail).all(|(a, b)| (a - b).abs() < 1e-6) {
        return Err(fail(
            "param.range",
            "rig.bone head and tail must be different points",
        ));
    }
    let mut value = (**input).clone();
    value.bone = Some(Segment { head, tail });
    Ok(value)
}

/// Marks the input as skinned by the named bone parts.
pub(crate) fn skin(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "rig.skin needs a shape"))?;
    let Some(Param::Str(list)) = node.params.get("bones") else {
        return Err(fail("param.type", "rig.skin needs bones"));
    };
    let mut bones: Vec<String> = Vec::new();
    for name in list
        .split([';', ','])
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if bones.iter().any(|known| known == name) {
            return Err(fail(
                "param.range",
                format!("rig.skin names bone \"{name}\" twice"),
            ));
        }
        bones.push(name.to_owned());
    }
    if bones.is_empty() || bones.len() > MAX_BONES {
        return Err(fail_suggest(
            "param.range",
            format!("rig.skin needs 1 to {MAX_BONES} bones, got {}", bones.len()),
            format!("name between 1 and {MAX_BONES} bone parts in `bones`"),
        ));
    }
    if input.skin.is_some() {
        return Err(fail("rig.duplicate", "a part can carry only one rig.skin"));
    }
    let blend = match node.params.get("blend") {
        Some(Param::Str(name)) if name == "dual_quaternion" => Blend::DualQuaternion,
        _ => Blend::Linear,
    };
    let paint = match node.params.get("paint") {
        Some(Param::Str(text)) => {
            paint::parse(text).map_err(|message| fail("param.range", message))?
        }
        _ => Vec::new(),
    };
    let mut value = (**input).clone();
    value.skin = Some(Skinning {
        bones,
        falloff: int(node, "falloff", 3).clamp(1, 8),
        influences: int(node, "influences", 4).clamp(1, 4),
        blend,
        paint,
        joints: Vec::new(),
        weights: Vec::new(),
    });
    Ok(value)
}

/// Marks the input as the target that a chain of bones reaches for.
pub(crate) fn ik(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    chain_node(node, inputs, "rig.ik", None)
}

fn chain_node(
    node: &Node,
    inputs: &[Rc<Value>],
    label: &str,
    secondary: Option<Secondary>,
) -> Result<Value, Fail> {
    let input = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            format!("{label} needs a shape to carry the chain"),
        )
    })?;
    let Some(Param::Str(list)) = node.params.get("bones") else {
        return Err(fail("param.type", format!("{label} needs bones")));
    };
    let mut bones: Vec<String> = Vec::new();
    for name in list
        .split([';', ','])
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if bones.iter().any(|known| known == name) {
            return Err(fail(
                "param.range",
                format!("{label} names bone \"{name}\" twice"),
            ));
        }
        bones.push(name.to_owned());
    }
    if bones.is_empty() || bones.len() > crate::ik::MAX_CHAIN {
        return Err(fail(
            "param.range",
            format!(
                "{label} needs 1 to {} bones from root to tip, got {}",
                crate::ik::MAX_CHAIN,
                bones.len()
            ),
        ));
    }
    if input.ik.is_some() {
        return Err(fail(
            "rig.duplicate",
            "a part can carry only one rig.ik or rig.secondary",
        ));
    }
    let two_bone = matches!(node.params.get("solver"), Some(Param::Str(name)) if name == "two_bone");
    if two_bone && bones.len() != 2 {
        return Err(fail(
            "param.range",
            format!(
                "{label} solver \"two_bone\" needs exactly two bones (an arm or a leg: upper; lower), got {}; use solver \"ccd\" for other lengths",
                bones.len()
            ),
        ));
    }
    let mut value = (**input).clone();
    value.ik = Some(IkChain {
        bones,
        iterations: if secondary.is_some() {
            1
        } else {
            int(node, "iterations", 32).clamp(1, crate::ik::MAX_ITERATIONS)
        },
        segments: Vec::new(),
        secondary,
        gait: None,
        two_bone: two_bone.then(|| TwoBoneSettings {
            pole: triple(node, "pole"),
            soften: number(node, "soften", 0.0).clamp(0.0, 1.0),
            twist: number(node, "twist", 0.0).clamp(-6.3, 6.3),
        }),
        aim: None,
        limit_degrees: number(node, "limit", 180.0).clamp(1.0, 180.0),
    });
    Ok(value)
}

/// Makes the foot target (a part that already carries `rig.ik`) walk: planted on the ground while its body passes over it, then swinging forward.
pub(crate) fn gait(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "rig.gait needs a foot target made with rig.ik"))?;
    let Some(chain) = input.ik.as_ref().filter(|chain| chain.secondary.is_none()) else {
        return Err(fail(
            "rig.gait_needs_ik",
            "rig.gait goes on a foot target that already has rig.ik: put it after rig.ik",
        ));
    };
    let Some(Param::Str(body)) = node.params.get("body") else {
        return Err(fail("param.type", "rig.gait needs body, the output name of the body part"));
    };
    let body = body.trim();
    if body.is_empty() {
        return Err(fail("param.range", "rig.gait body must name a part"));
    }
    let mut value = (**input).clone();
    value.ik = Some(IkChain {
        gait: Some(GaitSettings {
            body: body.to_owned(),
            phase: number(node, "phase", 0.0).clamp(0.0, 1.0),
            stride_length: number(node, "stride_length", 0.6).clamp(0.05, 20.0),
            step_height: number(node, "step_height", 0.15).clamp(0.0, 5.0),
            duty: number(node, "duty", 0.6).clamp(0.1, 1.0),
            reach: number(node, "reach", 0.0).clamp(0.0, 50.0),
            ground: number(node, "ground", 0.0).clamp(-1000.0, 1000.0),
        }),
        ..chain.clone()
    });
    Ok(value)
}

fn number(node: &Node, name: &str, default: f32) -> f32 {
    match node.params.get(name) {
        Some(Param::Num(value)) => {
            #[expect(clippy::cast_possible_truncation, reason = "bounded by the spec")]
            let narrow = *value as f32;
            narrow
        }
        Some(Param::Int(value)) => {
            #[expect(clippy::cast_precision_loss, reason = "bounded by the spec")]
            let narrow = *value as f32;
            narrow
        }
        _ => default,
    }
}

/// Marks the input as the carrier of a chain of bones that follows its root by springs (a tail, an ear, hair): the chain is solved
/// each frame by `ve-chain` and replaces the bones' own motion.
pub(crate) fn secondary(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let settings = Secondary {
        stiffness: number(node, "stiffness", 0.3).clamp(0.0, 1.0),
        damping: number(node, "damping", 4.0).clamp(0.0, 50.0),
        gravity: [0.0, -number(node, "gravity", 9.8).clamp(0.0, 50.0), 0.0],
    };
    chain_node(node, inputs, "rig.secondary", Some(settings))
}

/// Resolves every chain against the bone parts: each bone must carry `rig.bone`, be the parent of the next, and start where
/// the one before ends. A chain that cannot work is dropped with a warning, so the viewer only ever sees usable ones.
pub(crate) fn resolve_ik(parts: &mut [Part]) -> Vec<EvaluationWarning> {
    let mut warnings = Vec::new();
    let by_name: HashMap<String, (Option<Segment>, Option<String>)> = parts
        .iter()
        .map(|part| (part.name.clone(), (part.bone, part.parent.clone())))
        .collect();
    let mut kept = 0;
    for part in parts.iter_mut() {
        let Some(chain) = part.ik.take() else {
            continue;
        };
        let problem = chain_problem(&chain, &by_name);
        if let Some((code, message)) = problem {
            warnings.push(warning(
                part,
                code,
                format!("{}: {message}; the target does not move the arm", part.name),
            ));
            continue;
        }
        kept += 1;
        if kept > MAX_IK_CHAINS {
            warnings.push(warning(
                part,
                "rig.ik_budget",
                format!(
                    "a scene may solve at most {MAX_IK_CHAINS} chains; {} is ignored",
                    part.name
                ),
            ));
            continue;
        }
        let segments = chain
            .bones
            .iter()
            .filter_map(|name| by_name.get(name).and_then(|(segment, _)| *segment))
            .collect();
        part.ik = Some(IkChain { segments, ..chain });
    }
    warnings
}
