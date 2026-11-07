//! Spatial implicit field values and their explicit solid conversion.
use super::{EXTENT, single, spec, vec3};
use crate::spec::{Arity, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const FIELD: &[PortType] = &[PortType::Field];
const MAX_TUBE_POINTS: usize = 128;
const MAX_REPEAT_COUNT: f64 = 16.0;

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut operators = primitives();
    operators.extend(analytic_primitives());
    operators.extend(combiners());
    operators.extend(transforms());
    operators.extend(repeats());
    operators.push(noise());
    operators.extend(warps());
    operators.push(to_solid());
    operators
}

fn warps() -> Vec<Option<OperatorSpec>> {
    ["taper", "twist", "bend"]
        .into_iter()
        .map(|name| {
            let mut params = vec![
                ParamSpec {
                    name: "axis",
                    ty: ParamType::Choice {
                        options: &["x", "y", "z"],
                    },
                    default: Some(Value::Str("y".into())),
                    doc: "Axis the warp follows.",
                },
                ParamSpec {
                    name: "from",
                    ty: ParamType::Num { unit: Unit::Length, min: -EXTENT, max: EXTENT },
                    default: Some(Value::Num(0.0)),
                    doc: "Where along the axis the warp starts, in metres; before it the warp is held at its starting value. A bend keeps this point fixed.",
                },
                ParamSpec {
                    name: "to",
                    ty: ParamType::Num { unit: Unit::Length, min: -EXTENT, max: EXTENT },
                    default: Some(Value::Num(1.0)),
                    doc: "Where along the axis the warp ends, in metres; it must differ from `from` (less than `from` works the other way round); after it the warp is held at its end value.",
                },
            ];
            if name == "taper" {
                params.extend([
                    ParamSpec {
                        name: "scale_start",
                        ty: ParamType::Num {
                            unit: Unit::None,
                            min: 0.01,
                            max: 100.0,
                        },
                        default: Some(Value::Num(1.0)),
                        doc: "Cross-section scale at from.",
                    },
                    ParamSpec {
                        name: "scale_end",
                        ty: ParamType::Num {
                            unit: Unit::None,
                            min: 0.01,
                            max: 100.0,
                        },
                        default: Some(Value::Num(1.0)),
                        doc: "Cross-section scale at to.",
                    },
                ]);
            } else {
                params.push(ParamSpec {
                    name: "angle",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: if name == "bend" { -360.0 } else { -3600.0 },
                        max: if name == "bend" { 360.0 } else { 3600.0 },
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Total rotation in degrees.",
                });
            }
            if name == "bend" {
                params.push(ParamSpec {
                    name: "toward",
                    ty: ParamType::Choice {
                        options: &["auto", "x", "y", "z"],
                    },
                    default: Some(Value::Str("auto".into())),
                    doc: "The axis the bar curves toward (x, y or z, not the bend axis itself); auto picks the next one cyclically: x to y, y to z, z to x. A positive angle curves toward its positive side.",
                });
            }
            let family = match name {
                "taper" => "field.taper@1",
                "twist" => "field.twist@1",
                _ => "field.bend@1",
            };
            let help = match name {
                "taper" => "Tapers a field along an axis: the cross-section is scaled linearly from `scale_start` at `from` to `scale_end` at `to` (held beyond): a nose, a horn, a column that narrows. Example: field.taper@1(f, axis=\"y\", from=-0.5, to=0.5, scale_start=1, scale_end=0.5). Not an exact distance field.",
                "twist" => "Twists a field about an axis: the cross-section turns by 0 degrees at `from` up to `angle` degrees at `to` (held beyond), keeping the volume: a tusk, a shell, a rope. Example: field.twist@1(f, axis=\"y\", from=-0.5, to=0.5, angle=90). Not an exact distance field.",
                _ => "Bends a field: the axis curves into a circular arc toward `toward` through `angle` degrees between `from` and `to` (the point at `from` stays put; beyond `to` the axis continues straight along the end tangent): a tail, a horn, an arm. Example: field.bend@1(f, axis=\"y\", toward=\"x\", from=-0.5, to=0.5, angle=90). A negative angle curves the other way. Not an exact distance field.",
            };
            spec(
                family,
                help,
                vec![single("field", FIELD)],
                OutputSpec::Fixed(PortType::Field),
                params,
            )
        })
        .collect()
}

