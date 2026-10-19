//! Hollow surface composition cases.
use serde_json::{Value, json};
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluator, Quality};

type Row = (&'static str, &'static str, Value, &'static [&'static str]);

fn evaluate(
    rows: &[(&str, &str, Value, &[&str])],
    last: &str,
) -> Result<ve_modeling::Evaluation, String> {
    let mut session = Session::with_builtin(3);
    let mut commands = Vec::new();
    for (alias, op, params, inputs) in rows {
        commands.push(json!({"add_node": {"op": op, "params": params, "alias": alias}}));
        for input in *inputs {
            let (port, source) = input.split_once(':').unwrap_or(("in", input));
            commands.push(json!({"connect": {"from": {"node": source, "port": "out"}, "to": {"node": alias, "port": port}}}));
        }
    }
    commands.push(json!({"set_output": {"name": "part", "from": {"node": last, "port": "out"}}}));
    let batch = json!({"doc": session.document().id, "base_revision": 0, "transaction": "t_g5_hollow", "commands": commands});
    if let outcome @ Outcome::Rejected { .. } = session.apply_json(&batch.to_string()) {
        return Err(format!("fixture command batch rejected: {outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

#[test]
fn g5_03_difference_of_lofts_makes_a_hollow_vase_with_an_open_rim() {
    let result = evaluate(&vase_rows(), "$vase").unwrap();
    assert_eq!(result.parts.len(), 1);
    assert!(result.parts[0].volume > 0.0);
    let top = result.parts[0]
        .positions
        .iter()
        .filter(|point| (f64::from(point[1]) - 0.9).abs() < 1.0e-4)
        .map(|point| f64::from(point[0]).hypot(f64::from(point[2])))
        .fold(f64::INFINITY, f64::min);
    assert!(
        top > 0.25,
        "the cavity should reach the open rim, min top radius {top}"
    );
    for step in 0_u8..20 {
        let height = 0.2 + f32::from(step) * 0.035;
        let (inner, outer) =
            mesh_radial_span(&result.parts[0].positions, &result.parts[0].indices, height)
                .unwrap_or_else(|| panic!("no cross-section at {height}"));
        assert!(
            outer - inner >= 0.04,
            "wall at {height} is {}",
            outer - inner
        );
    }
}

#[test]
fn g5_03_section_bound_catches_a_thin_wall_between_sampling_heights() {
    let result = evaluate(&thin_wall_rows(), "$vase").unwrap();
    let section_gaps = [0.1_f64, 0.1, 0.995, 0.1, 0.1].map(|inner| 1.0 - inner);
    let proven_bound = section_gaps.into_iter().reduce(f64::min).unwrap_or(0.0);
    assert!((proven_bound - 0.005).abs() < 1.0e-9);
    assert!(proven_bound < 0.04);

    let part = &result.parts[0];
    let thin_section = 0.177_f32 * 3.0 + 0.0001;
    let (inner, outer) = mesh_radial_span(&part.positions, &part.indices, thin_section).unwrap();
    assert!(outer - inner < 0.01, "section wall is {}", outer - inner);
    for sample in [0.515_f32, 0.55] {
        let (inner, outer) = mesh_radial_span(&part.positions, &part.indices, sample).unwrap();
        assert!(
            outer - inner > 0.04,
            "sampled wall at {sample} is {}",
            outer - inner
        );
    }
}

#[test]
fn g5_03_extended_inner_sweep_makes_a_hollow_pipe() {
    let result = evaluate(&pipe_rows(), "$pipe").unwrap();
    assert!(result.parts[0].volume > 0.0);
    let (inner, outer) = radial_span(&result.parts[0].positions, 0.5);
    assert!(outer - inner > 0.08, "pipe wall is {}", outer - inner);
    for end in [0.0_f32, 1.0] {
        let (opening, _) = radial_span(&result.parts[0].positions, end);
        assert!(opening > 0.1, "pipe end at {end} should stay open");
    }
}

#[test]
fn g5_03_extended_inner_sweep_makes_a_hollow_curved_pipe() {
    let result = evaluate(&curved_pipe_rows(), "$pipe").unwrap();
    let part = &result.parts[0];
    assert!(part.volume > 0.0);
    let bounds = part.positions.iter().fold(
        ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]),
        |(mut low, mut high), point| {
            for axis in 0..3 {
                low[axis] = low[axis].min(point[axis]);
                high[axis] = high[axis].max(point[axis]);
            }
            (low, high)
        },
    );
    assert!(
        bounds.0[0] < -0.1 && bounds.1[0] > 0.9,
        "curved extent {bounds:?}"
    );
    assert!(bounds.1[1] - bounds.0[1] > 0.9, "curved extent {bounds:?}");
}

fn vase_rows() -> Vec<Row> {
    const OUTER: &[&str] = &[
        "sections:$o0",
        "sections:$o1",
        "sections:$o2",
        "sections:$o3",
        "sections:$o4",
        "sections:$o5",
    ];
    const INNER: &[&str] = &[
        "sections:$i0",
        "sections:$i1",
        "sections:$i2",
        "sections:$i3",
        "sections:$i4",
        "sections:$i5",
    ];
    vec![
        profile("$o0", 0.30, 48),
        profile("$o1", 0.24, 48),
        profile("$o2", 0.20, 48),
        profile("$o3", 0.26, 48),
        profile("$o4", 0.35, 48),
        profile("$o5", 0.40, 48),
        ("$outer", "solid.loft@1", json!({"spacing": 0.18}), OUTER),
        profile("$i0", 0.20, 48),
        profile("$i1", 0.15, 48),
        profile("$i2", 0.12, 48),
        profile("$i3", 0.18, 48),
        profile("$i4", 0.27, 48),
        profile("$i5", 0.30, 48),
        (
            "$inner_raw",
            "solid.loft@1",
            json!({"spacing": 0.18}),
            INNER,
        ),
        (
            "$inner",
            "xform.move@1",
            json!({"by": [0.0, 0.18, 0.0]}),
            &["$inner_raw"],
        ),
        (
            "$vase",
            "solid.difference@1",
            json!({}),
            &["in:$outer", "in:$inner"],
        ),
    ]
}

fn pipe_rows() -> Vec<Row> {
    vec![
        profile("$outer_profile", 0.25, 32),
        profile("$inner_profile", 0.15, 32),
        (
            "$outer_path",
            "curve.polyline@1",
            json!({"points": [[0,0,0],[0,0.5,0],[0,1,0]]}),
            &[],
        ),
        (
            "$inner_path",
            "curve.polyline@1",
            json!({"points": [[0,-0.1,0],[0,0.5,0],[0,1.1,0]]}),
            &[],
        ),
        (
            "$outer",
            "solid.sweep@1",
            json!({}),
            &["curve:$outer_path", "profile:$outer_profile"],
        ),
        (
            "$inner",
            "solid.sweep@1",
            json!({}),
            &["curve:$inner_path", "profile:$inner_profile"],
        ),
        (
            "$pipe",
            "solid.difference@1",
            json!({}),
            &["in:$outer", "in:$inner"],
        ),
    ]
}

fn thin_wall_rows() -> Vec<Row> {
    const OUTER: &[&str] = &[
        "sections:$o0",
        "sections:$o1",
        "sections:$o2",
        "sections:$o3",
        "sections:$o4",
    ];
    const INNER: &[&str] = &[
        "sections:$i0",
        "sections:$i1",
        "sections:$i2",
        "sections:$i3",
        "sections:$i4",
    ];
    vec![
        profile("$o0", 1.0, 48),
        profile("$o1", 1.0, 48),
        profile("$o2", 1.0, 48),
        profile("$o3", 1.0, 48),
        profile("$o4", 1.0, 48),
        profile("$i0", 0.1, 48),
        profile("$i1", 0.1, 48),
        profile("$i2", 0.995, 48),
        profile("$i3", 0.1, 48),
        profile("$i4", 0.1, 48),
        ("$outer", "solid.loft@1", json!({"spacing": 0.177}), OUTER),
        (
            "$inner_raw",
            "solid.loft@1",
            json!({"spacing": 0.177}),
            INNER,
        ),
        (
            "$inner",
            "xform.move@1",
            json!({"by": [0.0, 0.177, 0.0]}),
            &["$inner_raw"],
        ),
        (
            "$vase",
            "solid.difference@1",
            json!({}),
            &["in:$outer", "in:$inner"],
        ),
    ]
}

fn curved_pipe_rows() -> Vec<Row> {
    vec![
        profile("$outer_profile", 0.22, 32),
        profile("$inner_profile", 0.12, 32),
        (
            "$outer_path",
            "curve.bezier@1",
            json!({"points": [[0,0,0],[0,0.7,0],[0.3,1,0],[1,1,0]], "steps": 24}),
            &[],
        ),
        (
            "$inner_path",
            "curve.bezier@1",
            json!({"points": [[0,-0.2,0],[0,0.7,0],[0.3,1,0],[1.2,1,0]], "steps": 24}),
            &[],
        ),
        (
            "$outer",
            "solid.sweep@1",
            json!({}),
            &["curve:$outer_path", "profile:$outer_profile"],
        ),
        (
            "$inner",
            "solid.sweep@1",
            json!({}),
            &["curve:$inner_path", "profile:$inner_profile"],
        ),
        (
            "$pipe",
            "solid.difference@1",
            json!({}),
            &["in:$outer", "in:$inner"],
        ),
    ]
}

fn profile(alias: &'static str, radius: f64, segments: u64) -> Row {
    (
        alias,
        "profile.circle@1",
        json!({"radius": radius, "segments": segments}),
        &[],
    )
}

fn radial_span(points: &[[f32; 3]], height: f32) -> (f32, f32) {
    points
        .iter()
        .filter(|point| (point[1] - height).abs() < 1.0e-4)
        .map(|point| point[0].hypot(point[2]))
        .fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(inner, outer), radius| (inner.min(radius), outer.max(radius)),
        )
}

fn mesh_radial_span(points: &[[f32; 3]], indices: &[u32], height: f32) -> Option<(f32, f32)> {
    let mut radii = Vec::new();
    for triangle in indices.chunks_exact(3) {
        for edge in [[0, 1], [1, 2], [2, 0]] {
            let (Ok(first_index), Ok(second_index)) = (
                usize::try_from(triangle[edge[0]]),
                usize::try_from(triangle[edge[1]]),
            ) else {
                return None;
            };
            let first = points[first_index];
            let second = points[second_index];
            if (first[1] < height && second[1] > height)
                || (first[1] > height && second[1] < height)
            {
                let fraction = (height - first[1]) / (second[1] - first[1]);
                let x_coord = first[0] + fraction * (second[0] - first[0]);
                let z_coord = first[2] + fraction * (second[2] - first[2]);
                radii.push(x_coord.hypot(z_coord));
            }
        }
    }
    let inner = radii.iter().copied().reduce(f32::min)?;
    let outer = radii.iter().copied().reduce(f32::max)?;
    Some((inner, outer))
}
