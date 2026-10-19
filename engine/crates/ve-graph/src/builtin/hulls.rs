//! `solid.hull` and `solid.collision_boxes`: replace a solid by its convex hull or by the simple
//! shape that fits it best, both built on `ve-measure`.

use super::{SOLID, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Value};

/// Points a hull accepts when `max_vertices` is not given.
const DEFAULT_MAX_VERTICES: i64 = 256;
/// Most points a hull may be asked to accept; keeps a collision hull small enough to simulate.
const MAX_VERTICES: i64 = 2000;

fn max_vertices() -> ParamSpec {
    ParamSpec {
        name: "max_vertices",
        ty: ParamType::Int {
            min: 4,
            max: MAX_VERTICES,
        },
        default: Some(Value::Int(DEFAULT_MAX_VERTICES)),
        doc: "Most points the hull accepts; a solid with more is refused, so decimate it first or raise this.",
    }
}

fn shape() -> ParamSpec {
    ParamSpec {
        name: "shape",
        ty: ParamType::Choice {
            options: &["best", "box", "oriented_box", "capsule"],
        },
        default: Some(Value::Str("best".to_owned())),
        doc: "Which fitted shape to build: best (the tightest of the three), an axis-aligned box, an oriented box, or a capsule.",
    }
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let solid = OutputSpec::Fixed(PortType::Solid);
    vec![
        spec(
            "solid.hull@1",
            "Replaces a solid by its convex hull: the smallest convex shape that contains every one of its points, as a closed solid. \
             Refused when the solid has more points than max_vertices (decimate it first); the hull is at most 2 * max_vertices triangles.",
            vec![single("in", SOLID)],
            solid,
            vec![max_vertices()],
        ),
        spec(
            "solid.collision_boxes@1",
            "Replaces a solid by the simple shape that fits it best, for collision and level-of-detail: the axis-aligned box, the \
             oriented box (from the solid's principal axes), or a capsule along its longest axis. Emits a warning naming the chosen \
             shape and its volume ratio to the solid.",
            vec![single("in", SOLID)],
            solid,
            vec![shape()],
        ),
    ]
}
