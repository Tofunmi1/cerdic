//! Ground editing and placement by hand: `terrain.flatten` (a level pad with a soft shoulder) and `terrain.place` (copies standing on the ground).
use super::{EXTENT, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn number(
    name: &'static str,
    unit: Unit,
    default: f64,
    min: f64,
    max: f64,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num { unit, min, max },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn flatten() -> Option<OperatorSpec> {
    spec(
        "terrain.flatten@1",
        "Levels a round pad in the ground of a terrain.tile, with a soft shoulder that blends back into the land: the place for a house, a camp, a road end or a landing. By default the pad keeps the height of the ground at its centre; set `to` to \"height\" to level it to a height you choose (for a shared floor level across several pads). Do it AFTER terrain.erode (erosion rebuilds the ground from its source and would undo the pad), before terrain.mask (masks made earlier no longer describe the pad), and before terrain.place so the buildings stand on the pad. Several flatten nodes in a row are fine.",
        vec![super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "at",
                ty: ParamType::Vec2 {
                    unit: Unit::Length,
                    min: -EXTENT,
                    max: EXTENT,
                },
                default: Some(Value::List(vec![Value::Num(0.0), Value::Num(0.0)])),
                doc: "World x and z of the pad's centre in metres (a tile covers tile_x * size.x to (tile_x + 1) * size.x).",
            },
            number(
                "radius",
                Unit::Length,
                8.0,
                0.5,
                2000.0,
                "Radius of the level part in metres: a building's half width plus a metre or two.",
            ),
            number(
                "blend",
                Unit::Length,
                6.0,
                0.0,
                2000.0,
                "Width of the shoulder around the pad in metres, where the ground eases back to the land (0 is a sharp step).",
            ),
            ParamSpec {
                name: "to",
                ty: ParamType::Choice {
                    options: &["centre", "height"],
                },
                default: Some(Value::Str("centre".to_owned())),
                doc: "centre: keep the ground's height at the pad's centre. height: level it to `height`.",
            },
            number(
                "height",
                Unit::Length,
                0.0,
                -1000.0,
                20000.0,
                "Pad height in metres above y = 0 when `to` is \"height\".",
            ),
        ],
    )
}

fn place_on() -> Option<OperatorSpec> {
    spec(
        "terrain.place@1",
        "Stands copies of any shape on the ground of a terrain.tile at x,z positions you list: a hut here, a well there, a lone tree. Each copy sits at the ground height under it (after terrain.flatten, on the pad), so you never need to know the height. Add yaw_scale for a turn and size per copy, align to lean with the slope, sink to bed it in. Positions must lie on the tile. The shape should have its base at y = 0. For many plants at random use scatter.place.",
        vec![
            super::single("in", &[PortType::Solid, PortType::Inst, PortType::Poly]),
            super::single("surface", &[PortType::Poly]),
        ],
        OutputSpec::Fixed(PortType::Inst),
        vec![
            ParamSpec {
                name: "at",
                ty: ParamType::Points2 {
                    min_points: 1,
                    max_points: 4096,
                    min: -EXTENT,
                    max: EXTENT,
                },
                default: None,
                doc: "Where each copy stands: [x, z] in world metres, one entry per copy.",
            },
            ParamSpec {
                name: "yaw_scale",
                ty: ParamType::Points2 {
                    min_points: 0,
                    max_points: 4096,
                    min: -3600.0,
                    max: 3600.0,
                },
                default: Some(Value::List(Vec::new())),
                doc: "Optional, one [yaw, scale] per copy in the same order as `at`: yaw in degrees about the vertical axis, scale a size factor above 0 (1 is unchanged).",
            },
            number(
                "align",
                Unit::Ratio,
                0.0,
                0.0,
                1.0,
                "How much each copy leans with the ground: 0 stands upright (buildings), 1 lies flat on the slope (rocks, moss).",
            ),
            number(
                "sink",
                Unit::Length,
                0.0,
                -10.0,
                10.0,
                "Push copies down into the ground by this many metres (a hut looks bedded in at 0.1).",
            ),
        ],
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![flatten(), place_on()]
}
