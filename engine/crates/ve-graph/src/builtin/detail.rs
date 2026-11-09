//! Surface detail made of real geometry: panel lines and greebles on a face, and lit text.

use super::{SOLID, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn number(name: &'static str, unit: Unit, range: (f64, f64), default: f64, doc: &'static str) -> ParamSpec {
    ParamSpec { name, ty: ParamType::Num { unit, min: range.0, max: range.1 }, default: Some(Value::Num(default)), doc }
}

fn count(name: &'static str, max: i64, default: i64, doc: &'static str) -> ParamSpec {
    ParamSpec { name, ty: ParamType::Int { min: 1, max }, default: Some(Value::Int(default)), doc }
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "detail.panels@1",
            "Panel lines and greebles on one face of a solid: a grid of plates laid on the face, some standing out, some vented, the seams between them left open. It makes a NEW solid of just the plates (the input is not part of it), sitting exactly on the face so it touches the wall (join it with solid.union, texture it with material.preset, check it with check.attached). `face` is +x -x +y -y +z or -z: that side of the input's bounding box, covered edge to edge. `cols` and `rows` (1 to 32) are the grid along the face's two directions; `gap` is the seam between plates in metres; `depth` is the thickness of a flat plate; `raised` is the share (0 to 1) of plates that stand out 1.5 to 3 times `depth`; `vents` is the share of plates drawn as a vent (a flat plate with 4 to 6 slats on it); a plate is never both. The seed decides which plates, the same seed gives the same plates. Example: `p = detail.panels@1(wall, face=\"+z\", cols=6, rows=4, raised=0.3, vents=0.1, seed=3)`.",
            vec![single("in", SOLID)],
            OutputSpec::Fixed(PortType::Solid),
            vec![
                ParamSpec {
                    name: "face",
                    ty: ParamType::Choice { options: &["+x", "-x", "+y", "-y", "+z", "-z"] },
                    default: Some(Value::Str("+z".to_owned())),
                    doc: "Which side of the input's bounding box the plates cover.",
                },
                count("cols", 32, 4, "Plates along the face's first direction (x for +z and -z, z for +x and -x, x for +y and -y)."),
                count("rows", 32, 3, "Plates along the face's second direction (y for the x and z faces, z for the y faces)."),
                number("gap", Unit::Length, (0.0, 1.0), 0.02, "Seam between neighbouring plates in metres; each plate is inset by half of it on every side. A gap as wide as a plate is refused."),
                number("depth", Unit::Length, (0.001, 1.0), 0.04, "Thickness of a flat plate in metres."),
                number("raised", Unit::Ratio, (0.0, 1.0), 0.3, "Share of plates that stand out, each 1.5 to 3 times `depth` thick."),
                number("vents", Unit::Ratio, (0.0, 1.0), 0.1, "Share of plates drawn as a vent: a flat plate with 4 to 6 slats standing on it."),
                ParamSpec {
                    name: "seed",
                    ty: ParamType::Int { min: 0, max: 2_147_483_647 },
                    default: Some(Value::Int(1)),
                    doc: "Chooses which plates are raised or vented and how tall.",
                },
            ],
        ),
        spec(
            "rooms.plan@1",
            "A building from a list of rooms and the links between them: it lays the rooms out around a corridor (every room gets a door onto it, every link is a short walk), then makes a floor slab under every room and corridor, a wall along every edge, and an opening where each door is. There is no roof, so the inside shows and can be lit. The plan starts at the origin (x and z from 0) and stands on y = 0. `rooms` lists the rooms as `name:minWidth,minDepth,maxWidth,maxDepth` in metres separated by semicolons; `links` lists which rooms must connect as `0-1; 1-2` (room numbers from 0, the links must join every room); the seed decides the sizes, the order along the corridor and the door places. Wall and floor are one solid: texture it, or cut and panel it further. Example: `b = rooms.plan@1(rooms=\"hall:5,5,7,7; office:3,3,4,4; store:3,3,5,5\", links=\"0-1; 0-2\", seed=3)`.",
            Vec::new(),
            OutputSpec::Fixed(PortType::Solid),
            vec![
                ParamSpec {
                    name: "rooms",
                    ty: ParamType::Str { max_len: 2048 },
                    default: Some(Value::Str("hall:5,5,7,7; office:3,3,4,4; store:3,3,5,5".to_owned())),
                    doc: "The rooms: `name:minWidth,minDepth,maxWidth,maxDepth` in metres, separated by semicolons (1 to 64 rooms).",
                },
                ParamSpec {
                    name: "links",
                    ty: ParamType::Str { max_len: 1024 },
                    default: Some(Value::Str("0-1; 0-2".to_owned())),
                    doc: "Pairs of room numbers that must be a short walk apart, like `0-1; 1-2`; together they must connect every room.",
                },
                ParamSpec {
                    name: "seed",
                    ty: ParamType::Int { min: 0, max: 2_147_483_647 },
                    default: Some(Value::Int(1)),
                    doc: "Chooses the room sizes, their order along the corridor and where the doors are.",
                },
                number("wall_height", Unit::Length, (1.0, 10.0), 2.8, "Height of the walls in metres."),
                number("wall", Unit::Length, (0.05, 1.0), 0.15, "Thickness of the walls in metres."),
                number("corridor_width", Unit::Length, (0.8, 4.0), 1.6, "Narrowest corridor in metres."),
                number("door_width", Unit::Length, (0.6, 2.5), 1.0, "Narrowest door in metres."),
            ],
        ),
        spec(
            "sign.text@1",
            "Lit text as geometry, for neon and LED signs: the letters (a 5 by 7 pixel font) become small boxes, centred on x = 0, standing on y = 0 (the bottom row at y = 0), reaching from z = 0 to z = `depth`, facing +z. The node sets a dark housing colour and an emissive glow in `color` (emissive = color x glow, so glow 0 is unlit); a later solid.material or material.preset overrides it. There is no backing plate: make one with solid.box behind the letters. `height` is the height of the letters in metres, `depth` how far they stand out. Text of 1 to 48 characters with at least one lit pixel. Example: `s = sign.text@1(text=\"OPEN\", height=0.6, color=[0.1, 1, 1], glow=4)`.",
            Vec::new(),
            OutputSpec::Fixed(PortType::Solid),
            vec![
                ParamSpec {
                    name: "text",
                    ty: ParamType::Str { max_len: 48 },
                    default: Some(Value::Str("SIGN".to_owned())),
                    doc: "The words, 1 to 48 characters (capitals, digits and common punctuation).",
                },
                number("height", Unit::Length, (0.05, 10.0), 0.5, "Height of the letters in metres."),
                number("depth", Unit::Length, (0.005, 1.0), 0.04, "How far the letters stand out, in metres."),
                ParamSpec {
                    name: "color",
                    ty: ParamType::Vec3 { unit: Unit::None, min: 0.0, max: 100.0 },
                    default: Some(Value::List(vec![Value::Num(1.0), Value::Num(0.1), Value::Num(0.6)])),
                    doc: "Colour of the letters [r, g, b]; the housing is this colour scaled to 0.15, the glow is this colour times `glow`.",
                },
                number("glow", Unit::None, (0.0, 20.0), 3.0, "Brightness multiplier of the emissive light; 0 is unlit, above 1 blooms."),
            ],
        ),
    ]
}
