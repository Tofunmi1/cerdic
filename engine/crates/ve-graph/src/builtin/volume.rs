//! Volume grids: a closed mesh sampled into signed distances, combined, smoothed, grown or shrunk, and turned back into a mesh.

use super::{single, spec, variadic};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const MESH: &[PortType] = &[PortType::Solid, PortType::Poly];
const VOLUME: &[PortType] = &[PortType::Volume];

fn num(
    name: &'static str,
    unit: Unit,
    range: (f64, f64),
    default: f64,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit,
            min: range.0,
            max: range.1,
        },
        default: Some(Value::Num(default)),
        doc,
    }
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "volume.from_mesh@1",
            "Samples a closed mesh into a grid of distances to its surface (negative inside, positive outside), the starting point of the volume nodes: combine volumes like clay (volume.combine), blur them smooth (volume.smooth), grow or shrink them (volume.offset), and turn the result back into a clean all-quad mesh (volume.to_mesh). The mesh must be closed (a solid, or a polygon mesh with no open edges; poly.fill_holes closes one). The grid keeps exact distances only within `band` voxels of the surface, so offsets are limited to about band times voxel_size; a mesh with a sealed hollow inside is read as solid. Grids made with the same voxel_size line up and combine exactly. The grid is limited to 6,000,000 voxels.",
            vec![single("in", MESH)],
            OutputSpec::Fixed(PortType::Volume),
            vec![
                num(
                    "voxel_size",
                    Unit::Length,
                    (0.002, 10.0),
                    0.05,
                    "Edge of one voxel in metres: the finest detail the volume can hold and the size of the quads volume.to_mesh makes. Use the same value on volumes you will combine.",
                ),
                ParamSpec {
                    name: "band",
                    ty: ParamType::Int { min: 2, max: 24 },
                    default: Some(Value::Int(6)),
                    doc: "How many voxels out from the surface distances are exact. Offsets and blends reach at most this far.",
                },
            ],
        ),
        spec(
            "volume.combine@1",
            "Joins volumes: union (everything in any of them), subtract (the first minus all the others) or intersect (only what is in all of them), optionally with a smooth blend where the shapes meet, like clay run together. Every volume must have the same voxel_size. Unlike mesh booleans it cannot fail on a bad mesh, and the blend can be as soft as you like.",
            vec![variadic("in", VOLUME)],
            OutputSpec::Fixed(PortType::Volume),
            vec![
                ParamSpec {
                    name: "operation",
                    ty: ParamType::Choice {
                        options: &["union", "subtract", "intersect"],
                    },
                    default: Some(Value::Str("union".to_owned())),
                    doc: "union, subtract (first minus the rest) or intersect.",
                },
                num(
                    "blend",
                    Unit::Length,
                    (0.0, 100.0),
                    0.0,
                    "Metres over which the shapes melt into each other where they meet; 0 is a hard edge. Keep it within the volumes' band.",
                ),
            ],
        ),
        spec(
            "volume.smooth@1",
            "Smooths a volume: the surface relaxes like clay, sharp creases and small lumps round off and thin bridges thin out. Each pass moves every voxel's distance toward the average of its neighbours, so more passes round more and shrink thin parts a little.",
            vec![single("in", VOLUME)],
            OutputSpec::Fixed(PortType::Volume),
            vec![
                ParamSpec {
                    name: "iterations",
                    ty: ParamType::Int { min: 1, max: 50 },
                    default: Some(Value::Int(2)),
                    doc: "Smoothing passes.",
                },
                num(
                    "factor",
                    Unit::None,
                    (0.0, 1.0),
                    0.8,
                    "How far each pass moves toward the neighbours' average: 0 nothing, 1 all the way.",
                ),
            ],
        ),
        spec(
            "volume.offset@1",
            "Grows (positive distance) or shrinks (negative) a volume by moving its surface, in metres: a thicker or thinner shape, a coat around it, a hole made wider. Limited to about 0.9 of band times voxel_size.",
            vec![single("in", VOLUME)],
            OutputSpec::Fixed(PortType::Volume),
            vec![num(
                "distance",
                Unit::Length,
                (-100.0, 100.0),
                0.05,
                "How far the surface moves outward (negative: inward), in metres.",
            )],
        ),
        spec(
            "volume.to_mesh@1",
            "Turns a volume back into a mesh: the surface where the distance is zero, as a closed mesh of clean quads about one voxel across, facing outward (surface nets). Smoothed volumes give smooth meshes. Output the mesh, take it into poly.* nodes, or retopologise it with poly.remesh_quads. At most 500,000 quads.",
            vec![single("in", VOLUME)],
            OutputSpec::Fixed(PortType::Poly),
            vec![num(
                "iso",
                Unit::Length,
                (-10.0, 10.0),
                0.0,
                "Which level to mesh: 0 is the surface, positive a little outside it, negative a little inside.",
            )],
        ),
    ]
}
