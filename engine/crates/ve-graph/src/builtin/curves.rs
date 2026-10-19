//! Bounded authored curve and first surface operator definitions.
use super::{EXTENT, PROFILE, SOLID, single, spec, variadic};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit};

const CURVE: &[PortType] = &[PortType::Curve];

#[expect(
    clippy::too_many_lines,
    reason = "Keep the small authored curve operator catalogue together."
)]
pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "curve.polyline@1",
            "A space curve through ordered points.",
            vec![],
            OutputSpec::Fixed(PortType::Curve),
            vec![
                ParamSpec {
                    name: "points",
                    ty: ParamType::Points3 {
                        min_points: 2,
                        max_points: 4096,
                        min: -EXTENT,
                        max: EXTENT,
                    },
                    default: None,
                    doc: "Ordered control points [x, y, z] in metres.",
                },
                ParamSpec {
                    name: "closed",
                    ty: ParamType::Bool,
                    default: Some(crate::value::Value::Bool(false)),
                    doc: "Connect the final sample back to the first to form a periodic path.",
                },
            ],
        ),
        spec(
            "curve.bezier@1",
            "A cubic Bézier curve sampled deterministically.",
            vec![],
            OutputSpec::Fixed(PortType::Curve),
            vec![
                ParamSpec {
                    name: "points",
                    ty: ParamType::Points3 {
                        min_points: 4,
                        max_points: 4,
                        min: -EXTENT,
                        max: EXTENT,
                    },
                    default: None,
                    doc: "Four cubic Bézier controls [x, y, z] in metres.",
                },
                ParamSpec {
                    name: "steps",
                    ty: ParamType::Int { min: 1, max: 512 },
                    default: Some(crate::value::Value::Int(24)),
                    doc: "Deterministic subdivisions along the cubic.",
                },
                ParamSpec {
                    name: "closed",
                    ty: ParamType::Bool,
                    default: Some(crate::value::Value::Bool(false)),
                    doc: "Require matching endpoints and make the sampled path periodic.",
                },
            ],
        ),
        circle(),
        spec(
            "solid.sweep@1",
            "Sweeps a single closed profile along a curve (a pipe, rope, rail, tentacle, horn). scale_end tapers the tube to a different size at the far end of an open curve (0.2 gives a pointed horn or tail) and twist turns the profile along the way (a twisted rope or ribbon).",
            vec![single("curve", CURVE), single("profile", PROFILE)],
            OutputSpec::Fixed(SOLID[0]),
            vec![
                ParamSpec {
                    name: "scale_end",
                    ty: ParamType::Num {
                        unit: Unit::Ratio,
                        min: 0.05,
                        max: 4.0,
                    },
                    default: Some(crate::value::Value::Num(1.0)),
                    doc: "Size of the profile at the end of the curve relative to the start; it changes steadily along the length. Open curves only.",
                },
                ParamSpec {
                    name: "twist",
                    ty: ParamType::Num {
                        unit: Unit::Angle,
                        min: -std::f64::consts::TAU * 8.0,
                        max: std::f64::consts::TAU * 8.0,
                    },
                    default: Some(crate::value::Value::Num(0.0)),
                    doc: "Total turn of the profile about the path from start to end, in radians. Open curves only; only visible for a profile that is not round.",
                },
            ],
        ),
        spec(
            "solid.loft@1",
            "Lofts matching convex profile loops at even spacing along Y; steps above 1 blend smoothly between them.",
            vec![variadic("sections", PROFILE)],
            OutputSpec::Fixed(SOLID[0]),
            vec![
                ParamSpec {
                    name: "spacing",
                    ty: ParamType::Num {
                        unit: crate::value::Unit::Length,
                        min: 1.0e-6,
                        max: EXTENT,
                    },
                    default: Some(crate::value::Value::Num(0.1)),
                    doc: "Distance between adjacent sections in metres.",
                },
                ParamSpec {
                    name: "steps",
                    ty: ParamType::Int { min: 1, max: 8 },
                    default: Some(crate::value::Value::Int(1)),
                    doc: "Rows per span between sections. 1 keeps straight segments (cones and cylinders); 4 to 8 blend a smooth curve through the sections, as for a vase belly.",
                },
            ],
        ),
    ]
}

fn circle() -> Option<OperatorSpec> {
    spec(
        "curve.circle@1",
        "A closed, evenly sampled circle in the XZ plane.",
        vec![],
        OutputSpec::Fixed(PortType::Curve),
        vec![
            ParamSpec {
                name: "center",
                ty: ParamType::Vec3 {
                    unit: crate::value::Unit::Length,
                    min: -EXTENT,
                    max: EXTENT,
                },
                default: Some(crate::value::Value::List(vec![
                    crate::value::Value::Num(0.0),
                    crate::value::Value::Num(0.0),
                    crate::value::Value::Num(0.0),
                ])),
                doc: "Circle center [x, y, z] in metres.",
            },
            ParamSpec {
                name: "radius",
                ty: ParamType::Num {
                    unit: crate::value::Unit::Length,
                    min: 1.0e-6,
                    max: EXTENT,
                },
                default: Some(crate::value::Value::Num(1.0)),
                doc: "Circle radius in metres.",
            },
            ParamSpec {
                name: "segments",
                ty: ParamType::Int { min: 8, max: 512 },
                default: Some(crate::value::Value::Int(48)),
                doc: "Number of unique samples around the closed circle.",
            },
        ],
    )
}