fn noise() -> Option<OperatorSpec> {
    spec(
        "field.noise@1",
        "Displaces a field surface by bounded deterministic smooth or ridged noise.",
        vec![single("field", FIELD)],
        OutputSpec::Fixed(PortType::Field),
        vec![
            ParamSpec {
                name: "amplitude",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: EXTENT,
                },
                default: Some(Value::Num(0.02)),
                doc: "Maximum displacement in metres.",
            },
            ParamSpec {
                name: "scale",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.001,
                    max: EXTENT,
                },
                default: Some(Value::Num(0.25)),
                doc: "First octave wavelength in metres.",
            },
            ParamSpec {
                name: "octaves",
                ty: ParamType::Int { min: 1, max: 6 },
                default: Some(Value::Int(1)),
                doc: "Number of noise octaves.",
            },
            ParamSpec {
                name: "persistence",
                ty: ParamType::Num {
                    unit: Unit::None,
                    min: 0.0,
                    max: 1.0,
                },
                default: Some(Value::Num(0.5)),
                doc: "Weight ratio between octaves.",
            },
            ParamSpec {
                name: "seed",
                ty: ParamType::Int {
                    min: 0,
                    max: i64::from(u32::MAX),
                },
                default: Some(Value::Int(0)),
                doc: "Deterministic integer noise seed.",
            },
            ParamSpec {
                name: "mode",
                ty: ParamType::Choice {
                    options: &["smooth", "ridged"],
                },
                default: Some(Value::Str("smooth".to_owned())),
                doc: "Smooth noise or creased ridged noise.",
            },
        ],
    )
}

fn primitives() -> Vec<Option<OperatorSpec>> {
    let output = OutputSpec::Fixed(PortType::Field);
    vec![
        spec(
            "field.sphere@1",
            "Exact sphere field centered at the origin.",
            vec![],
            output,
            vec![super::length("radius", "Sphere radius in metres.")],
        ),
        spec(
            "field.box@1",
            "Exact rounded box field centered at the origin.",
            vec![],
            output,
            vec![
                vec3(
                    "half",
                    Unit::Length,
                    1.0e-6,
                    EXTENT,
                    "Half extents in metres.",
                ),
                ParamSpec {
                    name: "rounding",
                    ty: ParamType::Num { unit: Unit::Length, min: 0.0, max: EXTENT },
                    default: Some(Value::Num(0.0)),
                    doc: "Corner radius in metres; 0 gives sharp corners.",
                },
            ],
        ),
        spec(
            "field.torus@1",
            "Exact torus field around the Y axis.",
            vec![],
            output,
            vec![
                super::length("major", "Major radius in metres."),
                super::length("tube", "Tube radius in metres."),
            ],
        ),
        spec(
            "field.capsule@1",
            "Exact capsule field along the Y axis.",
            vec![],
            output,
            vec![
                super::length("radius", "Capsule radius in metres."),
                super::length("half_length", "Half-length of its central segment."),
            ],
        ),
        spec(
            "field.cylinder@1",
            "Exact finite cylinder field along the Y axis.",
            vec![],
            output,
            vec![
                super::length("radius", "Cylinder radius in metres."),
                super::length("half_height", "Cylinder half-height in metres."),
            ],
        ),
        tube(),
    ]
}

fn tube() -> Option<OperatorSpec> {
    spec(
        "field.tube@1",
        "Tube around a bounded ordered polyline: constant radius, or tapering from `radius` at the first point to `end_radius` at the last (a limb, tail or horn).",
        vec![],
        OutputSpec::Fixed(PortType::Field),
        vec![
            ParamSpec {
                name: "points",
                ty: ParamType::Points3 {
                    min_points: 2,
                    max_points: MAX_TUBE_POINTS,
                    min: -EXTENT,
                    max: EXTENT,
                },
                default: None,
                doc: "Ordered tube centerline points [x, y, z] in metres.",
            },
            super::length(
                "radius",
                "Tube radius in metres (at the first point when tapering).",
            ),
            ParamSpec {
                name: "end_radius",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: EXTENT,
                },
                default: Some(Value::Num(0.0)),
                doc: "Radius at the last point; the radius changes linearly with distance along the polyline. 0 keeps the radius constant.",
            },
        ],
    )
}

fn analytic_primitives() -> Vec<Option<OperatorSpec>> {
    let output = OutputSpec::Fixed(PortType::Field);
    vec![
        spec(
            "field.ellipsoid@1",
            "Axis-aligned ellipsoid; unequal radii use a conservative distance lower bound.",
            vec![],
            output,
            vec![vec3(
                "radii",
                Unit::Length,
                1.0e-6,
                EXTENT,
                "Positive XYZ radii in metres.",
            )],
        ),
        spec(
            "field.plane@1",
            "Normalized half-space plane; bounded meshing requires a closed field composition.",
            vec![],
            output,
            vec![
                vec3(
                    "normal",
                    Unit::None,
                    -EXTENT,
                    EXTENT,
                    "Nonzero plane normal.",
                ),
                ParamSpec {
                    name: "offset",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: -EXTENT,
                        max: EXTENT,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Signed plane offset in metres.",
                },
            ],
        ),
    ]
}

