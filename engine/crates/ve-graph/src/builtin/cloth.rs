//! Cloth: panels draped over bodies at evaluation time.

use super::{single, spec};
use crate::spec::{Arity, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const POLY: &[PortType] = &[PortType::Poly];
const SELECTION: &[PortType] = &[PortType::Selection];
const COLLIDER: &[PortType] = &[PortType::Solid, PortType::Poly, PortType::Volume];

fn num(
    name: &'static str,
    unit: Unit,
    range: (f64, f64),
    default: f64,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit,
            min: range.0,
            max: range.1,
        },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn live() -> Option<OperatorSpec> {
    spec(
        "cloth.sim@1",
        "Makes a garment live cloth that moves while the scene runs: a cape that streams behind a running figure, a banner in the wind, a skirt that swings. The garment is a polygon mesh (made with poly.grid or any polygon node, shaped as you like); it keeps its shape in the editor and is simulated every frame in the viewer. `pins` is a vertex selection (poly.select element=vertex, for example the top edge) held to the bone named in `pin_bone`, so it moves with that bone and the rest hangs and swings from it; with no `pin_bone` the pins stay fixed in the world (a flag on a pole that does not move). `collide_bones` names bone parts (made with rig.bone) whose capsules push the cloth away, so it drapes over a limb and cannot pass through it. Wind (`wind` and `drag`) blows through it. The garment must have no animation tracks of its own: animate the bone it is pinned to. Up to 6,000 points; the viewer steps it at 60 frames a second. It is deterministic. Self-collision is not modelled.",
        vec![
            single("in", POLY),
            PortSpec {
                name: "pins",
                accepts: SELECTION,
                arity: Arity::Optional,
            },
        ],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "pin_bone",
                ty: ParamType::Str { max_len: 64 },
                default: Some(Value::Str(String::new())),
                doc: "Output name of the bone part the pinned points follow; empty keeps them fixed in the world.",
            },
            ParamSpec {
                name: "collide_bones",
                ty: ParamType::Str { max_len: 512 },
                default: Some(Value::Str(String::new())),
                doc: "Output names of bone parts (separated by commas or semicolons, at most 16) that push the cloth away as capsules.",
            },
            num(
                "collider_radius",
                Unit::Length,
                (0.0, 2.0),
                0.07,
                "Radius of each collider capsule around its bone, in metres.",
            ),
            ParamSpec {
                name: "substeps",
                ty: ParamType::Int { min: 2, max: 32 },
                default: Some(Value::Int(6)),
                doc: "Solver steps per frame: more is stiffer and steadier but slower.",
            },
            num(
                "density",
                Unit::None,
                (0.01, 10.0),
                0.2,
                "Mass per square metre in kilograms.",
            ),
            num(
                "stretch",
                Unit::None,
                (0.0, 1.0),
                0.0,
                "How much the cloth can stretch: 0 does not.",
            ),
            num(
                "bend",
                Unit::None,
                (0.0, 1.0),
                0.02,
                "How easily it folds: 0 is stiff like card, larger is limp like silk.",
            ),
            num(
                "thickness",
                Unit::Length,
                (0.0, 0.2),
                0.01,
                "How far the cloth keeps from colliders, in metres.",
            ),
            num(
                "friction",
                Unit::Ratio,
                (0.0, 1.0),
                0.5,
                "How much it grips what it lies on.",
            ),
            num(
                "damping",
                Unit::None,
                (0.0, 5.0),
                0.5,
                "Speed lost per second.",
            ),
            num(
                "gravity",
                Unit::None,
                (0.0, 50.0),
                9.81,
                "Downward acceleration in metres per second squared.",
            ),
            ParamSpec {
                name: "wind",
                ty: ParamType::Vec3 {
                    unit: Unit::None,
                    min: -100.0,
                    max: 100.0,
                },
                default: Some(Value::List(vec![Value::Num(0.0); 3])),
                doc: "Wind velocity in metres per second (needs drag above 0 to act).",
            },
            num(
                "drag",
                Unit::None,
                (0.0, 10.0),
                0.0,
                "How hard air pushes on the cloth; 0 turns the wind off.",
            ),
            num(
                "floor",
                Unit::Length,
                (-1.0e6, 1.0e6),
                -1.0e6,
                "Height of a floor the cloth cannot sink through, in metres; the default means none.",
            ),
        ],
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        live(),
        spec(
            "cloth.drape@1",
            "Drops a flat panel onto a body and lets it settle under gravity, like cloth: a cape over shoulders, a tunic over a torso, a sheet over a table. The panel is a polygon mesh (a poly.grid, bent or moved into place above the body); the result keeps its faces and UVs and only its points move. `with` is what it falls on: any closed mesh (the body, a table) or a volume, and the panel must start outside it. `pins` is a vertex selection (poly.select element=vertex) held in place, for a collar or waistband that does not fall. It simulates `seconds` of real time at 60 steps a second with the same solver every cloth node uses; it is deterministic, so the same panel on the same body always drapes the same way. Soft cloth: raise `bend` (more folds) and `density`; stiff cloth: lower them. Up to 20,000 points. Self-collision is not modelled, so a panel that folds onto itself can pass through itself.",
            vec![
                single("in", POLY),
                PortSpec {
                    name: "pins",
                    accepts: SELECTION,
                    arity: Arity::Optional,
                },
                PortSpec {
                    name: "with",
                    accepts: COLLIDER,
                    arity: Arity::Optional,
                },
            ],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                num(
                    "seconds",
                    Unit::None,
                    (0.1, 20.0),
                    3.0,
                    "How long it is left to settle, in seconds.",
                ),
                ParamSpec {
                    name: "substeps",
                    ty: ParamType::Int { min: 2, max: 32 },
                    default: Some(Value::Int(8)),
                    doc: "Solver steps per frame: more is stiffer and steadier but slower.",
                },
                num(
                    "density",
                    Unit::None,
                    (0.01, 10.0),
                    0.2,
                    "Mass per square metre in kilograms (a light cotton is about 0.15, canvas about 0.4).",
                ),
                num(
                    "stretch",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "How much the cloth can stretch: 0 does not, larger gives like elastic.",
                ),
                num(
                    "bend",
                    Unit::None,
                    (0.0, 1.0),
                    0.02,
                    "How easily it folds: 0 is stiff like card, larger is limp like silk.",
                ),
                num(
                    "thickness",
                    Unit::Length,
                    (0.0, 0.2),
                    0.01,
                    "How far the cloth keeps from the body, in metres.",
                ),
                num(
                    "friction",
                    Unit::Ratio,
                    (0.0, 1.0),
                    0.5,
                    "How much it grips what it lies on: 0 slides freely, 1 sticks.",
                ),
                num(
                    "damping",
                    Unit::None,
                    (0.0, 5.0),
                    0.5,
                    "Speed lost per second: higher settles sooner and moves less.",
                ),
                num(
                    "gravity",
                    Unit::None,
                    (0.0, 50.0),
                    9.81,
                    "Downward acceleration in metres per second squared.",
                ),
                ParamSpec {
                    name: "wind",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: -100.0,
                        max: 100.0,
                    },
                    default: Some(Value::List(vec![Value::Num(0.0); 3])),
                    doc: "Wind velocity in metres per second (needs drag above 0 to act).",
                },
                num(
                    "drag",
                    Unit::None,
                    (0.0, 10.0),
                    0.0,
                    "How hard air pushes on the cloth; 0 turns the wind off.",
                ),
                num(
                    "voxel_size",
                    Unit::Length,
                    (0.005, 0.5),
                    0.03,
                    "How finely the body is sampled as a collider, in metres; smaller catches thin parts, larger is faster.",
                ),
                num(
                    "floor",
                    Unit::Length,
                    (-1.0e6, 1.0e6),
                    -1.0e6,
                    "Height of a floor the cloth cannot sink through, in metres; leave at the default for none.",
                ),
            ],
        ),
    ]
}
