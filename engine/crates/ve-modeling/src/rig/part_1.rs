use std::{collections::HashMap, rc::Rc};

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use crate::{EvaluationWarning, Fail, Part, cache::Value, fail, fail_suggest};

mod dual;
pub mod paint;

pub use dual::{Blend, DualPose, deform_dual, deform_prepared, prepare as prepare_dual};
use paint::Stamp;

/// Most bones one skin may use.
pub const MAX_BONES: usize = 32;
/// Most bones that may move one vertex.
pub const MAX_INFLUENCES: usize = 4;
/// Most skinned parts in a scene.
pub const MAX_SKINNED_PARTS: usize = 8;
/// Most skinned vertices in a scene.
pub const MAX_SKINNED_VERTICES: usize = 131_072;

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
}

/// Most chains in a scene.
pub const MAX_IK_CHAINS: usize = 8;

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
    let input = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            "rig.ik needs a shape to use as the target",
        )
    })?;
    let Some(Param::Str(list)) = node.params.get("bones") else {
        return Err(fail("param.type", "rig.ik needs bones"));
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
                format!("rig.ik names bone \"{name}\" twice"),
            ));
        }
        bones.push(name.to_owned());
    }
    if bones.is_empty() || bones.len() > crate::ik::MAX_CHAIN {
        return Err(fail(
            "param.range",
            format!(
                "rig.ik needs 1 to {} bones from root to tip, got {}",
                crate::ik::MAX_CHAIN,
                bones.len()
            ),
        ));
    }
    if input.ik.is_some() {
        return Err(fail(
            "rig.duplicate",
            "a part can be the target of only one rig.ik",
        ));
    }
    let mut value = (**input).clone();
    value.ik = Some(IkChain {
        bones,
        iterations: int(node, "iterations", 32).clamp(1, crate::ik::MAX_ITERATIONS),
        segments: Vec::new(),
    });
    Ok(value)
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
