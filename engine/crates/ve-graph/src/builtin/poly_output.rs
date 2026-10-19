//! Polygon export and UV layout operators.

use super::{single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const POLY: &[PortType] = &[PortType::Poly];

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "poly.pack_uv@1",
            "Packs existing corner UV islands into the 0..1 square; UV discontinuities define island boundaries.",
            vec![single("in", POLY)],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                flag("rotate", true, "Allow islands to rotate while packing."),
                flag(
                    "keep_density",
                    false,
                    "Preserve authored island scale and allow unused space.",
                ),
                choice(
                    "shape",
                    &["aabb", "convex"],
                    "aabb",
                    "Use axis-aligned bounds or convex hulls for packing.",
                ),
                number(
                    "margin",
                    Unit::None,
                    (0.0, 0.5),
                    Some(0.02),
                    "Fractional gap around islands.",
                ),
            ],
        ),
        spec(
            "poly.lod@1",
            "Builds one level of detail by repeatedly simplifying a polygon mesh; level 0 returns the input.",
            vec![single("in", POLY)],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                number(
                    "levels",
                    Unit::None,
                    (1.0, 4.0),
                    Some(3.0),
                    "Number of simplified levels in the chain.",
                ),
                number(
                    "level",
                    Unit::None,
                    (0.0, 4.0),
                    Some(1.0),
                    "Level to return; 0 is the input and 1..levels are progressively coarser.",
                ),
                number(
                    "ratio",
                    Unit::None,
                    (0.05, 0.9),
                    Some(0.5),
                    "Fraction of faces kept at each level.",
                ),
                flag(
                    "protect_uv_seams",
                    false,
                    "Preserve UV discontinuities while simplifying.",
                ),
                flag(
                    "protect_boundary",
                    false,
                    "Preserve open mesh boundaries while simplifying.",
                ),
            ],
        ),
    ]
}

fn flag(name: &'static str, default: bool, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Bool,
        default: Some(Value::Bool(default)),
        doc,
    }
}

fn choice(
    name: &'static str,
    options: &'static [&'static str],
    default: &'static str,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Choice { options },
        default: Some(Value::Str(default.to_owned())),
        doc,
    }
}

fn number(
    name: &'static str,
    unit: Unit,
    range: (f64, f64),
    default: Option<f64>,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit,
            min: range.0,
            max: range.1,
        },
        default: default.map(Value::Num),
        doc,
    }
}
