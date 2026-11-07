//! Liquids: a block of water, oil or honey that runs live.

use super::spec;
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

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

fn vec3(name: &'static str, min: f64, max: f64, default: [f64; 3], doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 {
            unit: Unit::Length,
            min,
            max,
        },
        default: Some(Value::List(
            default.iter().map(|v| Value::Num(*v)).collect(),
        )),
        doc,
    }
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "fluid.sim@1",
        "A liquid that runs while the scene runs: water pouring into a basin, a wave in a tank, honey sliding down a ramp. `min` and `max` are the corners of the block of liquid at the start; `spacing` is how far apart its particles are (0.05 m with a 0.4 x 0.5 x 0.4 m block is 640 particles; keep it between about 1,000 and 8,000 particles for 60 frames a second). `bounds_min` and `bounds_max` are the container: particles stay between them (set them to the tank's inside; a floor is the lower y). `viscosity` makes it thick (water 0.02, oil 0.2, honey 0.6). `collide_bones` names bone parts (rig.bone) whose capsules push the liquid, so a moving paddle or limb stirs it. The result is the liquid's surface as a smooth polygon mesh that the viewer rebuilds every frame from the particles, so give it a material (solid.material with a blue colour and low roughness reads as water). It is deterministic. No particle-to-solid-mesh collision: obstacles are the container box and bone capsules.",
        Vec::new(),
        OutputSpec::Fixed(PortType::Poly),
        vec![
            vec3(
                "min",
                -1000.0,
                1000.0,
                [0.0, 0.0, 0.0],
                "Lower corner of the block of liquid at the start, in metres.",
            ),
            vec3(
                "max",
                -1000.0,
                1000.0,
                [0.4, 0.5, 0.4],
                "Upper corner of the block of liquid at the start.",
            ),
            num(
                "spacing",
                Unit::Length,
                (0.01, 0.5),
                0.05,
                "Distance between particles at rest, in metres; smaller is finer and slower.",
            ),
            vec3(
                "bounds_min",
                -100000.0,
                100000.0,
                [-50.0, 0.0, -50.0],
                "Lower corner of the container the liquid stays in (the floor is its y).",
            ),
            vec3(
                "bounds_max",
                -100000.0,
                100000.0,
                [50.0, 100.0, 50.0],
                "Upper corner of the container.",
            ),
            num(
                "gravity",
                Unit::None,
                (0.0, 50.0),
                9.81,
                "Downward acceleration in metres per second squared.",
            ),
            num(
                "viscosity",
                Unit::Ratio,
                (0.0, 1.0),
                0.02,
                "How thick it is: velocity blending with neighbours (water 0.02, honey 0.6).",
            ),
            num(
                "friction",
                Unit::Ratio,
                (0.0, 1.0),
                0.1,
                "How much it grips the container and bones.",
            ),
            ParamSpec {
                name: "substeps",
                ty: ParamType::Int { min: 1, max: 8 },
                default: Some(Value::Int(3)),
                doc: "Solver steps per frame: more is steadier but slower.",
            },
            ParamSpec {
                name: "iterations",
                ty: ParamType::Int { min: 1, max: 8 },
                default: Some(Value::Int(4)),
                doc: "Density passes per step: more keeps the volume better but is slower.",
            },
            ParamSpec {
                name: "collide_bones",
                ty: ParamType::Str { max_len: 512 },
                default: Some(Value::Str(String::new())),
                doc: "Output names of bone parts (commas or semicolons, at most 16) that push the liquid as capsules.",
            },
            num(
                "collider_radius",
                Unit::Length,
                (0.0, 2.0),
                0.07,
                "Radius of each collider capsule around its bone, in metres.",
            ),
            num(
                "cell",
                Unit::Length,
                (0.0, 0.5),
                0.0,
                "Grid size the surface is meshed on; 0 picks 0.7 times the spacing. Smaller is smoother and costs more.",
            ),
            num(
                "level",
                Unit::Ratio,
                (0.2, 0.8),
                0.4,
                "Where the surface sits in the particle density: lower swells the liquid, higher shrinks it.",
            ),
            ParamSpec {
                name: "method",
                ty: ParamType::Choice {
                    options: &["pbf", "apic"],
                },
                default: Some(Value::Str("pbf".to_owned())),
                doc: "How the liquid is solved. pbf: position-based, light and cheap (up to 20,000 particles), a bit like a springy gel. apic: a pressure-projected grid and particle solver that keeps its volume and sloshes, splashes and settles like real water (up to 60,000 particles; the container's `bounds` should be tight, because the solver's grid covers them; with huge bounds it covers a window round the liquid); `iterations` scales how hard it solves the pressure and `substeps` caps how often a frame is split.",
            },
            ParamSpec {
                name: "max_triangles",
                ty: ParamType::Int {
                    min: 200,
                    max: 12000,
                },
                default: Some(Value::Int(4000)),
                doc: "Most triangles the surface may have while it runs; the part is padded to this so it can grow.",
            },
        ],
    )]
}
