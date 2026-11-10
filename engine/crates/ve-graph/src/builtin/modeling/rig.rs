use super::{COLOURABLE, number, single, spec, triple};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec};
use crate::value::{ParamType, Unit, Value};

pub(super) fn specs() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "rig.bone@1",
            "Marks a part as a bone of a skeleton and records its rest segment, from `head` (the joint it turns about) to `tail`, in world \
                     coordinates. Bones are ordinary parts: move them with anim.track, anim.wave and behaviours, and join them with anim.parent \
                     (a forearm bone follows an upper-arm bone). A skinned mesh (rig.skin) bends with them. The bone part itself is usually small \
                     or hidden.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "head",
                    ty: ParamType::Vec3 {
                        unit: Unit::Length,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: None,
                    doc: "Where the bone starts at rest: its joint, in world coordinates.",
                },
                ParamSpec {
                    name: "tail",
                    ty: ParamType::Vec3 {
                        unit: Unit::Length,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: None,
                    doc: "Where the bone ends at rest, in world coordinates. Must differ from head.",
                },
            ],
        ),
        spec(
            "rig.ik@1",
            "Makes this shape a TARGET that a chain of bones reaches for (inverse kinematics): in play and in the viewer, an arm, leg or tentacle \
                     built of bone parts bends so its tip follows the target as the target moves (animate it, or make it grabbable and carry it). \
                     `bones` lists the OUTPUT names of the bone parts from root to tip; each must carry rig.bone, be anim.parent-ed to the one before, and \
                     start where it ends. The chain's own anim.track motion is replaced by the solver while a target exists. A target that is out of reach \
                     leaves the chain straight and pointing at it.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "bones",
                    ty: ParamType::Str { max_len: 256 },
                    default: None,
                    doc: "Output names of the bone parts, root to tip, separated by semicolons (1 to 8), for example \"upper_arm; forearm; hand\".",
                },
                ParamSpec {
                    name: "iterations",
                    ty: ParamType::Int { min: 1, max: 64 },
                    default: Some(Value::Int(32)),
                    doc: "Solver passes per frame. More passes reach a target near full stretch more exactly.",
                },
                ParamSpec {
                    name: "limit",
                    ty: ParamType::Num {
                        unit: Unit::Angle,
                        min: 1.0,
                        max: 180.0,
                    },
                    default: Some(Value::Num(180.0)),
                    doc: "ccd only: the most any one bone may turn from its rest pose, in degrees (180 is no limit). A tentacle that must not fold double uses 60; a tail 40. A target the limits make unreachable is approached as far as they allow.",
                },
                ParamSpec {
                    name: "solver",
                    ty: ParamType::Choice {
                        options: &["ccd", "two_bone"],
                    },
                    default: Some(Value::Str("ccd".to_owned())),
                    doc: "ccd: any chain of 1 to 8 bones (tentacles, tails). two_bone: exactly two bones, an arm or a leg, solved exactly with a pole so the elbow or knee bends the way you say (no flipping), with soften and twist.",
                },
                ParamSpec {
                    name: "pole",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: Some(triple(0.0, 0.0, 0.0)),
                    doc: "two_bone only: the world direction the elbow or knee bends towards (a knee points forward [0, 0, 1], an elbow back [0, 0, -1] or out). [0, 0, 0] keeps the bend it has at rest.",
                },
                number(
                    "soften",
                    Unit::Ratio,
                    (0.0, 1.0),
                    0.0,
                    "two_bone only: 0 snaps the limb straight at full stretch; 0.2 to 0.5 eases it in before it locks, which avoids the popping knee. 1 eases over the whole reach.",
                ),
                number(
                    "twist",
                    Unit::Angle,
                    (-6.3, 6.3),
                    0.0,
                    "two_bone only: extra swing of the elbow or knee around the line from shoulder to target, in degrees.",
                ),
            ],
        ),
        spec(
            "rig.aim@1",
            "Makes this shape a TARGET that ONE bone part turns to look at (a head, an eye, a turret, a searchlight), in play and in the viewer. \
                     `bone` is the OUTPUT name of a part that carries rig.bone: the bone's own direction, from its head to its tail at rest, is what \
                     points at the target, and `up` (a direction at rest, default [0, 1, 0]) is the side that stays towards the `pole` so a head does not \
                     roll. Chain it from a body part with anim.parent so the head turns with the body. Use rig.ik for arms and legs.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "bone",
                    ty: ParamType::Str { max_len: 64 },
                    default: None,
                    doc: "Output name of the bone part that looks at this target, for example \"head\".",
                },
                ParamSpec {
                    name: "up",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: Some(triple(0.0, 1.0, 0.0)),
                    doc: "The bone's own up direction at rest (the top of a head is [0, 1, 0]).",
                },
                ParamSpec {
                    name: "pole",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: Some(triple(0.0, 1.0, 0.0)),
                    doc: "The world direction the bone's up leans towards while looking: [0, 1, 0] keeps a head upright. [0, 0, 0] keeps the current up.",
                },
                number(
                    "twist",
                    Unit::Angle,
                    (-6.3, 6.3),
                    0.0,
                    "Extra roll around the line of sight, in degrees (a head tilted while looking).",
                ),
                number(
                    "weight",
                    Unit::Ratio,
                    (0.0, 1.0),
                    1.0,
                    "How much of the turn to apply: 0.5 looks half-way towards the target (a lazy glance).",
                ),
            ],
        ),
        spec(
            "rig.gait@1",
            "Makes a foot target WALK. Put it after rig.ik on the shape that the leg's bone chain reaches for: instead of you animating that target, it plants on \
                     the ground while its body passes over it and swings forward in an arc to the next step (a procedural gait from the body's own motion; Spore did \
                     the same for any creature). `body` is the OUTPUT name of the part the leg is attached to; move that part (an anim.track, a behaviour or physics) \
                     and the feet follow it. Give each leg a phase: alternate legs 0 and 0.5 for a spider's stagger, 0, 0.25, 0.5, 0.75 for a four-legged walk. \
                     The foot never stretches farther than its reach from the hip.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "body",
                    ty: ParamType::Str { max_len: 64 },
                    default: None,
                    doc: "Output name of the body part this leg is attached to; its motion drives the walk.",
                },
                number(
                    "phase",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Where in the cycle this foot starts: alternate legs 0 and 0.5.",
                ),
                number(
                    "stride_length",
                    Unit::Length,
                    (0.05, 20.0),
                    0.6,
                    "Metres the body travels per full step cycle of this foot: longer is a lazier stride.",
                ),
                number(
                    "step_height",
                    Unit::Length,
                    (0.0, 5.0),
                    0.15,
                    "How high the foot lifts while swinging.",
                ),
                number(
                    "duty",
                    Unit::None,
                    (0.1, 1.0),
                    0.6,
                    "The share of the cycle the foot is planted: 0.6 walks, 0.4 runs, 0.8 creeps.",
                ),
                number(
                    "reach",
                    Unit::Length,
                    (0.0, 50.0),
                    0.0,
                    "The farthest the foot may be from the hip; 0 uses the length of the leg.",
                ),
                number(
                    "ground",
                    Unit::Length,
                    (-1000.0, 1000.0),
                    0.0,
                    "Height of the ground the feet plant on.",
                ),
            ],
        ),
        spec(
            "rig.secondary@1",
            "Gives a chain of bones secondary motion: a tail, ear, antenna, hair or cloth edge that lags behind the body and springs back, from a \
                     spring simulation run every frame in the viewer (inertia, damping, gravity and a pull back to the rest pose). The shape you attach it to \
                     only carries it (use the tip bone). `bones` lists the OUTPUT names of the bone parts root to tip; each must carry rig.bone, be \
                     anim.parent-ed to the one before, and start where it ends, as for rig.ik. The root bone's parent (the body) moves the chain; the chain's own \
                     anim.track motion is replaced. It needs a body that moves to show anything.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "bones",
                    ty: ParamType::Str { max_len: 256 },
                    default: None,
                    doc: "Output names of the bone parts, root to tip, separated by semicolons (1 to 8), for example \"tail1; tail2; tail3\".",
                },
                number(
                    "stiffness",
                    Unit::None,
                    (0.0, 1.0),
                    0.3,
                    "How hard the chain springs back to its rest pose: 0 hangs free (hair, a loose rope), 0.3 a tail, 1 stiff.",
                ),
                number(
                    "damping",
                    Unit::None,
                    (0.0, 50.0),
                    4.0,
                    "Drag per second: low swings on for a long time, high stops quickly.",
                ),
                number(
                    "gravity",
                    Unit::None,
                    (0.0, 50.0),
                    9.8,
                    "How hard the chain is pulled down, in m/s^2: 9.8 is normal, 0 floats (tentacles under water).",
                ),
            ],
        ),
        spec(
            "rig.skin@1",
            "Makes a shape bend with moving bones: each vertex follows the bones nearest to it (a blend, so a tube bends smoothly at a joint). \
                     `bones` lists the OUTPUT names of parts that carry rig.bone. A skinned shape cannot also be moved by its own anim.track, \
                     parent or behaviour: move its bones instead. Bones that do not exist are ignored with a warning.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "bones",
                    ty: ParamType::Str { max_len: 1024 },
                    default: None,
                    doc: "Output names of the bone parts, separated by semicolons (at most 64), for example \"upper_arm; forearm\".",
                },
                ParamSpec {
                    name: "falloff",
                    ty: ParamType::Int { min: 1, max: 8 },
                    default: Some(Value::Int(3)),
                    doc: "How sharply a bone's influence drops with distance. Higher gives tighter, more local bending; 1 blends broadly.",
                },
                ParamSpec {
                    name: "influences",
                    ty: ParamType::Int { min: 1, max: 4 },
                    default: Some(Value::Int(4)),
                    doc: "Most bones allowed to move one vertex (1 makes every vertex follow a single bone, which gives rigid pieces).",
                },
                ParamSpec {
                    name: "blend",
                    ty: ParamType::Choice {
                        options: &["linear", "dual_quaternion"],
                    },
                    default: Some(Value::Str("linear".to_owned())),
                    doc: "How bones combine on a vertex. linear is fast but a strong twist pinches a limb thin; dual_quaternion keeps its volume through twists. The glTF export is always linear.",
                },
                ParamSpec {
                    name: "paint",
                    ty: ParamType::Str { max_len: 2048 },
                    default: Some(Value::Str(String::new())),
                    doc: "Brush stamps that correct the automatic weights, separated by semicolons (at most 64): \"bone x y z radius strength\". strength 1 hands vertices under the brush centre to that bone, -1 takes the bone away from them, fading smoothly to nothing at the radius. Example: \"forearm 0 0.5 0 0.2 1; upper_arm 0 0.5 0 0.2 -1\".",
                },
            ],
        ),
        spec(
            "rig.morph@1",
            "Adds one Blender-style shape key to a character mesh. Make the target by duplicating the base and sculpting or moving its points; point count and face topology must stay identical. The output previews the chosen weight and keeps the named delta for glTF morph-target export. Chain nodes to add up to 16 keys.",
            vec![single("base", COLOURABLE), single("target", COLOURABLE)],
            OutputSpec::SameAsInput("base"),
            vec![
                ParamSpec {
                    name: "name",
                    ty: ParamType::Str { max_len: 32 },
                    default: None,
                    doc: "Unique name for this shape key, such as smile, blink_left or brow_raise.",
                },
                number(
                    "weight",
                    Unit::Ratio,
                    (-1.0, 1.0),
                    1.0,
                    "Current blend from neutral (0) to target (1); negative values invert the sculpted delta.",
                ),
            ],
        ),
    ]
}
