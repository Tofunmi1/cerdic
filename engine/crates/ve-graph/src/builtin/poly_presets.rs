//! Intent-level polygon operators: one node for what a model would otherwise build from several low-level ones.

use super::{EXTENT, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const POLY: &[PortType] = &[PortType::Poly];

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![round_edges()]
}

fn round_edges() -> Option<OperatorSpec> {
    spec(
        "poly.round_edges@1",
        "Rounds every hard edge of a polygon mesh in one step: the edges between faces turned apart by at least angle get a bevel of the given width. \
         Replaces poly.select by=sharp followed by poly.bevel_network. Merge a solid's coplanar triangles first (poly.from_solid then poly.merge_coplanar), \
         otherwise every triangle edge counts as hard. Refuses with selection.lost when no edge is sharp enough.",
        vec![single("in", POLY)],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "width",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 1.0e-6,
                    max: EXTENT,
                },
                default: None,
                doc: "Bevel size in metres; clamped so a bevel never overlaps itself.",
            },
            ParamSpec {
                name: "segments",
                ty: ParamType::Int { min: 1, max: 16 },
                default: Some(Value::Int(3)),
                doc: "Segments across each bevel: 1 is a flat chamfer, more round it.",
            },
            ParamSpec {
                name: "angle",
                ty: ParamType::Num {
                    unit: Unit::Angle,
                    min: 0.0,
                    max: std::f64::consts::PI,
                },
                default: Some(Value::Num(0.5)),
                doc: "The least turn between two faces' normals, in radians, for their edge to count as hard (0.5 is about 30 degrees).",
            },
        ],
    )
}
