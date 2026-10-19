//! Shared instances; unioning patterns remain separate operators.
use super::{EXTENT, single, spec};
use crate::{OperatorSpec, OutputSpec, ParamSpec, ParamType, PortType, Unit, Value};
/// What can be placed: solids, other sets, and polygon meshes (open ones, like leaf cards, are placed without ever being unioned).
const INPUT: &[PortType] = &[PortType::Solid, PortType::Inst, PortType::Poly];
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
fn seed() -> ParamSpec {
    ParamSpec {
        name: "seed",
        ty: ParamType::Int {
            min: 0,
            max: 2_147_483_647,
        },
        default: Some(Value::Int(0)),
        doc: "Authored random seed; the same seed gives the same result on every platform.",
    }
}
fn amount(
    name: &'static str,
    unit: Unit,
    max: f64,
    default: Option<f64>,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit,
            min: 0.0,
            max,
        },
        default: default.map(Value::Num),
        doc,
    }
}
fn variation() -> [ParamSpec; 2] {
    [
        amount(
            "yaw_jitter",
            Unit::Angle,
            std::f64::consts::PI,
            Some(0.0),
            "Largest random turn about Y, either way.",
        ),
        amount(
            "scale_jitter",
            Unit::Ratio,
            0.9,
            Some(0.0),
            "Uniform scale varies by up to this fraction either way (0.2 means 0.8 to 1.2).",
        ),
    ]
}
fn variation_ops() -> Vec<Option<OperatorSpec>> {
    let output = OutputSpec::Fixed(PortType::Inst);
    let [yaw, scale] = variation();
    let [yaw2, scale2] = variation();
    vec![
        spec(
            "inst.scatter@1",
            "Seeded scatter over a width x depth rectangle in the XZ plane centred on the origin; \
             min_spacing keeps centres apart or the node refuses.",
            vec![single("in", INPUT)],
            output,
            vec![
                ParamSpec {
                    name: "count",
                    ty: ParamType::Int {
                        min: 1,
                        max: 16_384,
                    },
                    default: None,
                    doc: "Number of placements; refused rather than reduced when the spacing cannot be met.",
                },
                amount("width", Unit::Length, EXTENT, None, "Extent along X."),
                amount("depth", Unit::Length, EXTENT, None, "Extent along Z."),
                amount(
                    "min_spacing",
                    Unit::Length,
                    EXTENT,
                    Some(0.0),
                    "Smallest centre distance.",
                ),
                seed(),
                yaw,
                scale,
            ],
        ),
        spec(
            "inst.jitter@1",
            "Varies each placement by a bounded random shift, turn and scale; element i depends only on the seed and i.",
            vec![single("in", INPUT)],
            output,
            vec![vector("shift", Unit::Length, true), seed(), yaw2, scale2],
        ),
        spec(
            "inst.scatter_on@1",
            "Seeded scatter over the surface of a solid, in any orientation (pebbles on a hill's sides, bolts on a tank, moss on a wall): points are drawn uniformly by area over `surface`, and with align each copy's +Y axis follows the surface normal there. min_distance keeps copies apart or the node refuses (it never places fewer than count). region limits it to upward, sideways or downward faces. Build the prototype with its base at y=0 so it sits on the surface.",
            vec![single("in", INPUT), single("surface", &[PortType::Solid])],
            output,
            vec![
                ParamSpec {
                    name: "count",
                    ty: ParamType::Int { min: 1, max: 5_000 },
                    default: Some(Value::Int(100)),
                    doc: "Number of placements; refused rather than reduced when min_distance cannot be met.",
                },
                amount(
                    "min_distance",
                    Unit::Length,
                    EXTENT,
                    Some(0.0),
                    "Smallest distance between two placements, in metres (0 for none).",
                ),
                ParamSpec {
                    name: "align",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(true)),
                    doc: "Turn each copy so its +Y axis is the surface normal; false keeps the prototype's orientation.",
                },
                ParamSpec {
                    name: "spin",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(true)),
                    doc: "Turn each copy by a seeded random angle about its own up axis.",
                },
                ParamSpec {
                    name: "tilt",
                    ty: ParamType::Num {
                        unit: Unit::Angle,
                        min: 0.0,
                        max: std::f64::consts::FRAC_PI_4,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Largest seeded lean away from the surface normal (0 to 45 degrees).",
                },
                ParamSpec {
                    name: "scale_jitter",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: 0.0,
                        max: 1.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Scale each copy by 1 plus or minus up to this fraction.",
                },
                ParamSpec {
                    name: "region",
                    ty: ParamType::Choice {
                        options: &["all", "up", "side", "down"],
                    },
                    default: Some(Value::Str("all".to_owned())),
                    doc: "Where on the surface: all, up (normal pointing up), side, or down.",
                },
                seed(),
            ],
        ),
        spec(
            "inst.drop@1",
            "Sets every placement down onto a surface (trees, rocks and grass on a terrain; props on a table): each copy slides straight down onto the highest point of `surface` under it. Scatter over the surface's footprint first, then drop. A copy is dropped from where its own origin (the prototype's origin after placement) is, so build the prototype with its base at y=0. Copies that are not over the surface are removed.",
            vec![single("in", INPUT), single("surface", &[PortType::Solid])],
            output,
            vec![
                ParamSpec {
                    name: "offset",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: -100.0,
                        max: 100.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Extra height above (or sink below, if negative) the surface in metres; a small negative value buries a tree trunk's base.",
                },
                ParamSpec {
                    name: "align",
                    ty: ParamType::Num {
                        unit: Unit::Ratio,
                        min: 0.0,
                        max: 1.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "0 keeps copies upright; 1 tilts each to stand square on the slope under it; in between tilts part of the way.",
                },
                ParamSpec {
                    name: "outside",
                    ty: ParamType::Choice {
                        options: &["remove", "keep"],
                    },
                    default: Some(Value::Str("remove".to_owned())),
                    doc: "What to do with copies not over the surface: remove them, or keep them where they were.",
                },
            ],
        ),
    ]
}
pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let output = OutputSpec::Fixed(PortType::Inst);
    let mut ops = variation_ops();
    ops.extend(base_ops(output));
    ops
}
fn base_ops(output: OutputSpec) -> Vec<Option<OperatorSpec>> {
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
