//! Shared instances; unioning patterns remain separate operators.
use super::{EXTENT, single, spec};
use crate::{OperatorSpec, OutputSpec, ParamSpec, ParamType, PortType, Unit, Value};
const INPUT: &[PortType] = &[PortType::Solid, PortType::Inst];
fn count(name: &'static str, default: Option<i64>) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Int {
            min: 1,
            max: 100_000,
        },
        default: default.map(Value::Int),
        doc: "Placement count; products must fit the instance budget.",
    }
}
fn vector(name: &'static str, unit: Unit, default: bool) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 {
            unit,
            min: -EXTENT,
            max: EXTENT,
        },
        default: default.then(|| Value::List(vec![Value::Num(0.0); 3])),
        doc: "Vector in metres or radians.",
    }
}
pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let output = OutputSpec::Fixed(PortType::Inst);
    vec![
        spec(
            "inst.array@1",
            "Shared grid; no union.",
            vec![single("in", INPUT)],
            output,
            vec![
                count("count_x", Some(1)),
                count("count_y", Some(1)),
                count("count_z", Some(1)),
                vector("spacing", Unit::Length, false),
            ],
        ),
        spec(
            "inst.repeat@1",
            "Shared copies rotated then translated; no union.",
            vec![single("in", INPUT)],
            output,
            vec![
                count("count", None),
                vector("step", Unit::Length, true),
                vector("rotate_step", Unit::Angle, true),
            ],
        ),
        spec(
            "inst.radial@1",
            "Shared copies around Y; no union.",
            vec![single("in", INPUT)],
            output,
            vec![
                count("count", None),
                ParamSpec {
                    name: "radius",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.0,
                        max: EXTENT,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Initial X offset.",
                },
            ],
        ),
        spec(
            "inst.mirror@1",
            "Original placements and their reflection, with corrected winding.",
            vec![single("in", INPUT)],
            output,
            vec![ParamSpec {
                name: "axis",
                ty: ParamType::Choice {
                    options: &["x", "y", "z"],
                },
                default: None,
                doc: "Reflection plane normal.",
            }],
        ),
        spec(
            "inst.realize@1",
            "Explicitly union placements, after geometry/storage preflight.",
            vec![single("in", &[PortType::Inst])],
            OutputSpec::Fixed(PortType::Solid),
            vec![],
        ),
    ]
}
