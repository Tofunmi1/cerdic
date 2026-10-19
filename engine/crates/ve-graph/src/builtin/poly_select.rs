//! `poly.select`: the operator that chooses faces, edges, vertices or boundary loops of a polygon mesh by a rule.

use super::{EXTENT, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const POLY: &[PortType] = &[PortType::Poly];

fn triple(x: f64, y: f64, z: f64) -> Value {
    Value::List(vec![Value::Num(x), Value::Num(y), Value::Num(z)])
}

pub(super) fn select() -> Option<OperatorSpec> {
    let mut params = rule_params();
    params.extend(position_params());
    params.extend(filter_params());
    spec(
        "poly.select@1",
        "Chooses faces, edges, vertices or boundary loops of a polygon mesh by a rule, for the edit operators. Choosing nothing refuses (selection.lost); expect=one refuses when several match (selection.ambiguous).",
        vec![single("in", POLY)],
        OutputSpec::Fixed(PortType::Selection),
        params,
    )
}

/// What to choose and by which rule.
fn rule_params() -> Vec<ParamSpec> {
    vec![
        ParamSpec {
            name: "element",
            ty: ParamType::Choice {
                options: &["face", "edge", "vertex", "loop"],
            },
            default: Some(Value::Str("face".into())),
            doc: "What to choose.",
        },
        ParamSpec {
            name: "by",
            ty: ParamType::Choice {
                options: &[
                    "all",
                    "normal",
                    "box",
                    "attribute",
                    "sharp",
                    "height",
                    "extreme",
                ],
            },
            default: Some(Value::Str("all".into())),
            doc: "The rule: all; normal (faces facing a direction); box (centre inside a box); attribute (faces or points with a tagged value); sharp (edges between faces turned apart by an angle); height (centre at one coordinate along an axis, give height and tolerance); extreme (the highest or lowest elements along an axis, give side).",
        },
        ParamSpec {
            name: "direction",
            ty: ParamType::Vec3 {
                unit: Unit::None,
                min: -1.0e6,
                max: 1.0e6,
            },
            default: Some(triple(0.0, 1.0, 0.0)),
            doc: "For by=normal: the direction the faces face (any length).",
        },
        ParamSpec {
            name: "angle",
            ty: ParamType::Num {
                unit: Unit::Angle,
                min: 0.0,
                max: std::f64::consts::PI,
            },
            default: Some(Value::Num(0.1)),
            doc: "For by=normal: how far a face may turn from the direction. For by=sharp: the least angle between the two faces' normals.",
        },
    ]
}

/// The coordinate rules: `height` and `extreme`.
fn position_params() -> Vec<ParamSpec> {
    vec![
        ParamSpec {
            name: "axis",
            ty: ParamType::Choice {
                options: &["x", "y", "z"],
            },
            default: Some(Value::Str("y".into())),
            doc: "For by=height and by=extreme: the axis the coordinate is read along (y is up).",
        },
        ParamSpec {
            name: "height",
            ty: ParamType::Num {
                unit: Unit::Length,
                min: -EXTENT,
                max: EXTENT,
            },
            default: Some(Value::Num(0.0)),
            doc: "For by=height: the coordinate along the axis, in metres. Faces, edges and loops are tested at their centre.",
        },
        ParamSpec {
            name: "tolerance",
            ty: ParamType::Num {
                unit: Unit::Length,
                min: 0.0,
                max: EXTENT,
            },
            default: Some(Value::Num(0.0001)),
            doc: "For by=height and by=extreme: how far from the target coordinate still counts, in metres.",
        },
        ParamSpec {
            name: "side",
            ty: ParamType::Choice {
                options: &["max", "min"],
            },
            default: Some(Value::Str("max".into())),
            doc: "For by=extreme: max chooses the elements furthest along the axis (the top for y), min the ones furthest back (the bottom).",
        },
    ]
}

/// The box, attribute and count filters.
fn filter_params() -> Vec<ParamSpec> {
    vec![
        ParamSpec {
            name: "box_min",
            ty: ParamType::Vec3 {
                unit: Unit::Length,
                min: -EXTENT,
                max: EXTENT,
            },
            default: Some(triple(-1.0e5, -1.0e5, -1.0e5)),
            doc: "For by=box: the lower corner. Faces, edges and loops are tested at their centre.",
        },
        ParamSpec {
            name: "box_max",
            ty: ParamType::Vec3 {
                unit: Unit::Length,
                min: -EXTENT,
                max: EXTENT,
            },
            default: Some(triple(1.0e5, 1.0e5, 1.0e5)),
            doc: "For by=box: the upper corner.",
        },
        ParamSpec {
            name: "attribute",
            ty: ParamType::Str { max_len: 64 },
            default: Some(Value::Str("region".into())),
            doc: "For by=attribute: the name of an integer face or point attribute (region is the colour region a solid came from).",
        },
        ParamSpec {
            name: "value",
            ty: ParamType::Int {
                min: 0,
                max: 2_147_483_647,
            },
            default: Some(Value::Int(0)),
            doc: "For by=attribute: the value to match.",
        },
        ParamSpec {
            name: "expect",
            ty: ParamType::Choice {
                options: &["any", "one"],
            },
            default: Some(Value::Str("any".into())),
            doc: "any accepts one or more matches; one refuses with selection.ambiguous when several match.",
        },
    ]
}
