use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use crate::{EvaluationWarning, Fail, Part, cache::Value, fail};

/// Most physics bodies in a scene (the world's own cap).
pub const MAX_BODIES: usize = 256;

/// How a body moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    /// Never moves; things collide with it (floors, walls, terrain).
    Static,
    /// Falls and collides.
    Dynamic,
    /// Follows its own animation or the hand that holds it, and pushes dynamic bodies.
    Kinematic,
}

/// What a body collides as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyShape {
    /// The convex hull of the part's vertices.
    Auto,
    /// The part's bounding box.
    Box,
    /// A ball that fits the bounding box.
    Sphere,
    /// A capsule along Y that fits the bounding box.
    Capsule,
    /// A cylinder along Y that fits the bounding box.
    Cylinder,
    /// The convex hull of the part's vertices (at most 256 are used).
    Convex,
    /// The exact triangle surface (static bodies only).
    Mesh,
}

/// A part's physical properties.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysBody {
    /// How it moves.
    pub kind: BodyKind,
    /// What it collides as.
    pub shape: BodyShape,
    /// Mass per cubic metre.
    pub density: f64,
    /// Sliding friction.
    pub friction: f64,
    /// Bounciness, 0 to 1.
    pub restitution: f64,
    /// Fraction of speed lost per second.
    pub linear_damping: f64,
    /// Fraction of spin lost per second.
    pub angular_damping: f64,
    /// Joints that connect this body to another part or to the world (from `phys.joint`).
    #[serde(default)]
    pub joints: Vec<PhysJoint>,
    /// Settings for the whole scene's physics, carried by one body (from `phys.world`): usually the floor's.
    #[serde(default)]
    pub world: Option<WorldSettings>,
}

/// Scene-wide physics settings: gravity and the walking character.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldSettings {
    /// Gravity in metres per second squared; Earth is `[0, -9.81, 0]`, the Moon `[0, -1.62, 0]`.
    pub gravity: [f64; 3],
    /// Whether the viewer walks in play mode (gravity, collisions, jumping) instead of flying.
    pub walker: bool,
    /// Walker height in metres (head to feet).
    pub height: f64,
    /// Walker radius in metres.
    pub radius: f64,
    /// Walking speed in metres per second.
    pub speed: f64,
    /// Upward speed of a jump in metres per second (4.5 is about one metre high).
    pub jump: f64,
}

/// Most joints one part may carry.
pub const MAX_JOINTS_PER_PART: usize = 4;

/// How a joint lets its two bodies move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JointKind {
    /// Rigidly together.
    Fixed,
    /// Turns about an axis (door, wheel, pendulum).
    Hinge,
    /// Slides along an axis (drawer, piston).
    Slider,
    /// Turns freely about a point (rope link, shoulder).
    Ball,
    /// Pulls two points toward a rest distance.
    Spring,
}

/// A joint from this part to another part (or the world), in world coordinates at the starting pose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysJoint {
    /// Output name of the other part; empty joins this part to the world.
    pub to: String,
    /// How it moves.
    pub kind: JointKind,
    /// The joint's point (on this part), metres.
    pub anchor: [f64; 3],
    /// The point on the other part for a spring (others use `anchor`).
    pub anchor_b: [f64; 3],
    /// Hinge axis or slider direction.
    pub axis: [f64; 3],
    /// Whether `low` and `high` apply.
    pub limited: bool,
    /// Lowest angle (degrees, hinge) or distance (metres, slider).
    pub low: f64,
    /// Highest angle (degrees, hinge) or distance (metres, slider).
    pub high: f64,
    /// Spring stiffness (acceleration per metre of stretch).
    pub stiffness: f64,
    /// Spring damping, or the friction of a hinge or slider.
    pub damping: f64,
    /// Motor aim: degrees per second (hinge) or metres per second (slider).
    pub motor_speed: f64,
    /// Strongest push or torque the motor may use; 0 means no motor.
    pub motor_force: f64,
    /// Distance a spring rests at; negative means the distance at the start.
    pub rest_length: f64,
}

fn choice<T: Copy>(node: &Node, name: &str, options: &[(&str, T)], default: T) -> T {
    match node.params.get(name) {
        Some(Param::Str(word)) => options
            .iter()
            .find(|(text, _)| text == word)
            .map_or(default, |(_, value)| *value),
        _ => default,
    }
}

fn number(node: &Node, name: &str, default: f64) -> f64 {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}

/// Records the body on the input.
pub(crate) fn body(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "phys.body needs a shape"))?;
    let kind = choice(
        node,
        "kind",
        &[
            ("static", BodyKind::Static),
            ("dynamic", BodyKind::Dynamic),
            ("kinematic", BodyKind::Kinematic),
        ],
        BodyKind::Dynamic,
    );
    let shape = choice(
        node,
        "shape",
        &[
            ("auto", BodyShape::Auto),
            ("box", BodyShape::Box),
            ("sphere", BodyShape::Sphere),
            ("capsule", BodyShape::Capsule),
            ("cylinder", BodyShape::Cylinder),
            ("convex", BodyShape::Convex),
            ("mesh", BodyShape::Mesh),
        ],
        BodyShape::Auto,
    );
    if shape == BodyShape::Mesh && kind != BodyKind::Static {
        return Err(fail(
            "param.range",
            "a mesh shape can only be static (floors, terrain, buildings): use shape=\"convex\" or \"box\" for a moving body",
        ));
    }
    if input.body.is_some() {
        return Err(fail(
            "phys.duplicate",
            "a part can carry only one phys.body",
        ));
    }
    let mut value = (**input).clone();
    value.body = Some(PhysBody {
        kind,
        shape,
        density: number(node, "density", 1.0),
        friction: number(node, "friction", 0.6),
        restitution: number(node, "restitution", 0.0),
        linear_damping: number(node, "linear_damping", 0.0),
        angular_damping: number(node, "angular_damping", 0.05),
        joints: Vec::new(),
        world: None,
    });
    Ok(value)
}

/// Adds scene-wide physics settings (gravity, a walking character) to the body on the input.
pub(crate) fn world(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "phys.world needs a shape"))?;
    if input.body.is_none() {
        return Err(fail(
            "phys.world_needs_body",
            "put phys.world after phys.body, on any one body (the floor is natural): it sets gravity and walking for the whole scene",
        ));
    }
    let gravity = triple(node, "gravity", [0.0, -9.81, 0.0]);
    if gravity.iter().any(|v| v.abs() > 1000.0) {
        return Err(fail(
            "param.range",
            "gravity must stay within 1000 m/s2 on each axis",
        ));
    }
    let mut value = (**input).clone();
    if let Some(body) = &mut value.body {
        body.world = Some(WorldSettings {
            gravity,
            walker: matches!(node.params.get("walker"), Some(Param::Bool(true))),
            height: number(node, "height", 1.7),
            radius: number(node, "radius", 0.3),
            speed: number(node, "speed", 3.0),
            jump: number(node, "jump", 4.5),
        });
    }
    Ok(value)
}

fn triple(node: &Node, name: &str, default: [f64; 3]) -> [f64; 3] {
    let Some(Param::List(items)) = node.params.get(name) else {
        return default;
    };
    let mut out = default;
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item.as_f64().unwrap_or(*slot);
    }
    out
}
