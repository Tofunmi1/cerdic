//! `body.bone` and `body.skin`: the skeleton and the skin weights of a `body.human`, as parts and a skin the animation system already understands.
//!
//! A bone is a tiny part hidden in the body at a named joint, carrying a `rig.bone` rest segment from that joint to the next one. The skin is the
//! pack's own per-vertex weights (nine bones each, from the MakeHuman/Anny data), cut to the strongest four among the bones that were listed; a bone
//! that was not listed gives its weight to its nearest listed ancestor, so an animator lists only the joints that move.
use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::Mesh;

use crate::human::Rig;
use crate::rig::{MAX_BONES, Segment, Skinning};
use crate::{Fail, cache::Value, fail, fail_suggest};

mod garment;
mod hem;
mod outfit;
mod skin_to;

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "body.bone" => Some(bone(node, inputs)),
        "body.skin" => Some(skin(node, inputs)),
        "body.skin_to" => Some(skin_to::run(node, inputs)),
        "body.garment" => Some(garment::run(node, inputs)),
        "body.outfit" => Some(outfit::run(node, inputs)),
        _ => None,
    }
}

/// A pack bone name as a part name: lowercase, `.` and `-` written `_` (`upperarm01.L` is `upperarm01_l`, `finger2-1.R` is `finger2_1_r`).
fn clean(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '.' || c == '-' {
                '_'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

fn text<'a>(node: &'a Node, name: &str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(value)) => value.trim(),
        _ => "",
    }
}

fn rig_of(node: &Node, inputs: &[Rc<Value>]) -> Result<(Rc<Value>, Rc<Rig>), Fail> {
    let family = node.op.family();
    let body = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            format!("{} needs a body.human", family.as_str()),
        )
    })?;
    let rig = body.human.clone().ok_or_else(|| {
        fail_suggest(
            "human.needs_body",
            format!(
                "{} needs the output of body.human as its input",
                family.as_str()
            ),
            "connect the body.human node itself, before solid.material",
        )
    })?;
    Ok((Rc::clone(body), rig))
}

fn unknown(rig: &Rig, name: &str) -> Fail {
    let names: Vec<String> = rig.bone_names.iter().map(|n| clean(n)).collect();
    fail_suggest(
        "param.invalid",
        format!("{name:?} is not a bone of the body"),
        format!("the bones are: {}", names.join(", ")),
    )
}

/// Where a bone ends: the mean of its children's heads that are not on top of it, else a stub continuing from its parent.
fn tail_of(rig: &Rig, index: usize) -> [f32; 3] {
    let head = rig.bone_heads[index];
    let children: Vec<[f32; 3]> = rig
        .bone_parents
        .iter()
        .enumerate()
        .filter(|(_, parent)| usize::try_from(**parent).is_ok_and(|p| p == index))
        .map(|(child, _)| rig.bone_heads[child])
        .filter(|h| distance(*h, head) > 0.004)
        .collect();
    if !children.is_empty() {
        let mut mean = [0.0_f32; 3];
        for child in &children {
            for axis in 0..3 {
                mean[axis] += child[axis];
            }
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a bone has a handful of children"
        )]
        let count = children.len() as f32;
        return mean.map(|v| v / count);
    }
    // A leaf (a fingertip, a toe, the head): continue in the direction the parent's bone ran, a little shorter.
    let parent = usize::try_from(rig.bone_parents[index]).ok();
    let direction = parent.map_or([0.0, 0.1, 0.0], |p| {
        let from = rig.bone_heads[p];
        [head[0] - from[0], head[1] - from[1], head[2] - from[2]]
    });
    let length = distance(direction, [0.0; 3]).max(1e-6);
    let stub = (length * 0.8).max(0.02);
    std::array::from_fn(|axis| head[axis] + direction[axis] / length * stub)
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// A 4 mm cube at `at`: small enough to hide inside the body.
fn speck(at: [f32; 3]) -> Mesh {
    const H: f32 = 0.002;
    let corner = |x: f32, y: f32, z: f32| [at[0] + x * H, at[1] + y * H, at[2] + z * H];
    let positions = vec![
        corner(-1.0, -1.0, -1.0),
        corner(1.0, -1.0, -1.0),
        corner(1.0, 1.0, -1.0),
        corner(-1.0, 1.0, -1.0),
        corner(-1.0, -1.0, 1.0),
        corner(1.0, -1.0, 1.0),
        corner(1.0, 1.0, 1.0),
        corner(-1.0, 1.0, 1.0),
    ];
    let indices = vec![
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 1, 2, 6, 1, 6, 5,
        0, 4, 7, 0, 7, 3,
    ];
    Mesh { positions, indices }
}

