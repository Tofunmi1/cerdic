//! More primitives (cone, torus, capsule), two more profiles (rect, circle) and the patterns that
//! turn one solid into several (mirror, array, radial, repeat).

use std::f64::consts::TAU;

use super::{EXTENT, XFORMABLE, length, segments, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn count(name: &'static str, min: i64, default: Option<i64>, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Int { min, max: 256 },
        default: default.map(Value::Int),
        doc,
    }
}

fn vec3(
    name: &'static str,
    unit: Unit,
    limit: f64,
    default: Option<Value>,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 {
            unit,
            min: -limit,
            max: limit,
        },
        default,
        doc,
    }
}

fn zero3() -> Value {
    Value::List(vec![Value::Num(0.0); 3])
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut all = profiles();
    all.extend(primitives());
    all.extend(edge_treatment());
    all.extend(patterns());
    all.extend(circular_patterns());
    all
}

fn profiles() -> Vec<Option<OperatorSpec>> {
    let profile = OutputSpec::Fixed(PortType::Profile);
    vec![
        spec(
            "profile.rect@1",
            "A rectangle centred on the origin of the XY plane.",
            vec![],
            profile,
            vec![ParamSpec {
                name: "size",
                ty: ParamType::Vec2 {
                    unit: Unit::Length,
                    min: 1.0e-6,
                    max: EXTENT,
                },
                default: None,
                doc: "Width and height in metres.",
            }],
        ),
        spec(
            "profile.circle@1",
            "A regular polygon approximating a circle, centred on the origin of the XY plane.",
            vec![],
            profile,
            vec![length("radius", "Radius in metres."), segments(32, 512)],
        ),
    ]
}

fn primitives() -> Vec<Option<OperatorSpec>> {
    let solid = OutputSpec::Fixed(PortType::Solid);
    vec![
        spec(
            "solid.cone@1",
            "A cone or truncated cone standing along Y, centred on the origin.",
            vec![],
            solid,
            vec![
                length("height", "Height in metres."),
                length("radius", "Radius at the bottom in metres."),
                ParamSpec {
                    name: "top_radius",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.0,
                        max: EXTENT,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Radius at the top; 0 makes a point.",
                },
                segments(32, 512),
            ],
        ),
        spec(
            "solid.torus@1",
            "A ring (doughnut) lying in the XZ plane, centred on the origin.",
            vec![],
            solid,
            vec![
                length(
                    "major",
                    "Distance from the centre to the middle of the tube, in metres.",
                ),
                length(
                    "minor",
                    "Radius of the tube in metres; must be smaller than major.",
                ),
                segments(48, 512),
                ParamSpec {
                    name: "tube_segments",
                    ty: ParamType::Int { min: 3, max: 256 },
                    default: Some(Value::Int(24)),
                    doc: "Facets around the tube.",
                },
            ],
        ),
        spec(
            "solid.capsule@1",
            "A cylinder with rounded ends standing along Y, centred on the origin.",
            vec![],
            solid,
            vec![
                length("radius", "Radius in metres."),
                length(
                    "height",
                    "Total length including both ends, at least twice the radius.",
                ),
                segments(24, 512),
            ],
        ),
    ]
}

/// Edge treatment that acts on a whole solid: the two bounded Minkowski operations.
fn edge_treatment() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "solid.round@1",
            "Rounds a closed solid by eroding then expanding it with a small sphere; bounded to 20,000 input and 250,000 output triangles.",
            vec![single("in", XFORMABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                length(
                    "radius",
                    "Edge radius in metres; less than half the thinnest solid dimension.",
                ),
                ParamSpec {
                    name: "segments",
                    ty: ParamType::Int { min: 6, max: 24 },
                    default: Some(Value::Int(12)),
                    doc: "Sphere profile resolution; higher values cost more kernel time.",
                },
            ],
        ),
        spec(
            "solid.chamfer@1",
            "Breaks convex edges with a flat 45-degree facet by eroding then expanding the solid with an octahedron; bounded to 20,000 input and 250,000 output triangles. Concave edges stay sharp.",
            vec![single("in", XFORMABLE)],
            OutputSpec::SameAsInput("in"),
            vec![length(
                "width",
                "Chamfer width in metres; less than half the thinnest solid dimension.",
            )],
        ),
    ]
}

fn patterns() -> Vec<Option<OperatorSpec>> {
    let same = OutputSpec::SameAsInput("in");
    vec![
        spec(
            "xform.mirror@1",
            "Adds the mirror image of a solid across the plane through the origin perpendicular to an axis.",
            vec![single("in", XFORMABLE)],
            same,
            vec![ParamSpec {
                name: "axis",
                ty: ParamType::Choice {
                    options: &["x", "y", "z"],
                },
                default: None,
                doc: "The axis the mirror plane is perpendicular to.",
            }],
        ),
        spec(
            "xform.array@1",
            "Copies a solid into a grid; the first copy stays where it is.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                count("count_x", 1, Some(1), "Copies along X."),
                count("count_y", 1, Some(1), "Copies along Y."),
                count("count_z", 1, Some(1), "Copies along Z."),
                vec3(
                    "spacing",
                    Unit::Length,
                    EXTENT,
                    None,
                    "Distance between neighbours along each axis, in metres.",
                ),
            ],
        ),
    ]
}

/// The patterns that turn around the Y axis or walk along a path.
fn circular_patterns() -> Vec<Option<OperatorSpec>> {
    let same = OutputSpec::SameAsInput("in");
    vec![
        spec(
            "xform.radial@1",
            "Copies a solid evenly around the Y axis; each copy is first moved out along X.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                count("count", 2, None, "Number of copies."),
                ParamSpec {
                    name: "radius",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.0,
                        max: EXTENT,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "How far out along X each copy is moved before it is turned.",
                },
            ],
        ),
        spec(
            "xform.repeat@1",
            "Copies a solid along a path: copy i is turned by i times rotate_step, then moved by i times step.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                count(
                    "count",
                    2,
                    None,
                    "Number of copies, including the original.",
                ),
                vec3(
                    "step",
                    Unit::Length,
                    EXTENT,
                    Some(zero3()),
                    "Movement per copy, in metres.",
                ),
                vec3(
                    "rotate_step",
                    Unit::Angle,
                    TAU,
                    Some(zero3()),
                    "Rotation per copy about X, then Y, then Z.",
                ),
            ],
        ),
    ]
}
