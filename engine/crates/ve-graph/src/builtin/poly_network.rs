//! Polygon edits built on Blender's bmesh: operators that take any selection, not just simple ones.
//! Bevel, inset, solidify, extrude, dissolve, poke, subdivide, wireframe and symmetrize.

use super::{EXTENT, single, spec};
use crate::spec::{Arity, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType};
use crate::value::{ParamType, Unit, Value};

pub(super) const POLY: &[PortType] = &[PortType::Poly];
const SELECTION: &[PortType] = &[PortType::Selection];

pub(super) fn optional_selection() -> PortSpec {
    PortSpec {
        name: "selection",
        accepts: SELECTION,
        arity: Arity::Optional,
    }
}

pub(super) fn number(
    name: &'static str,
    unit: Unit,
    range: (f64, f64),
    default: Option<f64>,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit,
            min: range.0,
            max: range.1,
        },
        default: default.map(Value::Num),
        doc,
    }
}

pub(super) fn length(name: &'static str, default: Option<f64>, doc: &'static str) -> ParamSpec {
    number(name, Unit::Length, (1.0e-6, EXTENT), default, doc)
}

fn signed_length(name: &'static str, default: f64, doc: &'static str) -> ParamSpec {
    number(name, Unit::Length, (-EXTENT, EXTENT), Some(default), doc)
}

pub(super) fn flag(name: &'static str, default: bool, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Bool,
        default: Some(Value::Bool(default)),
        doc,
    }
}

pub(super) fn choice(
    name: &'static str,
    options: &'static [&'static str],
    default: &'static str,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Choice { options },
        default: Some(Value::Str(default.into())),
        doc,
    }
}

fn faces_in(
    id: &str,
    doc: &'static str,
    selected: bool,
    params: Vec<ParamSpec>,
) -> Option<OperatorSpec> {
    let selection = if selected {
        single("selection", SELECTION)
    } else {
        optional_selection()
    };
    spec(
        id,
        doc,
        vec![single("in", POLY), selection],
        OutputSpec::Fixed(PortType::Poly),
        params,
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut all = vec![bevel()];
    all.extend(face_tools());
    all.extend(super::poly_network_whole::whole_mesh_tools());
    all
}

fn bevel() -> Option<OperatorSpec> {
    spec(
        "poly.bevel_network@1",
        "Rounds or chamfers selected edges of any layout: single edges, loops around a box, edges meeting at a corner, a whole cube. \
         Select edges with poly.select (element=edge; by=sharp picks the hard edges). Every selected edge must join two faces. \
         Faces, points and UV corners keep their attributes. Prefer this over poly.bevel_edges unless the edges are far apart.",
        vec![single("in", POLY), single("selection", SELECTION)],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            length(
                "width",
                None,
                "Bevel size in metres (a percentage, 0 to 100, when offset=percent). It is clamped so the bevel never overlaps itself.",
            ),
            ParamSpec {
                name: "segments",
                ty: ParamType::Int { min: 1, max: 16 },
                default: Some(Value::Int(3)),
                doc: "Segments across the bevel: 1 is a flat chamfer, more round it.",
            },
            number(
                "profile",
                Unit::None,
                (0.1, 1.0),
                Some(0.5),
                "Profile shape: 0.5 is a circular arc, lower is flatter and sharper, 1 is full.",
            ),
            choice(
                "offset",
                &["offset", "width", "depth", "percent"],
                "offset",
                "What width measures: offset (how far each new edge moves from the original along the faces), width (the bevel face's width), depth (how deep the bevel cuts) or percent (of the adjacent edges' length).",
            ),
        ],
    )
}

/// Operators that work on selected faces (inset, extrude, solidify, poke, wireframe) or edges (subdivide).
fn face_tools() -> Vec<Option<OperatorSpec>> {
    vec![
        faces_in(
            "poly.inset_region@1",
            "Insets the selected faces: shrinks the region inward by a distance, leaving a ring of faces, and can raise or sink the inner face. \
             Works on any polygon, not only planar convex ones. Select faces with poly.select.",
            true,
            vec![
                length("thickness", None, "Width of the ring, in metres."),
                signed_length(
                    "depth",
                    0.0,
                    "How far the inner face is raised along its normal (negative sinks it), in metres.",
                ),
                flag(
                    "even",
                    true,
                    "Scale the offset so the ring has even thickness at corners.",
                ),
                flag(
                    "relative",
                    false,
                    "Scale the offset by the surrounding geometry.",
                ),
                flag("boundary", true, "Also inset the region's outer boundary."),
                flag(
                    "outset",
                    false,
                    "Grow the region outward instead of shrinking it inward.",
                ),
            ],
        ),
        faces_in(
            "poly.extrude_region@1",
            "Extrudes the selected faces and moves the new faces by an offset, leaving walls around them. Select faces with poly.select.",
            true,
            vec![ParamSpec {
                name: "offset",
                ty: ParamType::Vec3 {
                    unit: Unit::Length,
                    min: -EXTENT,
                    max: EXTENT,
                },
                default: None,
                doc: "The vector the new faces move by, in metres.",
            }],
        ),
        faces_in(
            "poly.solidify@1",
            "Gives a surface thickness: adds a second surface offset along the normals and joins the two at open edges. \
             With no selection every face is thickened. Good for walls, shells and leaves.",
            false,
            vec![signed_length(
                "thickness",
                0.05,
                "Distance along each face's normal in metres, not zero (negative goes inward).",
            )],
        ),
        faces_in(
            "poly.poke@1",
            "Adds a vertex in the middle of each selected face (all faces when none are selected) and fans triangles to it, raising a pyramid.",
            false,
            vec![
                signed_length(
                    "offset",
                    0.0,
                    "How far the new vertex is raised along the normal, in metres.",
                ),
                choice(
                    "center",
                    &["median", "weighted", "bounds"],
                    "median",
                    "Where the new vertex sits: the mean of the corners, the angle-weighted mean, or the centre of the bounding box.",
                ),
                flag("relative", false, "Scale the offset by the face's size."),
            ],
        ),
        faces_in(
            "poly.wireframe@1",
            "Turns faces (all when none are selected) into a lattice of struts of a given thickness, like a cage or grille.",
            false,
            vec![
                length("thickness", None, "Strut thickness in metres."),
                signed_length(
                    "offset",
                    0.0,
                    "Offset of the struts along the normal, in metres.",
                ),
                flag(
                    "even",
                    true,
                    "Scale thickness so struts keep an even width at corners.",
                ),
                flag(
                    "relative",
                    false,
                    "Scale thickness by the surrounding geometry.",
                ),
                flag("boundary", true, "Also make struts along open boundaries."),
                flag("replace", true, "Remove the original faces."),
            ],
        ),
        spec(
            "poly.subdivide_edges@1",
            "Cuts the selected edges into pieces and splits the faces around them: adds loops and detail where you ask (select edges with poly.select element=edge).",
            vec![single("in", POLY), single("selection", SELECTION)],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                ParamSpec {
                    name: "cuts",
                    ty: ParamType::Int { min: 1, max: 8 },
                    default: Some(Value::Int(1)),
                    doc: "Cuts per edge.",
                },
                number(
                    "smooth",
                    Unit::None,
                    (0.0, 1.0),
                    Some(0.0),
                    "How much the new points are pushed to smooth the surface.",
                ),
                flag("grid_fill", true, "Fill quads with a grid of quads."),
            ],
        ),
    ]
}