fn bone(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (_, rig) = rig_of(node, inputs)?;
    let name = text(node, "bone");
    let index = rig
        .bone_names
        .iter()
        .position(|n| clean(n) == name)
        .ok_or_else(|| unknown(&rig, name))?;
    let head = rig.bone_heads[index];
    let tail = tail_of(&rig, index);
    let mut value = Value::solid(speck(head), 0.0);
    value.bone = Some(Segment { head, tail });
    Ok(value)
}

/// The pack's weights for every vertex of the body's surface, cut to the strongest four of the listed bones (see the module docs).
fn weights_for(
    rig: &Rig,
    human: &ve_human::Human,
    bones: &[String],
    source_indices: &[u32],
) -> Result<(Vec<[u16; 4]>, Vec<[f32; 4]>), Fail> {
    let weights = human.skin();
    if source_indices.is_empty()
        || source_indices
            .iter()
            .any(|source| *source as usize >= weights.len())
    {
        return Err(fail(
            "human.skin_mesh",
            "the body's surface no longer has a valid mapping to the human pack's skin weights",
        ));
    }
    weights_from(
        rig,
        bones,
        source_indices
            .iter()
            .map(|vertex| weights[*vertex as usize]),
    )
}

/// Nine pack-bone influences per vertex, cut to the strongest four of the listed bones: an unlisted bone gives its weight to its nearest listed ancestor.
fn weights_from(
    rig: &Rig,
    bones: &[String],
    influences: impl Iterator<Item = [(u8, f32); 9]>,
) -> Result<(Vec<[u16; 4]>, Vec<[f32; 4]>), Fail> {
    let cleaned: Vec<String> = rig.bone_names.iter().map(|n| clean(n)).collect();
    // The listed slot each pack bone hands its weight to: itself, else its nearest listed ancestor, else the first listed bone.
    let mut slot_of: Vec<Option<usize>> = vec![None; cleaned.len()];
    for (slot, name) in bones.iter().enumerate() {
        let index = cleaned
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| unknown(rig, name))?;
        slot_of[index] = Some(slot);
    }
    let resolved: Vec<usize> = (0..cleaned.len())
        .map(|bone| {
            let mut at = Some(bone);
            while let Some(current) = at {
                if let Some(slot) = slot_of[current] {
                    return slot;
                }
                at = usize::try_from(rig.bone_parents[current]).ok();
            }
            0
        })
        .collect();
    let (mut joints, mut blend) = (Vec::new(), Vec::new());
    for influences in influences {
        let mut share = vec![0.0_f32; bones.len()];
        for (bone, weight) in influences {
            if weight > 0.0 {
                share[resolved.get(usize::from(bone)).copied().unwrap_or(0)] += weight;
            }
        }
        let mut order: Vec<usize> = (0..share.len()).collect();
        order.sort_by(|a, b| share[*b].total_cmp(&share[*a]).then(a.cmp(b)));
        let total: f32 = order.iter().take(4).map(|k| share[*k]).sum();
        let (mut j, mut w) = ([0_u16; 4], [0.0_f32; 4]);
        for (place, k) in order.iter().take(4).enumerate() {
            j[place] = u16::try_from(*k).unwrap_or(0);
            w[place] = if total > 0.0 { share[*k] / total } else { 0.0 };
        }
        if total <= 0.0 {
            w[0] = 1.0;
        }
        joints.push(j);
        blend.push(w);
    }
    Ok((joints, blend))
}

