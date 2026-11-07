//! Polygon meshes: sources and conversions, element selection, and the face, edge and loop edits
//! (flip, weld, dissolve, delete, extrude, inset, bridge, bevel).

use super::{EXTENT, SOLID, single, spec};
use crate::spec::{Arity, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const POLY: &[PortType] = &[PortType::Poly];
const SELECTION: &[PortType] = &[PortType::Selection];

fn length(name: &'static str, default: Option<f64>, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Length,
            min: 1.0e-6,
            max: EXTENT,
        },
        default: default.map(Value::Num),
        doc,
    }
}

fn flag(name: &'static str, default: bool, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Bool,
        default: Some(Value::Bool(default)),
        doc,
    }
}

fn count(name: &'static str, max: i64, default: i64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Int { min: 1, max },
        default: Some(Value::Int(default)),
        doc,
    }
}

fn sharpness(name: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::None,
            min: 0.0,
            max: 10.0,
        },
        default: Some(Value::Num(10.0)),
        doc,
    }
}

fn optional(name: &'static str, accepts: &'static [PortType]) -> PortSpec {
    PortSpec {
        name,
        accepts,
        arity: Arity::Optional,
    }
}

fn edit(
    id: &str,
    doc: &'static str,
    selected: bool,
    params: Vec<ParamSpec>,
) -> Option<OperatorSpec> {
    let mut inputs = vec![single("in", POLY)];
    if selected {
        inputs.push(single("selection", SELECTION));
    }
    spec(id, doc, inputs, OutputSpec::Fixed(PortType::Poly), params)
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut all = sources();
    all.push(super::poly_select::select());
    all.extend(edits());
    all.extend(shape_edits());
    all
}

fn sources() -> Vec<Option<OperatorSpec>> {
    let poly = OutputSpec::Fixed(PortType::Poly);
    vec![
        spec(
            "poly.grid@1",
            "A flat grid of quads in the z = 0 plane with normalized u/v corner UVs, width wide centred on x = 0 and height tall centred on y = 0 (so it spans -height/2 to height/2: move it up to stand it on the floor), facing +z.",
            vec![],
            poly,
            vec![
                length("width", None, "Total width in metres."),
                length("height", None, "Total height in metres."),
                count("columns", 512, 4, "Quads across."),
                count("rows", 512, 4, "Quads up."),
            ],
        ),
        spec(
            "poly.import@1",
            "Brings your own mesh file into the document as an editable polygon mesh: the file's text goes in `data` and `format` says how to read it (obj, ascii ply or ascii stl). Nothing is fetched and nothing is bundled with the engine; the document carries the text, so the result is the same everywhere. OBJ keeps its polygons (triangles, quads and larger) and its texture coordinates as the mesh's u and v; PLY keeps its faces; STL is triangles welded by exact position. Normals, materials, groups and colours in the file are ignored (give the part a look with appearance nodes). `scale` multiplies every coordinate (use 0.001 for a file in millimetres, 0.01 for centimetres) and `up` says which axis the file calls up: a file made in Z-up software (`z`) is turned to this engine's Y up. Binary PLY and binary STL are refused with the way out (save them as ASCII); up to 200,000 vertices and faces and 1,000,000 characters of text. The result is a normal poly mesh: it can be selected, extruded, smoothed, unwrapped, decimated, put into sets or turned into a solid, and it is checked like any other (an open or non-manifold file imports fine as a poly; poly.to_solid asks for a closed one).",
            vec![],
            poly,
            vec![
                ParamSpec {
                    name: "data",
                    ty: ParamType::Str { max_len: 1_000_000 },
                    default: None,
                    doc: "The file's text.",
                },
                ParamSpec {
                    name: "format",
                    ty: ParamType::Choice {
                        options: &["obj", "ply", "stl"],
                    },
                    default: Some(Value::Str("obj".to_owned())),
                    doc: "How to read `data`: Wavefront obj, ascii ply or ascii stl.",
                },
                ParamSpec {
                    name: "scale",
                    ty: ParamType::Num {
                        unit: Unit::Ratio,
                        min: 1.0e-6,
                        max: 1.0e6,
                    },
                    default: Some(Value::Num(1.0)),
                    doc: "Multiplies every coordinate; the file's unit times this is metres.",
                },
                ParamSpec {
                    name: "up",
                    ty: ParamType::Choice {
                        options: &["y", "z"],
                    },
                    default: Some(Value::Str("y".to_owned())),
                    doc: "Which axis the file calls up; z is turned to Y up.",
                },
            ],
        ),
        spec(
            "poly.from_solid@1",
            "Turns a solid into an editable polygon mesh of its triangles. Merge coplanar triangles with poly.dissolve_faces.",
            vec![single("in", SOLID)],
            poly,
            vec![],
        ),
        spec(
            "poly.subdivide@1",
            "Final-step uniform Catmull-Clark subdivision of a polygon mesh. Preserve and edit the cage before subdivision: v1 polygon edits may refuse curved refined faces and bevel networks. Curved quads are triangulated on their shorter diagonal when converted to triangles.",
            vec![
                single("in", POLY),
                optional("creases", SELECTION),
                optional("corners", SELECTION),
            ],
            poly,
            vec![
                ParamSpec {
                    name: "levels",
                    ty: ParamType::Int { min: 1, max: 4 },
                    default: Some(Value::Int(1)),
                    doc: "Uniform Catmull-Clark refinement levels (1–4).",
                },
                sharpness(
                    "sharpness",
                    "Sharpness applied to selected crease edges (0 smooth, 10 fully sharp).",
                ),
                sharpness(
                    "corner_sharpness",
                    "Sharpness applied to selected corner vertices (0 smooth, 10 fully sharp).",
                ),
                ParamSpec {
                    name: "boundary",
                    ty: ParamType::Choice {
                        options: &["edges", "edges_and_corners"],
                    },
                    default: Some(Value::Str("edges_and_corners".into())),
                    doc: "Boundary interpolation rule for open meshes.",
                },
            ],
        ),
        spec(
            "poly.to_solid@1",
            "Turns a closed polygon mesh back into a solid (faces are triangulated properly, never fanned). Refuses an open or non-manifold mesh.",
            vec![single("in", POLY)],
            OutputSpec::Fixed(PortType::Solid),
            vec![],
        ),
    ]
}

