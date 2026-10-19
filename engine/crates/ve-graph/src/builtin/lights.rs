//! Authored scene lights.
use super::{EXTENT, spec, vec3};
use crate::spec::{OperatorSpec, OutputSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn colour() -> crate::spec::ParamSpec {
    vec3("color", Unit::None, 0.0, 1.0, "Light colour in sRGB.")
}

fn intensity() -> crate::spec::ParamSpec {
    crate::spec::ParamSpec {
        name: "intensity",
        ty: ParamType::Num {
            unit: Unit::None,
            min: 0.0,
            max: 100_000.0,
        },
        default: Some(Value::Num(10.0)),
        doc: "Radiant intensity; point and spot values are calibrated at one metre.",
    }
}

fn position() -> crate::spec::ParamSpec {
    vec3(
        "position",
        Unit::Length,
        -EXTENT,
        EXTENT,
        "World position in metres.",
    )
}

fn range() -> crate::spec::ParamSpec {
    crate::spec::ParamSpec {
        name: "range",
        ty: ParamType::Num {
            unit: Unit::Length,
            min: 0.001,
            max: EXTENT,
        },
        default: Some(Value::Num(8.0)),
        doc: "Distance in metres where light falloff reaches zero.",
    }
}

fn direction() -> crate::spec::ParamSpec {
    vec3(
        "direction",
        Unit::None,
        -1.0,
        1.0,
        "World direction; must be non-zero.",
    )
}

fn point() -> Option<OperatorSpec> {
    spec(
        "light.point@1",
        "A point light with bounded range.",
        vec![],
        OutputSpec::Fixed(PortType::Light),
        vec![position(), colour(), intensity(), range()],
    )
}

fn spot() -> Option<OperatorSpec> {
    spec(
        "light.spot@1",
        "A cone light with smooth inner and outer angles.",
        vec![],
        OutputSpec::Fixed(PortType::Light),
        vec![
            position(),
            direction(),
            colour(),
            intensity(),
            range(),
            crate::spec::ParamSpec {
                name: "inner_angle",
                ty: ParamType::Num {
                    unit: Unit::Angle,
                    min: 0.001,
                    max: 1.56,
                },
                default: Some(Value::Num(0.35)),
                doc: "Full cone angle in radians at full intensity.",
            },
            crate::spec::ParamSpec {
                name: "outer_angle",
                ty: ParamType::Num {
                    unit: Unit::Angle,
                    min: 0.002,
                    max: 1.57,
                },
                default: Some(Value::Num(0.6)),
                doc: "Full cone angle in radians where the beam reaches zero.",
            },
        ],
    )
}

fn sun() -> Option<OperatorSpec> {
    spec(
        "light.sun@1",
        "A directional sun; direction points from the light toward the scene.",
        vec![],
        OutputSpec::Fixed(PortType::Light),
        vec![direction(), colour(), intensity()],
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![point(), spot(), sun()]
}
