//! Walkable level node specification.
use super::{NUMBER, OperatorSpec, ParamType, Value, game_node, param};

pub(super) fn nav() -> Option<OperatorSpec> {
    let mut params = vec![param(
        "parts",
        ParamType::StrList {
            max_items: 2048,
            max_len: 4096,
        },
        None,
        "Walkable floor, ramp and stair part names.",
    )];
    for (key, value, doc) in [
        ("cell", 0.25, "Positive grid cell size in metres."),
        ("radius", 0.3, "Nonnegative agent radius in metres."),
        ("height", 1.8, "Positive agent height in metres."),
        ("step", 0.4, "Nonnegative maximum step height in metres."),
        ("slope", 45.0, "Maximum slope in degrees, between 0 and 90."),
    ] {
        params.push(param(key, NUMBER, Some(Value::Num(value)), doc));
    }
    game_node(
        "game.nav@1",
        "Declare the level parts to build navigation from. Example: `nav = game.nav@1(parts=[\"floor\", \"stairs\"], cell=0.2)`.",
        params,
    )
}
