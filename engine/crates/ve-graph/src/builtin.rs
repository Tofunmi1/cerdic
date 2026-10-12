//! The operators known to documents from the start. They are `Draft`: documents can hold them and
//! commands validate against them, but evaluation and provider schemas wait for their conformance
//! cases (G3 onward).

use crate::spec::{
    Arity, OpId, OperatorRegistry, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType,
    Readiness,
};
use crate::value::{ParamType, Unit, Value};

/// The largest size or offset accepted, in metres (the kernel's own extent limit).
const EXTENT: f64 = 1.0e6;

const SOLID: &[PortType] = &[PortType::Solid];
const PROFILE: &[PortType] = &[PortType::Profile];
const ANY: &[PortType] = &[
    PortType::Solid,
    PortType::Profile,
    PortType::Curve,
    PortType::Sdf,
    PortType::Inst,
];

fn length(name: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Length,
            min: 1.0e-6,
            max: EXTENT,
        },
        default: None,
        doc,
    }
}

fn segments(default: i64, max: i64) -> ParamSpec {
    ParamSpec {
        name: "segments",
        ty: ParamType::Int { min: 3, max },
        default: Some(Value::Int(default)),
        doc: "Facets around the axis.",
    }
}

fn vec3(name: &'static str, unit: Unit, min: f64, max: f64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 { unit, min, max },
        default: None,
        doc,
    }
}

fn spec(
    id: &str,
    doc: &'static str,
    inputs: Vec<PortSpec>,
    output: OutputSpec,
    params: Vec<ParamSpec>,
) -> Option<OperatorSpec> {
    Some(OperatorSpec {
        id: OpId::parse(id).ok()?,
        doc,
        inputs,
        output,
        params,
        readiness: Readiness::Draft,
    })
}

fn variadic(name: &'static str, accepts: &'static [PortType]) -> PortSpec {
    PortSpec {
        name,
        accepts,
        arity: Arity::Variadic { min: 2 },
    }
}

fn single(name: &'static str, accepts: &'static [PortType]) -> PortSpec {
    PortSpec {
        name,
        accepts,
        arity: Arity::One,
    }
}

fn all_specs() -> Vec<Option<OperatorSpec>> {
    let mut all = primitives();
    all.extend(booleans());
    all.extend(transforms());
    all.extend(profiles());
    all
}

fn primitives() -> Vec<Option<OperatorSpec>> {
    let solid = OutputSpec::Fixed(PortType::Solid);
    vec![
        spec(
            "solid.box@1",
            "Axis-aligned box centred on the origin.",
            vec![],
            solid,
            vec![vec3(
                "size",
                Unit::Length,
                1.0e-6,
                EXTENT,
                "Width, height and depth in metres.",
            )],
        ),
        spec(
            "solid.sphere@1",
            "Sphere centred on the origin.",
            vec![],
            solid,
            vec![length("radius", "Radius in metres."), segments(24, 512)],
        ),
        spec(
            "solid.cylinder@1",
            "Cylinder standing along Y, centred on the origin.",
            vec![],
            solid,
            vec![
                length("radius", "Radius in metres."),
                length("height", "Height in metres."),
                segments(24, 512),
            ],
        ),
    ]
}

fn booleans() -> Vec<Option<OperatorSpec>> {
    let solid = OutputSpec::Fixed(PortType::Solid);
    vec![
        spec(
            "solid.union@1",
            "Joins solids into one.",
            vec![variadic("in", SOLID)],
            solid,
            vec![],
        ),
        spec(
            "solid.difference@1",
            "The first solid minus all the others.",
            vec![variadic("in", SOLID)],
            solid,
            vec![],
        ),
        spec(
            "solid.intersect@1",
            "The volume shared by all solids.",
            vec![variadic("in", SOLID)],
            solid,
            vec![],
        ),
    ]
}

fn transforms() -> Vec<Option<OperatorSpec>> {
    let same = OutputSpec::SameAsInput("in");
    vec![
        spec(
            "xform.move@1",
            "Moves its input.",
            vec![single("in", ANY)],
            same,
            vec![vec3(
                "by",
                Unit::Length,
                -EXTENT,
                EXTENT,
                "Offset in metres.",
            )],
        ),
        spec(
            "xform.rotate@1",
            "Rotates its input about the origin (X, then Y, then Z).",
            vec![single("in", ANY)],
            same,
            vec![vec3(
                "angles",
                Unit::Angle,
                -1.0e4,
                1.0e4,
                "Rotation about each axis in radians.",
            )],
        ),
        spec(
            "xform.scale@1",
            "Scales its input about the origin.",
            vec![single("in", ANY)],
            same,
            vec![vec3(
                "by",
                Unit::Ratio,
                1.0e-6,
                1.0e6,
                "Scale factor per axis; positive only.",
            )],
        ),
    ]
}

fn profiles() -> Vec<Option<OperatorSpec>> {
    let solid = OutputSpec::Fixed(PortType::Solid);
    vec![
        spec(
            "profile.polygon@1",
            "A closed polygon from points in the XY plane.",
            vec![],
            OutputSpec::Fixed(PortType::Profile),
            vec![ParamSpec {
                name: "points",
                ty: ParamType::Points2 {
                    min_points: 3,
                    max_points: 4096,
                    min: -EXTENT,
                    max: EXTENT,
                },
                default: None,
                doc: "Corner points [x, y] in metres.",
            }],
        ),
        spec(
            "solid.revolve@1",
            "Spins a profile about the Y axis.",
            vec![single("in", PROFILE)],
            solid,
            vec![
                ParamSpec {
                    name: "angle",
                    ty: ParamType::Num {
                        unit: Unit::Angle,
                        min: 1.0e-3,
                        max: std::f64::consts::TAU,
                    },
                    default: Some(Value::Num(std::f64::consts::TAU)),
                    doc: "Sweep angle in radians.",
                },
                segments(32, 512),
            ],
        ),
    ]
}

/// The registry of operators every document can use.
#[must_use]
pub fn builtin_registry() -> OperatorRegistry {
    let mut registry = OperatorRegistry::new();
    for spec in all_specs().into_iter().flatten() {
        // The list above has no duplicates; a failed registration would leave the operator unknown
        // and the first command using it would report `parse.unknown_op`.
        let _ = registry.register(spec);
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_registers_and_is_a_draft() {
        let specs = all_specs();
        assert!(specs.iter().all(Option::is_some));
        let registry = builtin_registry();
        assert_eq!(registry.iter().count(), specs.len());
        assert_eq!(registry.ready().count(), 0);
    }

    #[test]
    fn defaults_satisfy_their_own_types() {
        for spec in builtin_registry().iter() {
            for param in &spec.params {
                if let Some(default) = &param.default {
                    assert!(
                        param.ty.check(default).is_ok(),
                        "{} {}",
                        spec.id,
                        param.name
                    );
                }
            }
        }
    }
}
