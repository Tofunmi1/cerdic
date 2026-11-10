//! Whole-mesh cleanup and shaping tools from Blender's bmesh operators: normals, smoothing, hole filling, triangle and quad
//! tidying, planarizing and degenerate removal. Each takes a polygon mesh and acts on all of it.

use super::poly_network::{POLY, choice, number};
use super::{single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn whole(id: &'static str, doc: &'static str, params: Vec<ParamSpec>) -> Option<OperatorSpec> {
    spec(
        id,
        doc,
        vec![single("in", POLY)],
        OutputSpec::Fixed(PortType::Poly),
        params,
    )
}

fn passes(name: &'static str, default: i64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Int { min: 1, max: 50 },
        default: Some(Value::Int(default)),
        doc,
    }
}

const SELECTION: &[PortType] = &[PortType::Selection];

fn with_selection(
    id: &'static str,
    doc: &'static str,
    params: Vec<ParamSpec>,
) -> Option<OperatorSpec> {
    spec(
        id,
        doc,
        vec![single("in", POLY), single("selection", SELECTION)],
        OutputSpec::Fixed(PortType::Poly),
        params,
    )
}

/// Tools that act on chosen edges or vertices (make the selection with poly.select).
fn selected() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "poly.uv_transform@1",
            "Moves, scales and turns UV coordinates: shift a layout, shrink it, rotate an island. With no selection it changes every UV; with a selection of faces (poly.select element=face, for example by=attribute to take one region) it changes only those faces' UVs, which is how one island is moved out of the way of another or scaled to a different texel density. The mesh needs UVs (poly.grid has them; otherwise run poly.unwrap_uv). Look at the result with poly.uv_layout.",
            vec![
                single("in", POLY),
                super::poly_network::optional_selection(),
            ],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                number(
                    "offset_u",
                    Unit::None,
                    (-100.0, 100.0),
                    Some(0.0),
                    "Shift along U, applied last (1 is a whole texture width).",
                ),
                number(
                    "offset_v",
                    Unit::None,
                    (-100.0, 100.0),
                    Some(0.0),
                    "Shift along V, applied last.",
                ),
                number(
                    "scale",
                    Unit::None,
                    (0.001, 1000.0),
                    Some(1.0),
                    "Scale about the pivot.",
                ),
                number(
                    "rotate",
                    Unit::Angle,
                    (-std::f64::consts::TAU, std::f64::consts::TAU),
                    Some(0.0),
                    "Turn about the pivot, counter-clockwise in UV space.",
                ),
                choice(
                    "pivot",
                    &["center", "origin"],
                    "center",
                    "What scale and rotate pivot about: the centre of the affected UVs, or the UV origin (0, 0).",
                ),
            ],
        ),
        whole(
            "poly.sculpt@1",
            "Sculpts a mesh with brush strokes, like clay: write the strokes as text, separated by semicolons, and they are applied in order. A stroke is `brush cx cy cz radius strength` (the point on or near the surface in metres, how far the brush reaches, how hard it works); the grab brush is `grab cx cy cz radius dx dy dz` (the vector to move by). Brushes: draw (push the surface along its average normal under the brush; negative strength digs), inflate (push each point along its own normal), smooth (relax toward the neighbours, strength 0 to 1), flatten (pull toward the average plane, 0 to 1), pinch (pull toward the centre, 0 to 1), grab (drag by a vector), mask (protect the reached vertices), and unmask (remove that protection). Masks persist as a `sculpt_mask` point attribute and all geometry brushes respect them. `falloff=surface` measures distance along connected mesh edges, so a stroke stays on one side of a thin surface; `falloff=sphere` uses ordinary 3D distance. Every brush fades smoothly to nothing at its radius, so strokes leave no step. Example, a nose on a sphere then a smoothed cheek: `draw 0 0.1 1 0.25 0.12; smooth 0.5 0 0.9 0.3 0.6`. It needs a mesh with enough vertices for the detail you want (a denser poly.grid, poly.subdivide_edges, poly.remesh_quads); at most 64 strokes and 500,000 vertices.",
            vec![
                ParamSpec {
                    name: "strokes",
                    ty: ParamType::Str { max_len: 4096 },
                    default: Some(Value::Str("draw 0 0 0 0.3 0.05".to_owned())),
                    doc: "The strokes, `brush cx cy cz radius strength;` separated by semicolons (grab takes a vector instead of a strength).",
                },
                ParamSpec {
                    name: "falloff",
                    ty: ParamType::Choice {
                        options: &["sphere", "surface"],
                    },
                    default: Some(Value::Str("sphere".to_owned())),
                    doc: "Measure brush distance through space (`sphere`) or along connected mesh edges (`surface`). Surface mode avoids sculpting through the back of a head, limb or thin shell; weld seams first if the brush should cross them.",
                },
                choice(
                    "symmetry",
                    &["none", "x", "y", "z"],
                    "none",
                    "Mirror every stroke about the plane through the origin that this axis is perpendicular to (x: left and right).",
                ),
            ],
        ),
        whole(
            "poly.uv_layout@1",
            "Turns the UV layout into a flat mesh you can look at: every face is drawn where its UVs put it on the XY plane (u to the right, v up), so islands show as separate patches, seams as gaps, overlaps as faces drawn on top of each other, stretching as faces that look nothing like their 3D shape. A face whose UVs run clockwise shows its back. Link it to an output and view it like any geometry; it does not change the original mesh.",
            vec![number(
                "size",
                Unit::Length,
                (0.001, 1000.0),
                Some(1.0),
                "Width of the whole 0 to 1 UV square in metres.",
            )],
        ),
        with_selection(
            "poly.split_edges@1",
            "Disconnects the faces along the selected edges: each side gets its own vertices, so the edge becomes a hard seam that shades sharp, can take a UV seam, and lets the two sides move apart (select edges with poly.select element=edge, for example by=sharp).",
            vec![],
        ),
        with_selection(
            "poly.rotate_edges@1",
            "Spins each selected edge to the other diagonal of the two faces it joins (triangle pairs become the other split, quad pairs are re-cut). Select edges with poly.select element=edge; each must join exactly two faces.",
            vec![choice(
                "direction",
                &["clockwise", "counter_clockwise"],
                "clockwise",
                "Which way the edge turns.",
            )],
        ),
        with_selection(
            "poly.connect_vertices@1",
            "Adds edges between the selected vertices of each face, splitting the face along them (select vertices with poly.select element=vertex; at least two, and two on the same face to make a cut). Cuts a quad across its diagonal or a polygon into pieces.",
            vec![ParamSpec {
                name: "check_degenerate",
                ty: ParamType::Bool,
                default: Some(Value::Bool(true)),
                doc: "Refuse cuts that would overlap or cross each other.",
            }],
        ),
        with_selection(
            "poly.space_loops@1",
            "Moves the vertices of the selected edge loops to even spacing along each loop's own curve, without changing its shape (select the loop's edges with poly.select element=edge, for example by=height on a tube). Evens out a ring after an uneven cut.",
            vec![
                number(
                    "factor",
                    Unit::None,
                    (0.0, 1.0),
                    Some(1.0),
                    "How far toward even spacing: 0 nothing, 1 fully even.",
                ),
                choice(
                    "interpolation",
                    &["cubic", "linear"],
                    "cubic",
                    "Follow the loop with a smooth curve or straight segments.",
                ),
            ],
        ),
        with_selection(
            "poly.relax_loops@1",
            "Smooths the selected edge loops so they flow in a gentler curve (select the loop's edges with poly.select element=edge). More iterations smooth more and can shrink a loop a little.",
            vec![
                passes("iterations", 3, "Passes, 1 to 50."),
                choice(
                    "interpolation",
                    &["cubic", "linear"],
                    "cubic",
                    "Smooth curve or straight segments.",
                ),
                ParamSpec {
                    name: "even_spacing",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(false)),
                    doc: "Also space the vertices evenly along the loop.",
                },
            ],
        ),
        with_selection(
            "poly.circularize@1",
            "Reshapes the selected edge loops into circles: an oval hole becomes round, a ragged ring a clean one (select the loop's edges with poly.select element=edge). Radius 0 fits a circle to the loop's own size.",
            vec![
                number(
                    "factor",
                    Unit::None,
                    (0.0, 1.0),
                    Some(1.0),
                    "How far toward a circle: 0 nothing, 1 a full circle.",
                ),
                number(
                    "radius",
                    Unit::Length,
                    (0.0, 1.0e3),
                    Some(0.0),
                    "Circle radius in metres; 0 fits one to the loop.",
                ),
                number(
                    "angle",
                    Unit::Angle,
                    (-std::f64::consts::TAU, std::f64::consts::TAU),
                    Some(0.0),
                    "Rotates the vertices around the circle.",
                ),
                number(
                    "flatten",
                    Unit::None,
                    (0.0, 1.0),
                    Some(1.0),
                    "How far the loop is flattened onto its best-fit plane.",
                ),
                ParamSpec {
                    name: "regular",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(true)),
                    doc: "Space the vertices evenly around the circle instead of keeping their old spacing.",
                },
            ],
        ),
    ]
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut all = selected();
    all.extend(whole_mesh());
    all
}

