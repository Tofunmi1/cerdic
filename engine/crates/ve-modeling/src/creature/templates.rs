//! Body plans: a few sliders fill the skeleton tree. They are starting points; the tree they return is an ordinary
//! [`Creature`] that can be edited bone by bone, or written from scratch for a body no template covers.
use super::{Blob, Bone, Creature, CreatureError, Role};
use serde::{Deserialize, Serialize};

mod biped;
mod quadruped;
mod serpent;

/// Which skeleton to start from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    /// Two legs, two arms, a head on a neck.
    Biped,
    /// Four legs, a neck and head, a tail; optional horns and wings.
    Quadruped,
    /// A long undulating body with no limbs.
    Serpent,
}

/// What a person or an LLM may turn. Every slider is a multiplier around 1 (0.5 to 2), except `size`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Sliders {
    /// Overall height in metres, from the ground to the top of the head or back (0.2 to 20).
    pub size: f64,
    /// Head size.
    pub head: f64,
    /// Limb thickness.
    pub limbs: f64,
    /// Torso width and bulk.
    pub torso: f64,
    /// Leg length.
    pub legs: f64,
    /// Neck length (quadruped).
    pub neck: f64,
    /// Tail length (quadruped, serpent).
    pub tail: f64,
    /// Body colour; each 0 to 1.
    pub color: Option<[f64; 3]>,
    /// Two horns (quadruped).
    pub horns: bool,
    /// Two wings, each an arm bone and two fingers (quadruped).
    pub wings: bool,
}

impl Default for Sliders {
    fn default() -> Self {
        Self {
            size: 1.7,
            head: 1.0,
            limbs: 1.0,
            torso: 1.0,
            legs: 1.0,
            neck: 1.0,
            tail: 1.0,
            color: None,
            horns: false,
            wings: false,
        }
    }
}

fn bone(
    name: &str,
    parent: Option<&str>,
    head: [f64; 3],
    tail: [f64; 3],
    radius: [f64; 2],
    role: Role,
) -> Bone {
    Bone {
        name: name.to_owned(),
        parent: parent.map(str::to_owned),
        head,
        tail,
        via: Vec::new(),
        radius,
        role,
        pair: false,
        phase: 0.0,
        blend: 0.0,
    }
}

fn paired(mut bone: Bone, phase: f64) -> Bone {
    bone.pair = true;
    bone.phase = phase;
    bone
}

fn blob(name: &str, center: [f64; 3], radii: [f64; 3], pair: bool) -> Blob {
    Blob {
        name: name.to_owned(),
        center,
        radii,
        pair,
        blend: 0.0,
    }
}

fn scaled(value: [f64; 3], by: f64) -> [f64; 3] {
    value.map(|v| v * by)
}

/// Builds the skeleton tree for `plan` from `sliders`.
///
/// # Errors
/// A slider is out of range, or the result is not a valid creature.
pub fn template(plan: Plan, sliders: &Sliders) -> Result<Creature, CreatureError> {
    let multipliers = [
        sliders.head,
        sliders.limbs,
        sliders.torso,
        sliders.legs,
        sliders.neck,
        sliders.tail,
    ];
    if multipliers.iter().any(|m| !(0.5..=2.0).contains(m)) {
        return Err(CreatureError(
            "every slider except size is between 0.5 and 2".to_owned(),
        ));
    }
    if !(0.2..=20.0).contains(&sliders.size) {
        return Err(CreatureError(
            "size is between 0.2 and 20 metres".to_owned(),
        ));
    }
    let mut creature = match plan {
        Plan::Biped => biped::biped(sliders),
        Plan::Quadruped => quadruped::quadruped(sliders),
        Plan::Serpent => serpent::serpent(sliders),
    };
    stand_on_ground(&mut creature, sliders.size, plan != Plan::Serpent);
    creature.validate()?;
    Ok(creature)
}

/// Scales the whole creature and sets its lowest point on the ground. For an upright body `size` is its height; for one
/// that lies along the ground (a serpent) it is the longest side.
fn stand_on_ground(creature: &mut Creature, size: f64, upright: bool) {
    let mut high = [f64::NEG_INFINITY; 3];
    let mut low = [f64::INFINITY; 3];
    let mut reach = |p: [f64; 3], r: [f64; 3]| {
        for axis in 0..3 {
            high[axis] = high[axis].max(p[axis] + r[axis]);
            low[axis] = low[axis].min(p[axis] - r[axis]);
        }
    };
    for b in &creature.bones {
        let r = b.radius[0].max(b.radius[1]);
        for p in std::iter::once(b.head)
            .chain(b.via.iter().copied())
            .chain(std::iter::once(b.tail))
        {
            reach(p, [r; 3]);
        }
    }
    for b in &creature.blobs {
        reach(b.center, b.radii);
    }
    let bottom = low[1];
    let measured = if upright {
        high[1] - low[1]
    } else {
        (0..3).map(|a| high[a] - low[a]).fold(0.0_f64, f64::max)
    };
    let factor = size / measured;
    let lift = |p: [f64; 3]| [p[0] * factor, (p[1] - bottom) * factor, p[2] * factor];
    for b in &mut creature.bones {
        b.head = lift(b.head);
        b.tail = lift(b.tail);
        b.via = b.via.iter().map(|p| lift(*p)).collect();
        b.radius = b.radius.map(|r| r * factor);
        b.blend *= factor;
    }
    for b in &mut creature.blobs {
        b.center = lift(b.center);
        b.radii = scaled(b.radii, factor);
        b.blend *= factor;
    }
}
