//! Operators that edit profiles, build solids from them, and constrain the result.

use super::{COLOURABLE, EXTENT, PROFILE, length, single, spec, variadic};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn number(
    name: &'static str,
    unit: Unit,
    bounds: (f64, f64),
    default: f64,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit,
            min: bounds.0,
            max: bounds.1,
        },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn triple(x: f64, y: f64, z: f64) -> Value {
    Value::List(vec![Value::Num(x), Value::Num(y), Value::Num(z)])
}

/// Profile edits, solids built from profiles, appearance, and constraints on them.
pub(super) fn from_profiles() -> Vec<Option<OperatorSpec>> {
    let mut all = profile_edits();
    all.extend(solids_from_profiles());
    all.extend(appearance());
    all.extend(constraints());
    all
}

fn profile_edits() -> Vec<Option<OperatorSpec>> {
    let profile = OutputSpec::Fixed(PortType::Profile);
    vec![
        spec(
            "profile.offset@1",
            "Grows (positive) or shrinks (negative) a profile, with mitred or round corners.",
            vec![single("in", PROFILE)],
            profile,
            vec![
                ParamSpec {
                    name: "delta",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: -EXTENT,
                        max: EXTENT,
                    },
                    default: None,
                    doc: "Distance in metres; an erosion that removes everything gives an empty profile.",
                },
                ParamSpec {
                    name: "join",
                    ty: ParamType::Choice {
                        options: &["miter", "round"],
                    },
                    default: Some(Value::Str("miter".to_owned())),
                    doc: "Corner treatment: miter keeps sharp corners (limit 2); round arcs them (64 segments per circle).",
                },
            ],
        ),
        spec(
            "profile.smooth@1",
            "Rounds every corner of a profile into a smooth curve by repeated corner cutting.",
            vec![single("in", PROFILE)],
            profile,
            vec![
                ParamSpec {
                    name: "iterations",
                    ty: ParamType::Int { min: 1, max: 5 },
                    default: Some(Value::Int(2)),
                    doc: "Each round doubles the points and halves the sharpness: 2 is gentle, 4 is very smooth. The shape shrinks slightly inward at the corners.",
                },
                ParamSpec {
                    name: "keep_axis",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(false)),
                    doc: "Set true for a profile you will revolve: points on the axis (x = 0) stay put, so the base stays flat and the axis edge is not trimmed.",
                },
            ],
        ),
        spec(
            "profile.fillet@1",
            "Rounds convex corners of a planar profile while preserving its original outer bounds.",
            vec![single("in", PROFILE)],
            profile,
            vec![length(
                "radius",
                "Corner radius in metres; the eroded core must remain nonempty.",
            )],
        ),
        spec(
            "profile.difference@1",
            "The first profile minus all the others.",
            vec![variadic("in", PROFILE)],
            profile,
            vec![],
        ),
    ]
}

fn solids_from_profiles() -> Vec<Option<OperatorSpec>> {
    let solid = OutputSpec::Fixed(PortType::Solid);
    vec![spec(
        "solid.extrude@1",
        "Stands a profile along Y from 0 to height (profile Y maps to -Z).",
        vec![single("in", PROFILE)],
        solid,
        vec![
            length("height", "Height in metres."),
            ParamSpec {
                name: "divisions",
                ty: ParamType::Int { min: 1, max: 256 },
                default: Some(Value::Int(1)),
                doc: "Vertical slabs; use 16 or more with a twist.",
            },
            number(
                "twist",
                Unit::Angle,
                (-10.0 * std::f64::consts::TAU, 10.0 * std::f64::consts::TAU),
                0.0,
                "Rotation of the top relative to the base, in radians.",
            ),
            ParamSpec {
                name: "top_scale",
                ty: ParamType::Vec2 {
                    unit: Unit::Ratio,
                    min: 1.0e-3,
                    max: 1.0e3,
                },
                default: Some(Value::List(vec![Value::Num(1.0), Value::Num(1.0)])),
                doc: "Size of the top relative to the base, per axis.",
            },
        ],
    )]
}

mod animation;
mod appearance;
mod behavior;
mod fx;
mod rig;

fn appearance() -> Vec<Option<OperatorSpec>> {
    [
        appearance::specs(),
        animation::specs(),
        rig::specs(),
        fx::specs(),
        behavior::specs(),
    ]
    .concat()
}