fn combiners() -> Vec<Option<OperatorSpec>> {
    let output = OutputSpec::Fixed(PortType::Field);
    let pair = || vec![single("a", FIELD), single("b", FIELD)];
    let variadic = vec![PortSpec {
        name: "fields",
        accepts: FIELD,
        arity: Arity::Variadic { min: 2 },
    }];
    vec![
        spec(
            "field.union@1",
            "Hard union of two or more spatial fields.",
            variadic,
            output,
            vec![],
        ),
        spec(
            "field.smooth_union@1",
            "Polynomial smooth union; its distance guarantee is None.",
            pair(),
            output,
            vec![super::length("radius", "Blend radius in metres.")],
        ),
        spec(
            "field.intersect@1",
            "Hard intersection of two fields.",
            pair(),
            output,
            vec![],
        ),
        spec(
            "field.subtract@1",
            "Subtracts field B from field A.",
            pair(),
            output,
            vec![],
        ),
    ]
}

fn transforms() -> Vec<Option<OperatorSpec>> {
    let output = OutputSpec::Fixed(PortType::Field);
    let input = || vec![single("field", FIELD)];
    vec![
        spec(
            "field.move@1",
            "Translates a field by an XYZ offset.",
            input(),
            output,
            vec![vec3(
                "by",
                Unit::Length,
                -EXTENT,
                EXTENT,
                "Translation in metres.",
            )],
        ),
        spec(
            "field.scale@1",
            "Uniformly scales a field around the origin.",
            input(),
            output,
            vec![ParamSpec {
                name: "factor",
                ty: ParamType::Num {
                    unit: Unit::Ratio,
                    min: 1.0e-6,
                    max: 1.0e6,
                },
                default: Some(Value::Num(1.0)),
                doc: "Uniform scale factor.",
            }],
        ),
        spec(
            "field.mirror@1",
            "Mirrors a field across selected origin planes.",
            input(),
            output,
            vec![vec3(
                "axes",
                Unit::None,
                0.0,
                1.0,
                "XYZ mirror switches as 0 or 1.",
            )],
        ),
        spec(
            "field.offset@1",
            "Offsets the zero surface along its field value.",
            input(),
            output,
            vec![ParamSpec {
                name: "amount",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: -EXTENT,
                    max: EXTENT,
                },
                default: Some(Value::Num(0.0)),
                doc: "Signed offset in metres.",
            }],
        ),
        spec(
            "field.shell@1",
            "Creates a symmetric shell around the zero surface.",
            input(),
            output,
            vec![super::length("half_width", "Shell half-width in metres.")],
        ),
    ]
}

fn repeats() -> Vec<Option<OperatorSpec>> {
    let output = OutputSpec::Fixed(PortType::Field);
    vec![spec(
        "field.repeat@1",
        "Boundedly repeats a field around the origin with folded coordinates.",
        vec![single("field", FIELD)],
        output,
        vec![
            vec3(
                "period",
                Unit::Length,
                1.0e-6,
                EXTENT,
                "Positive XYZ cell periods in metres.",
            ),
            vec3(
                "count",
                Unit::None,
                0.0,
                MAX_REPEAT_COUNT,
                "Integer copy counts per axis, from 0 to 16 on each side.",
            ),
        ],
    )]
}

fn to_solid() -> Option<OperatorSpec> {
    spec(
        "field.to_solid@1",
        "Meshes a field over explicit bounds and edge length.",
        vec![single("field", FIELD)],
        OutputSpec::Fixed(PortType::Solid),
        vec![
            vec3(
                "min",
                Unit::Length,
                -EXTENT,
                EXTENT,
                "Minimum XYZ bounds in metres.",
            ),
            vec3(
                "max",
                Unit::Length,
                -EXTENT,
                EXTENT,
                "Maximum XYZ bounds in metres.",
            ),
            ParamSpec {
                name: "edge_length",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 1.0e-4,
                    max: 1.0e3,
                },
                default: Some(Value::Num(0.05)),
                doc: "Grid spacing in metres; smaller values use more memory and time.",
            },
        ],
    )
}
