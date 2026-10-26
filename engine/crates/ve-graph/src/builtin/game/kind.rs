//! Entity kinds and their optional rolling and tool capabilities.
use super::{OperatorSpec, ParamType, TEXT, Unit, Value, game_node, param};

pub(super) fn kind() -> Option<OperatorSpec> {
    game_node(
        "game.kind@1",
        "Declare an entity kind. Examples: `hero = game.kind@1(name=\"player\", size=0.5, speed=4, tags=[\"hero\"])`; `ball = game.kind@1(name=\"ball\", size=0.03, roll=true)` rolls on a table or field; `tool = game.kind@1(name=\"tool\", swing=true, reach=0.9, loft=20, power=1.2)` swings into it; `pocket = game.kind@1(name=\"pocket\", size=0.07, pocket=true)` captures it.",
        vec![
            param("name", TEXT, None, "Kind name."),
            param(
                "size",
                ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 1.0e6,
                },
                Some(Value::Num(0.5)),
                "Collision radius in metres.",
            ),
            param(
                "speed",
                ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 1.0e6,
                },
                Some(Value::Num(0.0)),
                "Movement speed in metres per second.",
            ),
            param(
                "tags",
                ParamType::StrList {
                    max_items: 64,
                    max_len: 64,
                },
                Some(Value::List(Vec::new())),
                "Tags used by selectors.",
            ),
            param(
                "part",
                TEXT,
                Some(Value::Str(String::new())),
                "Scene output that draws this kind; empty makes it invisible.",
            ),
            param(
                "solid",
                ParamType::Vec3 {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 1.0e3,
                },
                Some(Value::List(vec![Value::Num(0.0); 3])),
                "Full width, height and depth in metres of a box centred on the entity that the hero cannot walk into (a door, a wall, a gate); [0, 0, 0] makes the kind not an obstacle. The box disappears when the entity is destroyed.",
            ),
            param(
                "grab",
                ParamType::Bool,
                Some(Value::Bool(false)),
                "True makes the kind grabbable: a squeezed grip within reach (the hand's size plus this kind's size) picks up the nearest one and carries it until the grip opens (events grab and release); carried things still touch sockets, doors and each other. A hand is an entity too: give it a kind, an input with follow=\"left_hand\" or \"right_hand\", and a part to draw it.",
            ),
            param(
                "fall",
                ParamType::Bool,
                Some(Value::Bool(false)),
                "Apply gravity and ground bounces.",
            ),
            param(
                "bounce",
                ParamType::Num {
                    unit: Unit::None,
                    min: 0.0,
                    max: 0.95,
                },
                Some(Value::Num(0.3)),
                "Fraction of vertical speed retained on bounce.",
            ),
            param(
                "roll",
                ParamType::Bool,
                Some(Value::Bool(false)),
                "Rolling thing follows the ground on a table or field.",
            ),
            param(
                "swing",
                ParamType::Bool,
                Some(Value::Bool(false)),
                "A hand-held tool swings its swept head into rolling things.",
            ),
            param(
                "reach",
                ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 1.0e9,
                },
                Some(Value::Num(0.0)),
                "Tool head reach in metres along the hand's local -Z axis.",
            ),
            param(
                "loft",
                ParamType::Num {
                    unit: Unit::None,
                    min: 0.0,
                    max: 80.0,
                },
                Some(Value::Num(0.0)),
                "Upward launch angle in degrees.",
            ),
            param(
                "power",
                ParamType::Num {
                    unit: Unit::None,
                    min: 0.0,
                    max: 5.0,
                },
                Some(Value::Num(1.0)),
                "Swing power multiplier, greater than zero and at most 5.",
            ),
            param(
                "pocket",
                ParamType::Bool,
                Some(Value::Bool(false)),
                "Kind is a pocket that captures slow grounded rolling things.",
            ),
        ],
    )
}
