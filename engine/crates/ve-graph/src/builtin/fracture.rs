//! Fracture: a shape broken into Voronoi pieces.

use super::{single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const SHAPE: &[PortType] = &[PortType::Solid, PortType::Poly];

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "poly.fracture@1",
        "Breaks a closed shape into pieces along a random Voronoi pattern, the way a wall, a vase or a boulder shatters. Each node makes ONE piece, number `piece` of `pieces`; write a loop over piece = 0 to pieces - 1 to make them all, each with its own output, then give each a physics body (phys.body, shape convex) to let them fall, or move them apart with xform.move to show the break. The pieces tile the shape exactly except for cracks `gap` wide, each a closed smooth-edged solid in the shape's own place (so they fit together at rest). The same `seed` always breaks the shape the same way; another seed breaks it differently. The shape must be closed (a solid, or a polygon mesh with no open edges). `voxel_size` is how finely the shape is sampled: a piece smaller than about two voxels vanishes, so keep voxels well under the piece size. A piece that is empty (a seed in a thin spot) is nothing and its output is skipped. Spike: random cells only, no interior crack faces beyond the cracks themselves.",
        vec![single("in", SHAPE)],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "pieces",
                ty: ParamType::Int { min: 2, max: 64 },
                default: Some(Value::Int(8)),
                doc: "How many pieces the shape breaks into (2 to 64).",
            },
            ParamSpec {
                name: "piece",
                ty: ParamType::Int { min: 0, max: 63 },
                default: Some(Value::Int(0)),
                doc: "Which piece this node makes, 0 to pieces - 1.",
            },
            ParamSpec {
                name: "seed",
                ty: ParamType::Int {
                    min: 0,
                    max: 1_000_000,
                },
                default: Some(Value::Int(1)),
                doc: "Pattern seed: the same seed breaks the same way.",
            },
            ParamSpec {
                name: "voxel_size",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.005,
                    max: 0.5,
                },
                default: Some(Value::Num(0.04)),
                doc: "How finely the shape is sampled, in metres; keep it well under the piece size.",
            },
            ParamSpec {
                name: "gap",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 0.2,
                },
                default: Some(Value::Num(0.01)),
                doc: "Width of the cracks between pieces, in metres (0 leaves them touching).",
            },
        ],
    )]
}
