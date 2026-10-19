//! Procedural water surface generator.
use super::spec;
use crate::spec::{OutputSpec, ParamSpec, PortType};
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

fn ratio(name: &'static str, default: f64, min: f64, max: f64, doc: &'static str) -> ParamSpec {
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

fn size_param() -> ParamSpec {
    ParamSpec {
        name: "size",
        ty: ParamType::Vec2 {
            unit: Unit::Length,
            min: 0.1,
            max: 2000.0,
        },
        default: Some(Value::List(vec![Value::Num(20.0), Value::Num(20.0)])),
        doc: "Water width along X and length along Z, in metres.",
    }
}

fn color_param() -> ParamSpec {
    ParamSpec {
        name: "color",
        ty: ParamType::Vec3 {
            unit: Unit::None,
            min: 0.0,
            max: 1.0,
        },
        default: Some(Value::List(vec![
            Value::Num(0.12),
            Value::Num(0.38),
            Value::Num(0.52),
        ])),
        doc: "Water tint in sRGB.",
    }
}

pub(super) fn all() -> Vec<Option<crate::spec::OperatorSpec>> {
    vec![spec(
        "water.surface@1",
        "A closed, transparent water body with a seeded wavy rest mesh and a lightweight animated viewer ripple. Exported geometry is static; it is not a physics body. Use solid.material after this node to override its water material.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        parameters(),
    )]
}

fn parameters() -> Vec<ParamSpec> {
    vec![
        size_param(),
        length(
            "level",
            0.0,
            -10000.0,
            10000.0,
            "Water surface height in world metres.",
        ),
        length(
            "depth",
            1.0,
            0.05,
            1000.0,
            "How far the water body extends below the surface.",
        ),
        color_param(),
        ratio(
            "opacity",
            0.6,
            0.0,
            1.0,
            "Water transparency: 0 is clear, 0.6 is water, 1 is opaque.",
        ),
        length(
            "wave_height",
            0.08,
            0.0,
            5.0,
            "Maximum seeded surface displacement in metres.",
        ),
        length(
            "wave_length",
            3.0,
            0.1,
            1000.0,
            "Scale of the broad surface undulation in metres.",
        ),
        ratio(
            "speed",
            1.0,
            0.0,
            10.0,
            "Viewer ripple speed; zero pauses the ripple.",
        ),
        ParamSpec {
            name: "seed",
            ty: ParamType::Int {
                min: 0,
                max: 1_000_000,
            },
            default: Some(Value::Int(0)),
            doc: "Picks a repeatable rest-surface pattern.",
        },
        ParamSpec {
            name: "resolution",
            ty: ParamType::Int { min: 4, max: 192 },
            default: Some(Value::Int(64)),
            doc: "Grid cells per side, capped at 192 (at most 73,728 triangles).",
        },
    ]
}
