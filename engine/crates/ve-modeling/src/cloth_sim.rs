//! `cloth.sim`: cloth that runs live. A garment (any polygon mesh) is marked as cloth with its pins and the bones it collides with;
//! the runtime then steps the solver every frame, holds the pinned points to a moving bone, pushes the cloth out of capsules along the
//! other bones, and blows wind through it. Nothing is simulated at evaluation time: the node only describes the cloth, and the part keeps
//! its rest shape (so it can also be exported and measured).
//!
//! The garment part must not carry animation tracks of its own: its points are moved in world space by the simulation.

use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use crate::cache::Value;
use crate::poly_ops::{Elements, fingerprint};
use crate::rig::Segment;
use crate::{EvaluationWarning, Fail, Part, fail, fail_suggest};

/// The most points a live cloth may have (it is stepped every frame).
pub const MAX_LIVE_POINTS: usize = 6_000;
/// The most bones one cloth collides with.
pub const MAX_COLLIDER_BONES: usize = 16;

/// What the runtime needs to simulate a part as cloth; the part's own points are the rest shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClothSpec {
    /// Numbers of the points held to `pin_bone` (or to where they are, when `pin_bone` is empty).
    pub pins: Vec<u32>,
    /// Output name of the bone part the pins follow; empty pins them in the world.
    pub pin_bone: String,
    /// Output names of the bone parts that push the cloth away, as capsules.
    pub collide_bones: Vec<String>,
    /// The rest segment of each collider bone, in the same order (filled in after evaluation; a bone that is not a bone is dropped).
    pub segments: Vec<Segment>,
    /// Radius of every collider capsule, in metres.
    pub radius: f32,
    /// Mass per square metre in kilograms.
    pub density: f32,
    /// Stretch compliance (0 does not stretch).
    pub stretch: f32,
    /// Bend compliance (0 is stiff).
    pub bend: f32,
    /// Distance kept from colliders, in metres.
    pub thickness: f32,
    /// Share of sliding speed lost on contact.
    pub friction: f32,
    /// Speed lost per second.
    pub damping: f32,
    /// Acceleration in metres per second squared.
    pub gravity: [f32; 3],
    /// Wind velocity in metres per second.
    pub wind: [f32; 3],
    /// How hard air pushes on the cloth.
    pub drag: f32,
    /// Solver steps per frame.
    pub substeps: u32,
    /// Height of a floor the cloth cannot sink through, when there is one.
    pub floor: Option<f32>,
}

fn number(node: &Node, name: &str) -> Result<f32, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .map(|v| v as f32)
        .ok_or_else(|| fail("param.type", format!("{name} is missing or not a number")))
}

fn vector(node: &Node, name: &str) -> Result<[f32; 3], Fail> {
    match node.params.get(name) {
        Some(Param::List(items)) if items.len() == 3 => {
            let mut out = [0.0_f32; 3];
            for (slot, item) in out.iter_mut().zip(items) {
                *slot = item
                    .as_f64()
                    .ok_or_else(|| fail("param.type", format!("{name} must be three numbers")))?
                    as f32;
            }
            Ok(out)
        }
        _ => Err(fail("param.type", format!("{name} must be three numbers"))),
    }
}

fn names(node: &Node, name: &str) -> Vec<String> {
    match node.params.get(name) {
        Some(Param::Str(list)) => list
            .split([';', ','])
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "cloth.sim").then(|| sim(node, inputs))
}

