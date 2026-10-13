//! Rigid assembly operations, independent of group interface versioning.
use super::{
    EXTENT, OperatorSpec, OutputSpec, ParamSpec, ParamType, PortType, SOLID, Unit, Value, single,
    spec, vec3,
};

fn name(name: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Str { max_len: 64 },
        default: None,
        doc,
    }
}

fn zero(name: &'static str, unit: Unit, doc: &'static str) -> ParamSpec {
    let mut param = vec3(name, unit, -EXTENT, EXTENT, doc);
    param.default = Some(Value::List(vec![Value::Int(0); 3]));
    param
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut pivot = vec3(
        "pivot",
        Unit::Length,
        -EXTENT,
        EXTENT,
        "Fixed point in metres.",
    );
    pivot.default = Some(Value::List(vec![Value::Int(0); 3]));
    vec![
        spec(
            "xform.align@1",
            "Rotates a direction onto another about a fixed pivot; zero directions are refused.",
            vec![single("in", SOLID)],
            OutputSpec::Fixed(PortType::Solid),
            vec![
                vec3(
                    "from",
                    Unit::Ratio,
                    -EXTENT,
                    EXTENT,
                    "Nonzero source direction.",
                ),
                vec3(
                    "to",
                    Unit::Ratio,
                    -EXTENT,
                    EXTENT,
                    "Nonzero target direction.",
                ),
                pivot,
            ],
        ),
        spec(
            "frame.add@1",
            "Declares one unique named rigid socket; rotations are X, then Y, then Z.",
            vec![single("in", SOLID)],
            OutputSpec::Fixed(PortType::Solid),
            vec![
                name("name", "Unique socket name."),
                zero("position", Unit::Length, "Position in metres."),
                zero(
                    "rotation",
                    Unit::Angle,
                    "Rotation in radians, X then Y then Z.",
                ),
            ],
        ),
        spec(
            "xform.attach@1",
            "Moves and rotates the source so its named socket coincides with the target socket.",
            vec![single("in", SOLID), single("to", SOLID)],
            OutputSpec::Fixed(PortType::Solid),
            vec![
                name("socket", "Source socket name."),
                name("to_socket", "Target socket name."),
            ],
        ),
    ]
}
