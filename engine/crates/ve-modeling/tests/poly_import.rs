//! `poly.import`: the user's own OBJ / ASCII PLY / ASCII STL text becomes an editable polygon mesh. Do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "fixture"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_import").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(("apply".to_owned(), format!("{applied:?}")));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A program that imports `data` (escaped for the text language) and outputs it.
fn import(format: &str, data: &str, extra: &str) -> String {
    let escaped = data
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("m = poly.import@1(format=\"{format}\", data=\"{escaped}\"{extra})\noutput m = m\n")
}

fn bounds(result: &Evaluation) -> ([f32; 3], [f32; 3]) {
    let part = &result.parts[0];
    let mut low = [f32::MAX; 3];
    let mut high = [f32::MIN; 3];
    for p in &part.positions {
        for axis in 0..3 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    }
    (low, high)
}

fn code(error: &(String, String)) -> &str {
    &error.1
}

const CUBE_OBJ: &str = "# a unit cube, quads, outward\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv 0 0 1\nv 1 0 1\nv 1 1 1\nv 0 1 1\n\
f 1 4 3 2\nf 5 6 7 8\nf 1 2 6 5\nf 2 3 7 6\nf 3 4 8 7\nf 4 1 5 8\n";

#[test]
fn an_obj_cube_keeps_its_quads_size_and_volume() {
    let result = evaluate(&import("obj", CUBE_OBJ, "")).unwrap_or_else(|e| panic!("{e:?}"));
    let (low, high) = bounds(&result);
    assert_eq!((low, high), ([0.0; 3], [1.0; 3]));
    assert_eq!(
        result.parts[0].indices.len(),
        12 * 3,
        "six quads are twelve triangles"
    );
    assert!(
        (result.parts[0].volume - 1.0).abs() < 1e-6,
        "a closed outward cube has volume 1: {}",
        result.parts[0].volume
    );
}

#[test]
fn scale_and_up_axis_turn_a_millimetre_z_up_file_into_metres_y_up() {
    // A 2000 x 1000 x 500 mm box with its long side along +X, its height 500 along Z (up in the file).
    let obj = "v 0 0 0\nv 2000 0 0\nv 2000 1000 0\nv 0 1000 0\nv 0 0 500\nv 2000 0 500\nv 2000 1000 500\nv 0 1000 500\n\
f 1 4 3 2\nf 5 6 7 8\nf 1 2 6 5\nf 2 3 7 6\nf 3 4 8 7\nf 4 1 5 8\n";
    let result = evaluate(&import("obj", obj, ", scale=0.001, up=\"z\""))
        .unwrap_or_else(|e| panic!("{e:?}"));
    let (low, high) = bounds(&result);
    let size = [high[0] - low[0], high[1] - low[1], high[2] - low[2]];
    // x stays 2 m, the file's Z (500 mm) becomes Y, the file's Y (1000 mm) becomes -Z depth.
    assert!(
        (size[0] - 2.0).abs() < 1e-5
            && (size[1] - 0.5).abs() < 1e-5
            && (size[2] - 1.0).abs() < 1e-5,
        "{size:?}"
    );
    assert!(
        (result.parts[0].volume - 1.0).abs() < 1e-4,
        "still outward and 2 x 0.5 x 1 = 1 m3: {}",
        result.parts[0].volume
    );
}

#[test]
fn polygons_of_any_size_negative_indices_and_texture_coordinates_are_read() {
    // A pentagon fan and a triangle; the last face uses negative indices; every corner has a texture coordinate.
    let obj = "v 0 0 0\nv 1 0 0\nv 1.3 1 0\nv 0.5 1.6 0\nv -0.3 1 0\nv 3 0 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvt 0.5 0.5\n\
f 1/1 2/2 3/3 4/4 5/5\nf -1/1 -6/2 -5/3\n";
    let result = evaluate(&import("obj", obj, "")).unwrap_or_else(|e| panic!("{e:?}"));
    let part = &result.parts[0];
    assert_eq!(
        part.indices.len(),
        3 * 3 + 3,
        "a pentagon is three triangles, plus one triangle"
    );
    assert_eq!(
        part.uvs.len(),
        part.indices.len(),
        "one UV per triangle corner"
    );
    assert!(
        part.uvs.iter().any(|uv| *uv == [0.5, 0.5]),
        "the pentagon's fifth coordinate arrived"
    );
}

