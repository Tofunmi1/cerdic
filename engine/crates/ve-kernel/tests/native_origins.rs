//! Every element a native operator makes must name the input element it descends from. The caller copies attributes (UVs, roles, colours) by that name and refuses
//! the whole edit if one is missing, so an operator that leaves a new vertex or face unclaimed breaks any mesh that already carries attributes.

use ve_kernel::{MeshOp, MeshSelection, PokeCenter, PolygonMesh, edit_mesh};

const POSITIONS: [[f32; 3]; 8] = [
    [-0.5, -0.5, -0.5],
    [0.5, -0.5, -0.5],
    [0.5, 0.5, -0.5],
    [-0.5, 0.5, -0.5],
    [-0.5, -0.5, 0.5],
    [0.5, -0.5, 0.5],
    [0.5, 0.5, 0.5],
    [-0.5, 0.5, 0.5],
];
const CORNERS: [u32; 24] = [0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7];

fn cube() -> PolygonMesh<'static> {
    PolygonMesh { positions: &POSITIONS, face_sizes: &[4; 6], corners: &CORNERS }
}

#[test]
fn every_new_element_names_an_input_element() {
    let top_edges: [[u32; 2]; 2] = [[2, 3], [6, 7]];
    let top_face = [3_u32];
    let cases: Vec<(&str, MeshSelection<'_>, MeshOp)> = vec![
        ("loop cut", MeshSelection::default(), MeshOp::LoopCut { at: [0.0, 0.5, -0.5], cuts: 2, smooth: 0.0 }),
        ("poke", MeshSelection::default(), MeshOp::Poke { offset: 0.1, center: PokeCenter::Median, relative: false }),
        (
            "subdivide edges",
            MeshSelection { edges: &top_edges, ..MeshSelection::default() },
            MeshOp::Subdivide { cuts: 1, smooth: 0.0, grid_fill: false },
        ),
        ("solidify", MeshSelection::default(), MeshOp::Solidify { thickness: 0.1 }),
        (
            "wireframe",
            MeshSelection::default(),
            MeshOp::Wireframe { thickness: 0.05, offset: 0.0, even: true, relative: false, boundary: true, replace: true },
        ),
        ("triangulate", MeshSelection::default(), MeshOp::Triangulate),
        ("extrude", MeshSelection { faces: &top_face, ..MeshSelection::default() }, MeshOp::Extrude { offset: [0.0, 0.2, 0.0] }),
        (
            "inset",
            MeshSelection { faces: &top_face, ..MeshSelection::default() },
            MeshOp::Inset { thickness: 0.1, depth: 0.0, even: true, relative: false, boundary: true, outset: false },
        ),
        (
            "bisect with fill",
            MeshSelection::default(),
            MeshOp::Bisect { point: [0.0, 0.1, 0.0], normal: [0.0, 1.0, 0.0], distance: 1e-5, clear_inner: false, clear_outer: true, fill: true },
        ),
        // Spinning needs an open profile: one face of the cube, whose four edges are its boundary.
        ("spin", MeshSelection { faces: &top_face, ..MeshSelection::default() }, MeshOp::Spin { center: [2.0, 0.0, 0.0], axis: [0.0, 1.0, 0.0], angle: 1.5, steps: 4, merge: false, distance: 1e-5 }),
        ("mirror", MeshSelection::default(), MeshOp::Mirror { point: [0.75, 0.0, 0.0], normal: [1.0, 0.0, 0.0], merge: false, distance: 1e-5 }),
    ];
    let mut broken = Vec::new();
    for (name, selection, op) in cases {
        let result = match edit_mesh(&cube(), &selection, &op) {
            Ok(result) => result,
            Err(error) => { broken.push(format!("{name}: refused ({error:?})")); continue; }
        };
        let vertices = result.vertex_origin.iter().filter(|&&origin| origin as usize >= POSITIONS.len()).count();
        let faces = result.face_parent.iter().filter(|&&parent| parent as usize >= 6).count();
        if vertices + faces > 0 { broken.push(format!("{name}: {vertices} vertices and {faces} faces without an origin")); }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}