fn edits() -> Vec<Option<OperatorSpec>> {
    vec![
        edit(
            "poly.flip_faces@1",
            "Reverses the winding of selected faces. Whole connected components always work; a partial flip may not add inconsistent edges unless allow_inconsistent is true.",
            true,
            vec![flag(
                "allow_inconsistent",
                false,
                "Allow a partial flip that leaves winding conflicts.",
            )],
        ),
        edit(
            "poly.weld@1",
            "Merges vertices closer than a distance. Refuses when a face would collapse.",
            false,
            vec![length(
                "distance",
                Some(0.0001),
                "Merge distance in metres.",
            )],
        ),
        edit(
            "poly.merge_coplanar@1",
            "Merges connected coplanar faces into single faces, such as a solid's triangles into its flat faces. A patch that cannot become one simple face (the wall face around a window, a pinch) stays as it is by default, so a cut wall can then be rounded with poly.round_edges.",
            false,
            vec![
                ParamSpec {
                    name: "angle",
                    ty: ParamType::Num {
                        unit: Unit::Angle,
                        min: 0.0,
                        max: 0.5,
                    },
                    default: Some(Value::Num(0.01)),
                    doc: "How far a face's normal may differ from the patch's first face and still count as coplanar, in radians.",
                },
                ParamSpec {
                    name: "holes",
                    ty: ParamType::Choice {
                        options: &["keep", "refuse"],
                    },
                    default: Some(Value::Str("keep".into())),
                    doc: "keep leaves a patch that cannot become one simple face (a hole or pinch) unmerged; refuse fails with the reason instead.",
                },
            ],
        ),
        edit(
            "poly.dissolve_faces@1",
            "Merges a connected patch of selected faces into one face (select faces).",
            true,
            vec![],
        ),
        edit(
            "poly.dissolve_edge@1",
            "Removes one interior edge and merges its two faces (select one edge).",
            true,
            vec![],
        ),
        edit(
            "poly.dissolve_vertex@1",
            "Removes one vertex and merges its faces (select one vertex).",
            true,
            vec![],
        ),
        edit(
            "poly.delete_faces@1",
            "Removes selected faces and leaves an open boundary. Refuses to delete every face or isolate a vertex.",
            true,
            vec![],
        ),
    ]
}

fn shape_edits() -> Vec<Option<OperatorSpec>> {
    vec![
        edit(
            "poly.extrude_faces@1",
            "Pushes selected planar faces out by an offset and joins their border to the mesh with side faces. A connected patch moves as one; with individual each face moves on its own.",
            true,
            vec![
                ParamSpec {
                    name: "offset",
                    ty: ParamType::Vec3 {
                        unit: Unit::Length,
                        min: -EXTENT,
                        max: EXTENT,
                    },
                    default: None,
                    doc: "Where the faces move, in metres; it must point out of the face.",
                },
                flag(
                    "individual",
                    false,
                    "Extrude each selected face on its own.",
                ),
            ],
        ),
        edit(
            "poly.inset_faces@1",
            "Shrinks each selected planar convex face inward by a distance, leaving a ring of quads and an inner face.",
            true,
            vec![length(
                "distance",
                None,
                "How far the border moves in, in metres.",
            )],
        ),
        edit(
            "poly.bridge_loops@1",
            "Joins two boundary loops with the same number of vertices by a ring of quads (select two loops).",
            true,
            vec![],
        ),
        edit(
            "poly.bevel_edges@1",
            "Rounds or chamfers selected edges. Edges must be disjoint, between two planar faces, with three faces at each end.",
            true,
            vec![
                length(
                    "width",
                    None,
                    "How far the bevel reaches onto each face, in metres.",
                ),
                count(
                    "segments",
                    4,
                    1,
                    "Profile segments: 1 is a flat chamfer, more round it.",
                ),
            ],
        ),
    ]
}