#[test]
fn ascii_ply_and_ascii_stl_import_the_same_shape() {
    let ply = "ply\nformat ascii 1.0\ncomment a tetrahedron\nelement vertex 4\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\n\
element face 4\nproperty list uchar int vertex_indices\nend_header\n0 0 0 255\n1 0 0 255\n0 1 0 255\n0 0 1 255\n3 0 2 1\n3 0 1 3\n3 0 3 2\n3 1 2 3\n";
    let stl = "solid tetra\nfacet normal 0 0 -1\nouter loop\nvertex 0 0 0\nvertex 0 1 0\nvertex 1 0 0\nendloop\nendfacet\n\
facet normal 0 -1 0\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 0 1\nendloop\nendfacet\n\
facet normal -1 0 0\nouter loop\nvertex 0 0 0\nvertex 0 0 1\nvertex 0 1 0\nendloop\nendfacet\n\
facet normal 1 1 1\nouter loop\nvertex 1 0 0\nvertex 0 1 0\nvertex 0 0 1\nendloop\nendfacet\nendsolid tetra\n";
    for (format, text) in [("ply", ply), ("stl", stl)] {
        let result =
            evaluate(&import(format, text, "")).unwrap_or_else(|e| panic!("{format}: {e:?}"));
        let part = &result.parts[0];
        assert_eq!(part.indices.len(), 12, "{format}: four triangles");
        assert_eq!(
            part.positions.len(),
            4,
            "{format}: four shared vertices (STL welds duplicates)"
        );
        assert!(
            (part.volume - 1.0 / 6.0).abs() < 1e-6,
            "{format}: a unit tetrahedron is a sixth of a cube: {}",
            part.volume
        );
    }
}

#[test]
fn an_imported_mesh_is_an_ordinary_poly_that_can_be_edited() {
    let program = format!(
        "{}big = xform.scale@1(m, by=[3, 3, 3])\noutput big = big\n",
        import("obj", CUBE_OBJ, "").replace("output m = m\n", "")
    );
    let result = evaluate(&program).unwrap_or_else(|e| panic!("{e:?}"));
    let (low, high) = bounds(&result);
    assert!(
        ((high[0] - low[0]) - 3.0).abs() < 1e-5,
        "scaled after import: {low:?} {high:?}"
    );
    // And a poly operator accepts it.
    let subdivided = format!(
        "{}s = poly.subdivide@1(m)\noutput s = s\n",
        import("obj", CUBE_OBJ, "").replace("output m = m\n", "")
    );
    let result = evaluate(&subdivided).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(
        result.parts[0].indices.len() > 36,
        "subdivision refined the imported cube"
    );
}

#[test]
fn bad_files_are_refused_with_the_line_and_the_way_out() {
    let cases: [(&str, &str, &str); 8] = [
        ("obj", "v 0 0 0\nv 1 0 0\nf 1 2 9\n", "line 3"),
        ("obj", "v 0 0 0\nv 1 0\n", "line 2"),
        (
            "obj",
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2\n",
            "at least three",
        ),
        ("obj", "v 0 0 0\nv 1 0 0\n", "no faces"),
        (
            "obj",
            "v 0 0 nan\nv 1 0 0\nv 0 1 0\nf 1 2 3\n",
            "not finite",
        ),
        (
            "ply",
            "ply\nformat binary_little_endian 1.0\nend_header\n",
            "ascii",
        ),
        ("stl", "\u{0}\u{0}\u{0}binary", "binary STL"),
        ("obj", "   \n", "no data"),
    ];
    for (format, data, expect) in cases {
        let error = evaluate(&import(format, data, "")).expect_err(data);
        assert!(
            code(&error).contains(expect) || error.0.contains(expect),
            "{format} {data:?}: wanted `{expect}`, got {error:?}"
        );
    }
}

#[test]
fn the_same_text_always_gives_the_same_mesh() {
    let a = evaluate(&import("obj", CUBE_OBJ, "")).unwrap();
    let b = evaluate(&import("obj", CUBE_OBJ, "")).unwrap();
    assert_eq!(a.parts[0].positions, b.parts[0].positions);
    assert_eq!(a.parts[0].indices, b.parts[0].indices);
}