/// The bone names in `bones`, validated: 1 to `MAX_BONES`, no repeats.
fn bone_list(node: &Node, family: &str) -> Result<Vec<String>, Fail> {
    let mut bones: Vec<String> = Vec::new();
    for name in text(node, "bones")
        .split([';', ','])
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        if bones.iter().any(|known| known == name) {
            return Err(fail(
                "param.range",
                format!("{family} names bone {name:?} twice"),
            ));
        }
        bones.push(name.to_owned());
    }
    if bones.len() > MAX_BONES {
        return Err(fail_suggest(
            "param.range",
            format!(
                "{family} takes at most {MAX_BONES} bones, got {}",
                bones.len()
            ),
            format!("list no more than {MAX_BONES} bones in `bones`"),
        ));
    }
    Ok(bones)
}

/// The skin of a body or of what it wears: the bone parts it follows (`prefix` goes in front of each name) and how their motions combine.
fn skinning(
    node: &Node,
    bones: Vec<String>,
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
) -> Skinning {
    let prefix = text(node, "prefix");
    Skinning {
        bones: bones
            .into_iter()
            .map(|name| format!("{prefix}{name}"))
            .collect(),
        falloff: 3,
        influences: 4,
        // Dual quaternions keep a shoulder or an elbow from collapsing the way linear blending does (the candy-wrapper pinch of a raised arm);
        // linear blending is what the renderer skins on the GPU.
        blend: if text(node, "blend") == "linear" {
            crate::rig::Blend::Linear
        } else {
            crate::rig::Blend::DualQuaternion
        },
        paint: Vec::new(),
        joints,
        weights,
    }
}

fn pack_or_fail() -> Result<&'static ve_human::Human, Fail> {
    crate::human::pack().ok_or_else(|| {
        fail_suggest(
            "human.pack_missing",
            "the human body pack is not loaded",
            "load assets/human/human.pack into the evaluator (set_human_pack) and evaluate again",
        )
    })
}

fn skin(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (body, rig) = rig_of(node, inputs)?;
    let human = pack_or_fail()?;
    if body.skin.is_some() {
        return Err(fail("rig.duplicate", "a part can carry only one skin"));
    }
    let bones = bone_list(node, "body.skin")?;
    if bones.is_empty() {
        return Err(fail_suggest(
            "param.range",
            format!("body.skin needs 1 to {MAX_BONES} bones, got 0"),
            format!("list between 1 and {MAX_BONES} bones in `bones`"),
        ));
    }
    let (joints, weights) = weights_for(&rig, human, &bones, &body.human_source_indices)?;
    let mut value = (*body).clone();
    value.skin = Some(skinning(node, bones, joints, weights));
    Ok(value)
}

/// Whether the pack bone `name` (cleaned) belongs to the garment `cover`.
fn covers(cover: &str, name: &str) -> bool {
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| name.starts_with(p));
    match cover {
        // The lowest spine bone (spine05) sits at the hips: trousers take it (a waistband), a shirt ends above it.
        "shirt" => name != "spine05" && starts(&["spine", "clavicle", "shoulder", "upperarm"]),
        "long_sleeves" => {
            name != "spine05" && starts(&["spine", "clavicle", "shoulder", "upperarm", "lowerarm"])
        }
        "pants" => name == "spine05" || starts(&["pelvis", "upperleg", "lowerleg"]),
        "shorts" => name == "spine05" || starts(&["pelvis", "upperleg"]),
        "shoes" => starts(&["foot", "toe"]),
        "gloves" => starts(&["wrist", "finger", "metacarpal"]),
        _ => false,
    }
}

const COVERS: [&str; 6] = [
    "shirt",
    "long_sleeves",
    "pants",
    "shorts",
    "shoes",
    "gloves",
];
