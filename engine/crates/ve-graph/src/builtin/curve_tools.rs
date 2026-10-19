//! Curve generators and modifiers: spline, helix, rounded corners, resample, reverse.
use super::{EXTENT, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const CURVE: &[PortType] = &[PortType::Curve];

fn length(name: &'static str, default: f64, min: f64, max: f64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Length,
            min,
            max,
        },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn whole(name: &'static str, default: i64, min: i64, max: i64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Int { min, max },
        default: Some(Value::Int(default)),
        doc,
    }
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let curve_out = || OutputSpec::Fixed(PortType::Curve);
    vec![
        spec(
            "curve.spline@1",
            "A smooth curve that passes through every point you give (a centripetal Catmull-Rom spline: no loops or overshoot between points). Use it for a winding path, a pipe or rope route, a rail, an organic outline. Feed it to solid.sweep with a profile.",
            vec![],
            curve_out(),
            vec![
                ParamSpec {
                    name: "points",
                    ty: ParamType::Points3 {
                        min_points: 2,
                        max_points: 256,
                        min: -EXTENT,
                        max: EXTENT,
                    },
                    default: None,
                    doc: "Points [x, y, z] in metres the curve passes through, in order.",
                },
                whole(
                    "steps",
                    8,
                    1,
                    64,
                    "Samples between each pair of points; more is smoother.",
                ),
                ParamSpec {
                    name: "closed",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(false)),
                    doc: "Join the last point back to the first (needs at least 3 points).",
                },
            ],
        ),
        spec(
            "curve.helix@1",
            "A helix about a vertical axis (a spring, a corkscrew, a spiral staircase rail, a coil). Differing end_radius makes a cone-shaped spiral. Rises from center's height by pitch for every turn. Feed it to solid.sweep with a small profile.",
            vec![],
            curve_out(),
            vec![
                ParamSpec {
                    name: "center",
                    ty: ParamType::Vec3 {
                        unit: Unit::Length,
                        min: -EXTENT,
                        max: EXTENT,
                    },
                    default: Some(Value::List(vec![
                        Value::Num(0.0),
                        Value::Num(0.0),
                        Value::Num(0.0),
                    ])),
                    doc: "The point on the axis where the helix starts.",
                },
                length(
                    "radius",
                    0.5,
                    0.001,
                    1000.0,
                    "Radius at the start in metres.",
                ),
                length(
                    "end_radius",
                    0.5,
                    0.001,
                    1000.0,
                    "Radius at the end; set it different from radius for a conical spiral.",
                ),
                length(
                    "pitch",
                    0.2,
                    0.0,
                    1000.0,
                    "How far it rises per full turn, in metres (0 gives a flat circular spiral).",
                ),
                whole("turns", 3, 1, 64, "Number of full turns."),
                whole(
                    "segments_per_turn",
                    16,
                    4,
                    128,
                    "Straight pieces per turn; more is rounder.",
                ),
            ],
        ),
        spec(
            "curve.fillet@1",
            "Rounds the corners of a curve (a polyline path with sharp bends becomes a smooth pipe route). Each interior corner is replaced by a smooth curve that starts `distance` before the corner and ends `distance` after it.",
            vec![single("curve", CURVE)],
            curve_out(),
            vec![
                length(
                    "distance",
                    0.1,
                    0.0001,
                    1000.0,
                    "How far from each corner the rounding starts, in metres; shortened automatically on short segments.",
                ),
                whole("steps", 6, 1, 32, "Samples per rounded corner."),
            ],
        ),
        spec(
            "curve.resample@1",
            "Replaces a curve by evenly spaced points along its length (same shape, uniform spacing), for even rungs, rails and repeats.",
            vec![single("curve", CURVE)],
            curve_out(),
            vec![whole(
                "count",
                16,
                2,
                8192,
                "How many points; the first and last stay at the ends of an open curve.",
            )],
        ),
        spec(
            "curve.reverse@1",
            "Reverses the direction of a curve (swap which end is the start), which flips which end a sweep starts from.",
            vec![single("curve", CURVE)],
            curve_out(),
            vec![],
        ),
    ]
}
