//! Whole-mesh polygon operators: unwrap, dissolve, decimate, unsubdivide, symmetrize,
//! triangulate and remove doubles. Split out of `poly_network.rs` to keep each file small.

use super::poly_network::{POLY, choice, flag, length, number, optional_selection};
use super::{single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

/// Operators that take no selection (or only optional seams).
pub(super) fn whole_mesh_tools() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "poly.unwrap_uv@1",
            "Unwraps a polygon mesh to UV coordinates (corner attributes u and v) with Blender's LSCM or angle-based flattening and packs the islands into the unit square. \
             Select seam edges with poly.select element=edge (for example by=sharp on a box) to cut a closed shape into islands; an already open mesh needs none.",
            vec![single("in", POLY), optional_selection()],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                choice(
                    "method",
                    &["conformal", "angle"],
                    "conformal",
                    "conformal is fast and keeps angles; angle (ABF++) distorts angles less on curved shapes but is slower.",
                ),
                flag(
                    "pack",
                    true,
                    "Pack the islands into the unit square; otherwise they overlap near the origin.",
                ),
                flag(
                    "rotate",
                    false,
                    "Let packing rotate islands to fit tighter.",
                ),
                number(
                    "margin",
                    Unit::None,
                    (0.0, 0.5),
                    Some(0.02),
                    "Space between packed islands, as a fraction of the unit square.",
                ),
                flag(
                    "auto_seams",
                    true,
                    "When no seam edges are selected, cut automatically: the surface is split into charts whose faces stay within seam_angle of the chart's first face (a box gives six, a sphere several near-flat patches), plus paths that open closed or tube-shaped pieces. Selected seams are always used as given.",
                ),
                number(
                    "seam_angle",
                    Unit::Angle,
                    (0.0, std::f64::consts::PI),
                    Some(1.1),
                    "Width of a chart's cone when auto_seams is on (1.1 rad is about 63 degrees): smaller gives more, flatter charts and less stretch but more seams; larger gives fewer charts.",
                ),
            ],
        ),
        spec(
            "poly.dissolve_limit@1",
            "Merges neighbouring faces that are flatter than an angle: cleans triangulated or over-detailed flat areas back into big polygons.",
            vec![single("in", POLY)],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                number(
                    "angle",
                    Unit::Angle,
                    (0.0, std::f64::consts::PI),
                    Some(0.0873),
                    "Faces whose normals differ by less than this merge.",
                ),
                flag("boundaries", false, "Also dissolve along open boundaries."),
            ],
        ),
        spec(
            "poly.decimate@1",
            "Reduces the face count by collapsing the edges that change the shape least: level of detail (LOD) for far-away or cheap versions of a model. \
             ratio is the fraction of faces kept (0.25 keeps a quarter).",
            vec![single("in", POLY)],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                number(
                    "ratio",
                    Unit::None,
                    (0.01, 1.0),
                    None,
                    "Fraction of the faces to keep, for example 0.5 for half.",
                ),
                choice(
                    "symmetry",
                    &["none", "x", "y", "z"],
                    "none",
                    "Keep the result mirror-symmetric about this axis plane.",
                ),
                flag(
                    "triangulate",
                    true,
                    "Triangulate first (needed unless the mesh is already triangles).",
                ),
            ],
        ),
        spec(
            "poly.unsubdivide@1",
            "Reverses a subdivision: removes the vertices a regular grid of quads could be rebuilt without. Use it to get a coarse cage back from a smooth, dense mesh.",
            vec![single("in", POLY)],
            OutputSpec::Fixed(PortType::Poly),
            vec![ParamSpec {
                name: "iterations",
                ty: ParamType::Int { min: 1, max: 4 },
                default: Some(Value::Int(1)),
                doc: "How many levels to undo.",
            }],
        ),
        spec(
            "poly.symmetrize@1",
            "Keeps one side of an axis plane through the origin and mirrors it onto the other, so a half model becomes a symmetric whole.",
            vec![single("in", POLY)],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                choice(
                    "axis",
                    &["x", "y", "z"],
                    "x",
                    "The axis the mirror plane is perpendicular to.",
                ),
                choice(
                    "keep",
                    &["positive", "negative"],
                    "positive",
                    "Which side is kept and copied onto the other.",
                ),
                length(
                    "distance",
                    Some(1.0e-4),
                    "Vertices closer than this to the plane, or to their mirror, are merged.",
                ),
            ],
        ),
        spec(
            "poly.triangulate@1",
            "Splits selected polygons into triangles using Blender's beauty triangulation; with no selection, triangulates every face.",
            vec![single("in", POLY), optional_selection()],
            OutputSpec::Fixed(PortType::Poly),
            vec![],
        ),
        spec(
            "poly.remove_doubles@1",
            "Merges vertices within a distance in metres. With no selection, checks every vertex.",
            vec![single("in", POLY), optional_selection()],
            OutputSpec::Fixed(PortType::Poly),
            vec![length(
                "distance",
                None,
                "Maximum merge distance in metres (1e-6 to 1e3).",
            )],
        ),
    ]
}
