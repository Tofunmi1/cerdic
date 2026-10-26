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
        "For lakes on a landscape use water.fill, which follows the hollow and shoreline. A closed, transparent water body with a seeded wavy rest mesh and a lightweight animated viewer ripple. Exported geometry is static; it is not a physics body. Use solid.material after this node to override its water material.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        {
            let mut params = parameters();
            params.extend(sim_parameters());
            params
        },
    )]
}

fn parameters() -> Vec<ParamSpec> {
    vec![
        size_param(),
        ratio(
            "ripple",
            1.0,
            0.0,
            2.0,
            "Wave amplitude multiplier; zero is a flat animated sheet.",
        ),
        ratio(
            "flow",
            1.0,
            0.0,
            4.0,
            "Wave travel speed multiplier; zero freezes the waves.",
        ),
        ParamSpec {
            name: "direction",
            ty: ParamType::Num {
                unit: Unit::Angle,
                min: -std::f64::consts::TAU,
                max: std::f64::consts::TAU,
            },
            default: Some(Value::Num(0.0)),
            doc: "Main wave travel direction in degrees; zero is positive X.",
        },
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
            doc: "Picks repeatable rest-surface and travelling-wave patterns.",
        },
        ParamSpec {
            name: "resolution",
            ty: ParamType::Int { min: 4, max: 192 },
            default: Some(Value::Int(64)),
            doc: "Grid cells per side, capped at 192 (at most 73,728 triangles).",
        },
    ]
}

/// Shared interactive water controls.
pub(super) fn sim_parameters() -> Vec<ParamSpec> {
    vec![
        ParamSpec {
            name: "sim",
            ty: ParamType::Bool,
            default: Some(Value::Bool(false)),
            doc: "Enable interactive shallow-water ripples.",
        },
        ParamSpec {
            name: "sim_resolution",
            ty: ParamType::Int { min: 32, max: 256 },
            default: Some(Value::Int(128)),
            doc: "Simulation cells along the longer side.",
        },
        ratio("sim_damping", 0.6, 0.0, 5.0, "Velocity damping per second."),
        length(
            "sim_depth",
            1.0,
            0.05,
            50.0,
            "Fallback physical depth in metres, setting ripple speed.",
        ),
        ratio(
            "rain",
            0.0,
            0.0,
            1.0,
            "Seeded drops per second per 100 square metres of water.",
        ),
    ]
}