fn whole_mesh() -> Vec<Option<OperatorSpec>> {
    vec![
        whole(
            "poly.remesh_quads@1",
            "Rebuilds a mesh as clean, even quads of about the size you ask for (Instant Meshes): turns the uneven triangles of a field mesh, a boolean or a script into a tidy quad mesh that subdivides, smooths and deforms well. For smooth organic shapes (creatures, rocks, blobs). It does not keep sharp edges: a box with a hole cut in it, or any hard-surface part, is refused with the reason rather than returned with holes. UVs and other layers are not kept (unwrap again afterwards). Input up to 200,000 triangles; roughly a second per 50,000.",
            vec![
                ParamSpec {
                    name: "faces",
                    ty: ParamType::Int {
                        min: 8,
                        max: 20_000,
                    },
                    default: Some(Value::Int(1000)),
                    doc: "About how many quads to make (within about 10 percent).",
                },
                ParamSpec {
                    name: "seed",
                    ty: ParamType::Int {
                        min: 0,
                        max: 4_294_967_295,
                    },
                    default: Some(Value::Int(1)),
                    doc: "Picks one of the equally good layouts; the same seed gives the same mesh.",
                },
            ],
        ),
        whole(
            "poly.recalc_normals@1",
            "Makes every face point the same way, outward from the mesh (or inward), whatever its winding was. Use it after a script, a mirror or a boolean leaves some faces inside out (dark patches, faces seen through).",
            vec![choice(
                "direction",
                &["outward", "inward"],
                "outward",
                "Which side the faces face.",
            )],
        ),
        whole(
            "poly.smooth@1",
            "Evens out a surface by moving every vertex toward the average of its neighbours, a little each pass. Rounds lumpy meshes; too many passes shrinks and softens detail.",
            vec![
                number(
                    "factor",
                    Unit::None,
                    (0.0, 1.0),
                    Some(0.5),
                    "How far a vertex moves toward its neighbours each pass: 0 nothing, 1 all the way.",
                ),
                passes("iterations", 1, "Passes, 1 to 50."),
            ],
        ),
        whole(
            "poly.fill_holes@1",
            "Closes the open boundary loops of a mesh with faces (a hole left by deleting faces, an open end). A hole with more sides than max_sides stays open, so a big opening is not filled by accident.",
            vec![ParamSpec {
                name: "max_sides",
                ty: ParamType::Int { min: 3, max: 1000 },
                default: Some(Value::Int(4)),
                doc: "The most edges a hole may have and still be filled (4 fills square holes; raise it for bigger ones).",
            }],
        ),
        whole(
            "poly.beautify@1",
            "Flips the shared edge of triangle pairs where Blender's beauty measure prefers the other diagonal, to even out a triangulated mesh before smoothing or displacing it. The measure is Blender's own (by area or by angle); on a given quad the two can disagree, and a pair it already likes is left alone.",
            vec![choice(
                "method",
                &["area", "angle"],
                "area",
                "Which of Blender's beauty measures decides the diagonal.",
            )],
        ),
        whole(
            "poly.planar_faces@1",
            "Flattens each face toward its best-fit plane by moving its vertices, so quads that were bent become flat. Neighbouring faces pull on each other, so several passes settle it.",
            vec![
                passes("iterations", 10, "Passes, 1 to 50."),
                number(
                    "factor",
                    Unit::None,
                    (0.0, 1.0),
                    Some(1.0),
                    "How far vertices move toward the plane each pass.",
                ),
            ],
        ),
        whole(
            "poly.join_triangles@1",
            "Joins triangle pairs into quads where the pair is flat and square enough: turns a triangulated mesh back into quads for clean subdivision and editing. Tighter limits join fewer pairs.",
            vec![
                number(
                    "face_angle",
                    Unit::Angle,
                    (0.0, std::f64::consts::PI),
                    Some(0.698),
                    "Largest angle between the two triangles' normals that still joins (0.698 rad is 40 degrees).",
                ),
                number(
                    "shape_angle",
                    Unit::Angle,
                    (0.0, std::f64::consts::PI),
                    Some(0.698),
                    "Largest deviation of the new quad's corners from right angles that still joins (0.698 rad is 40 degrees).",
                ),
            ],
        ),
        whole(
            "poly.dissolve_degenerate@1",
            "Removes edges shorter than a distance and faces with almost no area, which cause shading glitches and failed operations after a script or a boolean.",
            vec![number(
                "distance",
                Unit::Length,
                (1.0e-9, 1.0e3),
                Some(1.0e-4),
                "Edges shorter than this, in metres, are removed.",
            )],
        ),
    ]
}