fn constraints() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "check.min_wall@1",
            "Fails when the wall between an outer and an inner profile is thinner than `min`.",
            vec![single("outer", PROFILE), single("inner", PROFILE)],
            OutputSpec::Fixed(PortType::Check),
            vec![length("min", "Smallest allowed wall thickness in metres.")],
        ),
        spec(
            "check.attached@1",
            "Finds solid parts separated from their nearest neighbour by a gap larger than the tolerance.",
            vec![super::variadic("parts", super::SOLID)],
            OutputSpec::Fixed(PortType::Check),
            vec![
                number(
                    "tolerance",
                    Unit::Length,
                    (0.0, 1.0),
                    0.02,
                    "Largest gap between a part and its nearest neighbour that still counts as attached",
                ),
                ParamSpec {
                    name: "severity",
                    ty: ParamType::Choice {
                        options: &["warning", "error"],
                    },
                    default: Some(Value::Str("warning".to_owned())),
                    doc: "warning lists the floating parts in the evaluation's warnings; error fails the evaluation",
                },
            ],
        ),
        spec(
            "check.motion@1",
            "Samples the named parts through a clip to find feet that slide while planted and loops that pop at the seam. `feet` is a comma-separated list of part output names; `speed` is the travel speed of an in-place clip along +x. Example: `check.motion@1(leg, feet=\"left_foot\", clip=\"walk\", speed=1.2)`. ",
            vec![super::variadic_one_or_more("parts", super::SOLID)],
            OutputSpec::Fixed(PortType::Check),
            vec![
                ParamSpec {
                    name: "feet",
                    ty: ParamType::Str { max_len: 512 },
                    default: Some(Value::Str(String::new())),
                    doc: "Comma-separated output names of parts whose lowest point should be checked for foot sliding.",
                },
                ParamSpec {
                    name: "clip",
                    ty: ParamType::Str { max_len: 24 },
                    default: Some(Value::Str(String::new())),
                    doc: "Clip to sample; empty selects the main clip.",
                },
                number(
                    "duration",
                    Unit::None,
                    (0.1, 30.0),
                    1.0,
                    "Sample duration in seconds.",
                ),
                ParamSpec {
                    name: "fps",
                    ty: ParamType::Int { min: 5, max: 120 },
                    default: Some(Value::Int(30)),
                    doc: "Samples per second; duration times fps is limited to 3,600 frames.",
                },
                number(
                    "speed",
                    Unit::None,
                    (-20.0, 20.0),
                    0.0,
                    "Travel speed in metres per second along +x for an in-place clip.",
                ),
                number(
                    "slide_max",
                    Unit::None,
                    (0.0, 5.0),
                    0.1,
                    "Largest average planted-foot speed allowed in metres per second.",
                ),
                ParamSpec {
                    name: "severity",
                    ty: ParamType::Choice {
                        options: &["warning", "error"],
                    },
                    default: Some(Value::Str("warning".to_owned())),
                    doc: "warning reports findings; error fails evaluation on the first finding.",
                },
            ],
        ),
        spec(
            "script.js@1",
            "A script is behaviour that runs against the live world and is saved with the document. `source` is the body of an async function with `world`, `on`, `every`, `log`, `time` and `random`; `name` is a unique lowercase identifier (empty uses this node's label), and `enabled` controls whether it runs. In the source string, `\\n` makes a newline and `\\\"` makes a quote.\nExample:\nbob = script.js@1(source=\"on('frame', (dt, t) => world.set('crate', 'Transform', {translation: [0, 0.2 * Math.sin(t * 3), 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1]}));\")",
            vec![],
            OutputSpec::Fixed(PortType::Check),
            vec![
                ParamSpec {
                    name: "source",
                    ty: ParamType::Str { max_len: 65_536 },
                    default: None,
                    doc: "Body of the async script function; 1 to 65,536 bytes and not blank.",
                },
                ParamSpec {
                    name: "name",
                    ty: ParamType::Str { max_len: 65_536 },
                    default: Some(Value::Str(String::new())),
                    doc: "Unique lowercase letters, digits and underscores, starting with a letter; empty uses the node label or id.",
                },
                ParamSpec {
                    name: "enabled",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(true)),
                    doc: "Whether this script should run.",
                },
            ],
        ),
    ]
}
