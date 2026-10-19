//! Terrain generator.
use super::spec;
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn length(name: &'static str, default: f64, min: f64, max: f64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Length,
            min,
            max,
        },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn fraction(name: &'static str, default: f64, min: f64, max: f64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Ratio,
            min,
            max,
        },
        default: Some(Value::Num(default)),
        doc,
    }
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "terrain.heightfield@1",
        "A landscape: a seeded fractal ground (rolling hills, ridged mountains, an island) as one closed solid slab, centred on the origin with its lowest ground at y=0 or above and a flat base under it. It is an ordinary solid, so it takes solid.material, can be cut with booleans, and can carry copies placed by inst.drop. Same seed, same land.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        vec![
            ParamSpec {
                name: "size",
                ty: ParamType::Vec2 {
                    unit: Unit::Length,
                    min: 1.0,
                    max: 2000.0,
                },
                default: Some(Value::List(vec![Value::Num(20.0), Value::Num(20.0)])),
                doc: "Width along X and depth along Z in metres.",
            },
            length(
                "height",
                3.0,
                0.0,
                500.0,
                "Tallest ground above the base level, in metres.",
            ),
            length(
                "scale",
                8.0,
                0.5,
                1000.0,
                "Size of the biggest hills in metres; smaller gives more, tighter hills.",
            ),
            ParamSpec {
                name: "octaves",
                ty: ParamType::Int { min: 1, max: 6 },
                default: Some(Value::Int(4)),
                doc: "Layers of finer detail (1 is smooth blobs, 5 or 6 is craggy).",
            },
            fraction(
                "roughness",
                0.5,
                0.1,
                0.9,
                "How strong each finer layer is relative to the one before (higher is rougher).",
            ),
            ParamSpec {
                name: "seed",
                ty: ParamType::Int {
                    min: 0,
                    max: 1_000_000,
                },
                default: Some(Value::Int(0)),
                doc: "Picks a different landscape.",
            },
            ParamSpec {
                name: "resolution",
                ty: ParamType::Int { min: 4, max: 192 },
                default: Some(Value::Int(64)),
                doc: "Grid cells along each side (the ground has 2 x resolution x resolution triangles; 64 is smooth enough for a 20 m field, use 128 for large scenes).",
            },
            length(
                "base",
                0.5,
                0.05,
                100.0,
                "Thickness of the slab below y=0 in metres.",
            ),
            ParamSpec {
                name: "style",
                ty: ParamType::Choice {
                    options: &["hills", "ridged"],
                },
                default: Some(Value::Str("hills".to_owned())),
                doc: "hills: soft rolling ground. ridged: sharp mountain ridges.",
            },
            fraction(
                "edge_falloff",
                0.0,
                0.0,
                1.0,
                "0 keeps the full height to the edge; 0.5 or more slopes the land down toward the edges (an island or a cut-out landscape).",
            ),
            fraction(
                "flat_below",
                0.0,
                0.0,
                0.95,
                "Fraction of the height below which the ground is flattened: hollows become flat beds you can fill with a water slab at that level.",
            ),
        ],
    )]
}
