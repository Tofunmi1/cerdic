//! Numbers as values: a constant, arithmetic, and a measurement of a solid. A number output can be linked into
//! any number parameter of another node (a port named like the parameter, or `size.y` for one component of a
//! vector), which is how one part's size drives another's.

use super::{single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

/// Largest magnitude a value may have: well past any size in metres, small enough to stay exact.
const LIMIT: f64 = 1.0e9;

/// What `value.size` can measure.
const MEASURES: &[&str] = &[
    "width", "height", "depth", "longest", "shortest", "volume", "area",
];

/// The places `value.point` can read.
const POINTS: &[&str] = &["head", "tail", "min", "center", "max"];

/// What `value.motion` can measure.
const MOTIONS: &[&str] = &["travel", "path", "duration", "speed", "lift"];

/// The coordinates `value.point` can read.
const AXES: &[&str] = &["x", "y", "z"];

/// What `value.math` can do.
const MATH_OPS: &[&str] = &["add", "sub", "mul", "div", "min", "max", "pow", "mod"];

fn number(name: &'static str, default: Option<f64>, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::None,
            min: -LIMIT,
            max: LIMIT,
        },
        default: default.map(Value::Num),
        doc,
    }
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let out = OutputSpec::Fixed(PortType::Number);
    vec![
        spec(
            "value.number@1",
            "A number, to link into any number parameter of another node (a port named like the parameter, or like size.y for one component of a vector). Use it to name a dimension once and reuse it.",
            vec![],
            out,
            vec![number("value", None, "The number.")],
        ),
        spec(
            "value.math@1",
            "Arithmetic on two numbers. a and b are number parameters, so either can be a literal or the output of another value node (a=width, b=2). op: add, sub, mul, div, min, max, pow (a to the power b) or mod (remainder). Division by zero and a non-finite result are refused.",
            vec![],
            out,
            vec![
                ParamSpec {
                    name: "op",
                    ty: ParamType::Choice { options: MATH_OPS },
                    default: Some(Value::Str("add".to_owned())),
                    doc: "The operation.",
                },
                number(
                    "a",
                    Some(0.0),
                    "The first number; link a value node here to compute from it.",
                ),
                number(
                    "b",
                    Some(0.0),
                    "The second number; link a value node here to compute from it.",
                ),
            ],
        ),
        spec(
            "value.point@1",
            "Reads a place on a part as a number: the head (the joint it turns about) or the tail of a bone (body.bone, rig.bone), or the min, center or max of any solid or polygon mesh's bounding box, along x, y or z. Link it into a number parameter, one component at a time (for example `by.y=hy` or `position.x=hx`), so one part is placed from where another is. A part that is not a bone has no head or tail.",
            vec![single("in", &[PortType::Solid, PortType::Poly])],
            out,
            vec![
                ParamSpec {
                    name: "at",
                    ty: ParamType::Choice { options: POINTS },
                    default: Some(Value::Str("center".to_owned())),
                    doc: "Which place: head or tail (a bone's joint and far end), or min, center, max (the corners and middle of the bounding box).",
                },
                ParamSpec {
                    name: "axis",
                    ty: ParamType::Choice { options: AXES },
                    default: Some(Value::Str("y".to_owned())),
                    doc: "Which coordinate to read: x, y or z (y is up).",
                },
            ],
        ),
        spec(
            "value.motion@1",
            "Measures how far a clip moves a part, from its position tracks (anim.track property=position): the straight horizontal `travel` from the first key to the last, the horizontal `path` length, the `duration` in seconds, the `speed` (travel over duration) and the vertical `lift` (highest minus lowest). Use it to read the real speed of a walk or a dash instead of guessing: link `speed` into a state machine's walk threshold or a game unit's speed, or `travel` into how far a part should be placed. `clip` names the clip (empty is the part's main clip). The part must be linked after the track.",
            vec![single(
                "in",
                &[PortType::Solid, PortType::Poly, PortType::Inst],
            )],
            out,
            vec![
                ParamSpec {
                    name: "measure",
                    ty: ParamType::Choice { options: MOTIONS },
                    default: Some(Value::Str("travel".to_owned())),
                    doc: "What to read: travel (straight horizontal distance, metres), path (horizontal path length), duration (seconds), speed (travel over duration, metres per second) or lift (vertical range, metres).",
                },
                ParamSpec {
                    name: "clip",
                    ty: ParamType::Str { max_len: 24 },
                    default: Some(Value::Str(String::new())),
                    doc: "The clip to read; empty reads the part's main clip.",
                },
            ],
        ),
        spec(
            "value.size@1",
            "Measures a solid or polygon mesh: the size of its bounding box along x (width), y (height) or z (depth), the longest or shortest of those, its enclosed volume (solids) or its surface area. Link the result into another node's parameter to size one part from another.",
            vec![single("in", &[PortType::Solid, PortType::Poly])],
            out,
            vec![ParamSpec {
                name: "measure",
                ty: ParamType::Choice { options: MEASURES },
                default: Some(Value::Str("height".to_owned())),
                doc: "What to measure: width (x extent), height (y), depth (z), longest, shortest, volume or area.",
            }],
        ),
    ]
}
