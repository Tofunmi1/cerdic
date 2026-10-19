use super::{COLOURABLE, single, spec};
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
                    doc: "Output names of the bone parts, separated by semicolons (at most 32), for example \"upper_arm; forearm\".",
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
    ]
}
