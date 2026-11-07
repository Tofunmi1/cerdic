//! `phys.ragdoll`: a skeleton of bone parts that turns into a ragdoll on command (plan W5, `docs/plans/engine-libraries-adoption.md`).
//!
//! The node records which bone parts make the ragdoll and how loose its joints are; after evaluation it is resolved against the bones (`rig.bone`
//! segments and `anim.parent` links), and a skeleton that cannot work is dropped with a reason. The runtime builds the bodies with
//! `ve_physics::ragdoll` from the characters' pose at the moment it is triggered and then poses the bone parts from the bodies.
use std::{collections::HashMap, rc::Rc};

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use crate::{EvaluationWarning, Fail, Part, cache::Value, fail, fail_suggest, rig::Segment};

/// Most bones in one ragdoll (the physics limit).
pub const MAX_RAGDOLL_BONES: usize = 32;

/// A ragdoll definition carried by a part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RagdollSpec {
    /// The name a game or script uses to trigger it (`physics.ragdoll`).
    pub name: String,
    /// Output names of the bone parts, parents before children.
    pub bones: Vec<String>,
    /// Output names of the bones that are hinges (elbows, knees) rather than sockets.
    pub hinge_names: Vec<String>,
    /// Each bone's rest segment (empty until resolved against the parts).
    #[serde(default)]
    pub segments: Vec<Segment>,
    /// Each bone's parent as an index into `bones` (empty until resolved).
    #[serde(default)]
    pub parents: Vec<Option<usize>>,
    /// Which bones are hinges (empty until resolved).
    #[serde(default)]
    pub hinges: Vec<bool>,
    /// Half-angle of the socket cone, radians.
    pub swing: f32,
    /// Twist either way about a bone, radians.
    pub twist: f32,
    /// Lowest and highest hinge angle, radians.
    pub hinge_limits: [f32; 2],
    /// The hinge axis when a hinge bone is straight at rest (a bent one uses the plane of its bend).
    pub hinge_axis: [f32; 3],
    /// Mass per cubic metre.
    pub density: f32,
    /// Resistance of the joints to moving.
    pub damping: f32,
    /// Capsule radius of every bone in metres; 0 sizes each from its length.
    pub radius: f32,
}

fn text<'a>(node: &'a Node, name: &str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(s)) => s.as_str(),
        _ => "",
    }
}

fn names(list: &str) -> Vec<String> {
    list.split([';', ','])
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect()
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "ragdoll settings are small and bounded by the spec"
)]
fn number(node: &Node, name: &str, default: f64) -> f32 {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .unwrap_or(default) as f32
}

fn list<const N: usize>(node: &Node, name: &str, default: [f32; N]) -> [f32; N] {
    let Some(Param::List(items)) = node.params.get(name) else {
        return default;
    };
    let mut out = default;
    for (slot, item) in out.iter_mut().zip(items) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ragdoll settings are small and bounded by the spec"
        )]
        {
            *slot = item.as_f64().unwrap_or(f64::from(*slot)) as f32;
        }
    }
    out
}

/// `phys.ragdoll`: makes the input shape the carrier of a ragdoll definition.
pub(crate) fn ragdoll(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            "phys.ragdoll needs a shape to carry it",
        )
    })?;
    let name = text(node, "name").trim().to_owned();
    if name.is_empty()
        || name.len() > 32
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(fail(
            "param.range",
            "phys.ragdoll name: use 1 to 32 letters, digits, _ or -; it is how a game finds the ragdoll",
        ));
    }
    let bones = names(text(node, "bones"));
    if bones.is_empty() || bones.len() > MAX_RAGDOLL_BONES {
        return Err(fail(
            "param.range",
            format!(
                "phys.ragdoll bones takes at least 1 and at most {MAX_RAGDOLL_BONES} bone part names (parents first), got {}",
                bones.len()
            ),
        ));
    }
    if let Some(twice) = bones
        .iter()
        .enumerate()
        .find_map(|(i, b)| bones[..i].contains(b).then_some(b))
    {
        return Err(fail(
            "param.range",
            format!("phys.ragdoll names bone \"{twice}\" twice"),
        ));
    }
    if input.ragdoll.is_some() {
        return Err(fail_suggest(
            "rig.duplicate",
            "a part can carry only one phys.ragdoll",
            "put each ragdoll on its own shape",
        ));
    }
    let mut value = (**input).clone();
    value.ragdoll = Some(RagdollSpec {
        name,
        bones,
        hinge_names: names(text(node, "hinges")),
        segments: Vec::new(),
        parents: Vec::new(),
        hinges: Vec::new(),
        swing: number(node, "swing", 1.0),
        twist: number(node, "twist", 0.5),
        hinge_limits: list(node, "hinge_limits", [0.0, 2.4]),
        hinge_axis: list(node, "hinge_axis", [1.0, 0.0, 0.0]),
        density: number(node, "density", 500.0),
        damping: number(node, "damping", 0.3),
        radius: number(node, "radius", 0.0),
    });
    Ok(value)
}

/// After evaluation: resolves every ragdoll against the bone parts (each bone must carry `rig.bone`, a parent comes before its children) and drops
/// the ones that cannot work, and a second ragdoll with a name already used, each with a warning.
pub(crate) fn resolve(parts: &mut [Part]) -> Vec<EvaluationWarning> {
    let by_name: HashMap<String, (Option<Segment>, Option<String>)> = parts
        .iter()
        .map(|part| (part.name.clone(), (part.bone, part.parent.clone())))
        .collect();
    let warn = |part: &Part, code: &str, message: String| EvaluationWarning {
        code: code.to_owned(),
        node: part.node.to_string(),
        related: Vec::new(),
        message,
    };
    let mut warnings = Vec::new();
    let mut used: Vec<String> = Vec::new();
    for part in parts.iter_mut() {
        let Some(mut spec) = part.ragdoll.take() else {
            continue;
        };
        if used.contains(&spec.name) {
            warnings.push(warn(
                part,
                "phys.ragdoll_name",
                format!(
                    "a ragdoll called \"{}\" already exists; rename this one",
                    spec.name
                ),
            ));
            continue;
        }
        let mut segments = Vec::new();
        let mut problem = None;
        for bone in &spec.bones {
            match by_name.get(bone) {
                Some((Some(segment), _)) => segments.push(*segment),
                _ => {
                    problem = Some((
                        "phys.ragdoll_bone_missing",
                        format!("\"{bone}\" is not an output part with a rig.bone"),
                    ));
                    break;
                }
            }
        }
        let mut parents = Vec::new();
        if problem.is_none() {
            for (index, bone) in spec.bones.iter().enumerate() {
                let parent = by_name
                    .get(bone)
                    .and_then(|(_, p)| p.as_ref())
                    .and_then(|p| spec.bones.iter().position(|b| b == p));
                if parent.is_some_and(|p| p >= index) {
                    problem = Some((
                        "phys.ragdoll_order",
                        format!("\"{bone}\" is listed before its parent; list bones parents first"),
                    ));
                    break;
                }
                parents.push(parent);
            }
        }
        if let Some((code, message)) = problem {
            warnings.push(warn(
                part,
                code,
                format!("{}: {message}; this ragdoll does nothing", spec.name),
            ));
            continue;
        }
        spec.hinges = spec
            .bones
            .iter()
            .map(|b| spec.hinge_names.contains(b))
            .collect();
        spec.segments = segments;
        spec.parents = parents;
        used.push(spec.name.clone());
        part.ragdoll = Some(spec);
    }
    warnings
}
