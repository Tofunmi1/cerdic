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
mod rig;

fn appearance() -> Vec<Option<OperatorSpec>> {
    [
        appearance::specs(),
        animation::specs(),
        rig::specs(),
        behavior::specs(),
    ]
    .concat()
}

fn constraints() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "check.min_wall@1",
        "Fails when the wall between an outer and an inner profile is thinner than `min`.",
        vec![single("outer", PROFILE), single("inner", PROFILE)],
        OutputSpec::Fixed(PortType::Check),
        vec![length("min", "Smallest allowed wall thickness in metres.")],
    )]
}