fn sim(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let by_name = |name: &str| {
        node.inputs
            .keys()
            .position(|k| k == name)
            .and_then(|at| inputs.get(at))
    };
    let garment = by_name("in")
        .ok_or_else(|| fail("graph.missing_input", "cloth.sim needs a garment on `in`"))?;
    let Some(poly) = garment.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    if garment.mesh.positions.len() != poly.positions().len() {
        return Err(fail(
            "cloth.panel",
            "the garment's drawn points do not match its polygon points; pass it through a plain polygon node first",
        ));
    }
    if poly.positions().len() > MAX_LIVE_POINTS {
        return Err(fail_suggest(
            "budget.cloth",
            format!(
                "the garment has {} points; live cloth takes at most {MAX_LIVE_POINTS}",
                poly.positions().len()
            ),
            "use a coarser garment (fewer columns and rows)",
        ));
    }
    if !garment.animation.is_empty() {
        return Err(fail_suggest(
            "cloth.animated",
            "the garment has animation tracks; live cloth moves its own points".to_owned(),
            "remove the tracks from the garment and animate the bone it is pinned to instead",
        ));
    }
    let mut pins = Vec::new();
    if let Some(selection) = by_name("pins") {
        let selection = selection.selection.as_deref().ok_or_else(|| {
            fail(
                "poly.selection_kind",
                "pins needs a selection of vertices from poly.select",
            )
        })?;
        if selection.fingerprint != fingerprint(poly) {
            return Err(fail(
                "selection.stale",
                "the pin selection was made on a different mesh than the garment",
            ));
        }
        let Elements::Vertices(items) = &selection.elements else {
            return Err(fail_suggest(
                "poly.selection_kind",
                format!(
                    "pins needs a selection of vertices but got {}s",
                    selection.elements.kind()
                ),
                "set element=vertex on poly.select",
            ));
        };
        pins.clone_from(items);
    }
    let collide_bones = names(node, "collide_bones");
    if collide_bones.len() > MAX_COLLIDER_BONES {
        return Err(fail(
            "param.range",
            format!(
                "cloth.sim collides with at most {MAX_COLLIDER_BONES} bones, got {}",
                collide_bones.len()
            ),
        ));
    }
    let floor = number(node, "floor")?;
    let spec = ClothSpec {
        pins,
        pin_bone: names(node, "pin_bone")
            .into_iter()
            .next()
            .unwrap_or_default(),
        collide_bones,
        segments: Vec::new(),
        radius: number(node, "collider_radius")?,
        density: number(node, "density")?,
        stretch: number(node, "stretch")?,
        bend: number(node, "bend")?,
        thickness: number(node, "thickness")?,
        friction: number(node, "friction")?,
        damping: number(node, "damping")?,
        gravity: [0.0, -number(node, "gravity")?, 0.0],
        wind: vector(node, "wind")?,
        drag: number(node, "drag")?,
        substeps: number(node, "substeps")? as u32,
        floor: (floor > -1.0e5).then_some(floor),
    };
    let mut value = (**garment).clone();
    value.cloth = Some(Rc::new(spec));
    Ok(value)
}

/// Fills each cloth's collider segments from the bone parts it names, and drops what cannot work with a warning, so the viewer only ever
/// sees cloth it can simulate.
pub(crate) fn resolve(parts: &mut [Part]) -> Vec<EvaluationWarning> {
    let bones: std::collections::HashMap<String, Option<Segment>> =
        parts.iter().map(|p| (p.name.clone(), p.bone)).collect();
    let all: std::collections::HashSet<String> = parts.iter().map(|p| p.name.clone()).collect();
    let mut warnings = Vec::new();
    for part in parts.iter_mut() {
        let Some(mut spec) = part.cloth.take() else {
            continue;
        };
        let warn = |code: &str, message: String| EvaluationWarning {
            code: code.to_owned(),
            node: part.node.to_string(),
            related: Vec::new(),
            message,
        };
        if !spec.pin_bone.is_empty() && !all.contains(&spec.pin_bone) {
            warnings.push(warn(
                "cloth.pin_bone",
                format!("{}: pin_bone \"{}\" is not a part of the scene (name an output); the cloth is not simulated", part.name, spec.pin_bone),
            ));
            continue;
        }
        let mut kept = Vec::new();
        let mut segments = Vec::new();
        for name in std::mem::take(&mut spec.collide_bones) {
            match bones.get(&name) {
                Some(Some(segment)) => {
                    kept.push(name);
                    segments.push(*segment);
                }
                _ => warnings.push(warn(
                    "cloth.collide_bone",
                    format!("{}: collide bone \"{name}\" is not a bone part (it needs rig.bone); it is ignored", part.name),
                )),
            }
        }
        spec.collide_bones = kept;
        spec.segments = segments;
        part.cloth = Some(spec);
    }
    warnings
}
