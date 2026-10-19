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
            "curve.tube@1",
            "A round tube along a curve for trunks, branches, vines, pipes, roots, and cables. The radius runs from radius to end_radius along the length (a taper), or through the stations of radii. \
             The radius is set at the curve's own points, so give the curve enough points for the shape you want (curve.resample adds them; a two-point curve can only taper in a straight line). \
             The tube does not twist as it winds and keeps its width at bends; a turn sharper than 150 degrees is refused (round the curve first with curve.fillet). A closed curve makes a ring.",
            vec![single("curve", CURVE)],
            OutputSpec::Fixed(SOLID[0]),
            vec![
                ParamSpec {
                    name: "radius",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.0,
                        max: 100.0,
                    },
                    default: Some(crate::value::Value::Num(0.1)),
                    doc: "Radius at the start in metres.",
                },
                ParamSpec {
                    name: "end_radius",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.0,
                        max: 100.0,
                    },
                    default: Some(crate::value::Value::Num(0.0)),
                    doc: "Radius at the end; zero keeps the start radius.",
                },
                ParamSpec {
                    name: "sides",
                    ty: ParamType::Int { min: 3, max: 64 },
                    default: Some(crate::value::Value::Int(8)),
                    doc: "Number of sides around the tube.",
                },
                ParamSpec {
                    name: "caps",
                    ty: ParamType::Bool,
                    default: Some(crate::value::Value::Bool(true)),
                    doc: "Flat end caps on an open curve. Leave this on: a tube with open ends is not a solid and is refused. A closed curve has no ends.",
                },
                ParamSpec {
                    name: "radii",
                    ty: ParamType::Str { max_len: 256 },
                    default: Some(crate::value::Value::Str(String::new())),
                    doc: "Optional radii in metres, 2 to 16 separated by commas, at equal steps along the length, for example \"0.3, 0.12, 0.1\" for a flared base. Replaces radius and end_radius.",
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
