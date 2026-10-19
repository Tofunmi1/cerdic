//! The model-facing measure tool: summary text, full JSON and malformed input.

use ve_measure::measure_mesh;

/// A cube of side `2 * half` as flat arrays (xyz positions, triangle indices).
fn cube(half: f32) -> (Vec<f32>, Vec<u32>) {
    let h = half;
    let positions = vec![
        -h, -h, -h, h, -h, -h, h, h, -h, -h, h, -h, -h, -h, h, h, -h, h, h, h, h, -h, h, h,
    ];
    let indices = vec![
        0, 3, 2, 0, 2, 1, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7, 3,
        1, 2, 6, 1, 6, 5,
    ];
    (positions, indices)
}

#[test]
fn a_cube_summary_reports_volume_watertight_and_thickness() {
    let (positions, indices) = cube(0.5);
    let summary = measure_mesh(&positions, &indices, &[], "summary");
    assert!(summary.contains("volume: 1.000000 m3"), "{summary}");
    assert!(summary.contains("watertight: yes"), "{summary}");
    assert!(summary.contains("triangles: 12"), "{summary}");
    assert!(summary.contains("thickness:"), "{summary}");
    // The verdict is a line of its own, before the appended thickness line: a closed cube is ok.
    assert!(summary.contains("\nok\n"), "{summary}");
}

#[test]
fn a_cube_with_one_triangle_removed_reports_open_edges_and_a_fix() {
    let (positions, mut indices) = cube(0.5);
    indices.drain(0..3);
    let summary = measure_mesh(&positions, &indices, &[], "summary");
    assert!(summary.contains("3 open edges"), "{summary}");
    assert!(
        summary.contains("close the hole or use poly.fill"),
        "{summary}"
    );
    assert!(summary.contains("watertight: no"), "{summary}");
}

#[test]
fn out_of_range_indices_return_an_error_and_do_not_panic() {
    let positions = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let indices = vec![0, 1, 9];
    let out = measure_mesh(&positions, &indices, &[], "summary");
    assert!(out.starts_with("{\"error\""), "{out}");
    assert!(out.contains("suggestion"), "{out}");
    assert!(out.contains('9'), "{out}");
}

#[test]
fn odd_flat_arrays_return_an_error_and_do_not_panic() {
    let out = measure_mesh(&[0.0, 0.0], &[0, 1, 2], &[], "summary");
    assert!(out.starts_with("{\"error\""), "{out}");
}

#[test]
fn full_json_has_every_section() {
    let (positions, indices) = cube(0.5);
    let json = measure_mesh(&positions, &indices, &[], "full");
    for key in [
        "triangles",
        "vertices",
        "bounding_box",
        "surface_area",
        "volume",
        "centre_of_mass",
        "watertight",
        "inside_out",
        "euler_characteristic",
        "genus",
        "components",
        "problems",
        "thickness",
        "symmetry",
        "uv",
        "bounding_shapes",
    ] {
        assert!(
            json.contains(&format!("\"{key}\":")),
            "missing {key}: {json}"
        );
    }
    assert!(json.contains("\"volume\":1"), "{json}");
    // No uvs were supplied, so the UV section is null rather than an error.
    assert!(json.contains("\"uv\":null"), "{json}");
}

#[test]
fn full_json_reports_uvs_when_given() {
    let (positions, indices) = cube(0.5);
    // A trivial per-vertex UV for each of the eight corners.
    let uvs: Vec<f32> = (0u8..8)
        .flat_map(|i| [f32::from(i % 4) / 3.0, f32::from(i / 4)])
        .collect();
    let json = measure_mesh(&positions, &indices, &uvs, "full");
    assert!(json.contains("\"islands\":"), "{json}");
    assert!(!json.contains("\"uv\":null"), "{json}");
}
