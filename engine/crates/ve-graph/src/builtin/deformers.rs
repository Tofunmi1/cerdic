//! Bounded vertex-space solid deformers.
use super::{single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

/// What a deformer takes: a solid (refined and checked as a solid) or a polygon mesh (every vertex moved as it is).
const XFORMABLE: &[PortType] = &[PortType::Solid, PortType::Poly];

fn tolerance() -> ParamSpec {
    ParamSpec {
        name: "tolerance",
        ty: ParamType::Num {
            unit: Unit::Length,
            min: 0.0005,
            max: 0.1,
        },
        default: Some(Value::Num(0.01)),
        doc: "Maximum sampled edge chord deviation in metres; refinement is bounded to two levels.",
    }
}

fn shift(name: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Length,
            min: -100.0,
            max: 100.0,
        },
        default: Some(Value::Num(0.0)),
        doc,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the deformer operator catalogue together."
)]
pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let same = OutputSpec::SameAsInput("in");
    vec![
        spec(
            "xform.twist@1",
            "Twists a solid around its local Y extent.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                ParamSpec {
                    name: "angle",
                    ty: ParamType::Num {
                        unit: Unit::Angle,
                        min: -std::f64::consts::TAU,
                        max: std::f64::consts::TAU,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Total twist from the bottom to top of the solid in radians.",
                },
                tolerance(),
            ],
        ),
        spec(
            "xform.taper@1",
            "Tapers a solid along its local Y extent.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                ParamSpec {
                    name: "top_scale",
                    ty: ParamType::Num {
                        unit: Unit::Ratio,
                        min: 0.1,
                        max: 4.0,
                    },
                    default: Some(Value::Num(1.0)),
                    doc: "X/Z scale at the top relative to the bottom.",
                },
                tolerance(),
            ],
        ),
        spec(
            "xform.bend@1",
            "Bends a solid around the Z axis along its local Y extent.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                ParamSpec {
                    name: "angle",
                    ty: ParamType::Num {
                        unit: Unit::Angle,
                        min: -std::f64::consts::FRAC_PI_2,
                        max: std::f64::consts::FRAC_PI_2,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Total bend angle from the bottom to top in radians.",
                },
                tolerance(),
            ],
        ),
        spec(
            "xform.shear@1",
            "Slants a solid: the top moves sideways by x_shift and z_shift (metres) relative to the bottom, in proportion to height, like a leaning tower or a parallelogram.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                shift(
                    "x_shift",
                    "How far the top moves along X relative to the bottom.",
                ),
                shift(
                    "z_shift",
                    "How far the top moves along Z relative to the bottom.",
                ),
                tolerance(),
            ],
        ),
        spec(
            "xform.wave@1",
            "Ripples a solid like a flag or a wavy wall: points move sideways by a sine wave that runs up the solid's height (Y). Subdivide the shape (or let the tolerance refine it) so there are vertices to ripple.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                ParamSpec {
                    name: "amplitude",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: -100.0,
                        max: 100.0,
                    },
                    default: Some(Value::Num(0.1)),
                    doc: "How far the crests move points sideways, in metres.",
                },
                ParamSpec {
                    name: "wavelength",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.01,
                        max: 1000.0,
                    },
                    default: Some(Value::Num(1.0)),
                    doc: "Height of one full wave in metres.",
                },
                ParamSpec {
                    name: "phase",
                    ty: ParamType::Num {
                        unit: Unit::Angle,
                        min: -std::f64::consts::TAU,
                        max: std::f64::consts::TAU,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Where in the wave the bottom starts, in radians.",
                },
                ParamSpec {
                    name: "direction",
                    ty: ParamType::Choice {
                        options: &["x", "z"],
                    },
                    default: Some(Value::Str("x".to_owned())),
                    doc: "Which way points move: x or z.",
                },
                tolerance(),
            ],
        ),
        spec(
            "xform.noise@1",
            "Roughens a solid with smooth random displacement (rocks, hills, lumpy clay): every point is pushed by smooth noise. The same seed always gives the same shape. Gentle settings only: a fold or a collapsed face is refused with a reason.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                ParamSpec {
                    name: "amplitude",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.0,
                        max: 100.0,
                    },
                    default: Some(Value::Num(0.05)),
                    doc: "Largest push in any direction, in metres. Keep it well below the feature scale.",
                },
                ParamSpec {
                    name: "scale",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.01,
                        max: 1000.0,
                    },
                    default: Some(Value::Num(0.5)),
                    doc: "Size of one lump in metres.",
                },
                ParamSpec {
                    name: "seed",
                    ty: ParamType::Int {
                        min: 0,
                        max: 1_000_000,
                    },
                    default: Some(Value::Int(0)),
                    doc: "Picks a different random shape.",
                },
                tolerance(),
            ],
        ),
        spec(
            "xform.lattice@1",
            "Free-form deformation: a grid of control points over the solid's bounding box, some of which you move; the solid follows smoothly (a cartoon squash, a bulge, a tapered bottle). Control point (i j k) runs 0 to divisions-1 along X, Y, Z; (0 0 0) is the minimum corner.",
            vec![single("in", XFORMABLE)],
            same,
            vec![
                ParamSpec {
                    name: "divisions",
                    ty: ParamType::Int { min: 2, max: 4 },
                    default: Some(Value::Int(3)),
                    doc: "Control points along each axis (2 to 4). 2 is a plain box of corners; 3 or 4 gives room to bulge.",
                },
                ParamSpec {
                    name: "offsets",
                    ty: ParamType::Str { max_len: 2048 },
                    default: Some(Value::Str(String::new())),
                    doc: "Control points to move, separated by semicolons (at most 64): \"i j k dx dy dz\" in metres. Example, bulge a 3-point lattice's middle outward: \"1 1 2 0 0 0.2; 1 1 0 0 0 -0.2\".",
                },
                tolerance(),
            ],
        ),
    ]
}
