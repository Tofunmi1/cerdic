//! Rigid assembly operations, independent of group interface versioning.
use super::{EXTENT, OperatorSpec, OutputSpec, PortType, SOLID, Unit, Value, single, spec, vec3};

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut pivot = vec3(
        "pivot",
        Unit::Length,
        -EXTENT,
        EXTENT,
        "Fixed point in metres.",
    );
    pivot.default = Some(Value::List(vec![Value::Int(0); 3]));
    vec![spec(
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
    )]
}
