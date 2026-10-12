//! G3: profile operators, solids from profiles, the wall constraint and the frozen vase transaction.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "case helpers unwrap fixtures that are known to be valid and fail loudly otherwise"
)]
use serde_json::{Value, json};
use std::f64::consts::{PI, TAU};
use ve_graph::{Diagnostic, Document, Outcome, Session};
use ve_modeling::{Evaluation, Evaluator, Quality};

struct Lab {
    session: Session,
    evaluator: Evaluator,
    key: u32,
}

impl Lab {
    fn new() -> Self {
        Self {
            session: Session::with_builtin(7),
            evaluator: Evaluator::default(),
            key: 0,
        }
    }

    /// Applies commands to a candidate and evaluates it; only a fully evaluated candidate replaces
    /// the committed session, as the document host does.
    fn commit(&mut self, commands: &[Value]) -> Result<Evaluation, Box<Diagnostic>> {
        self.key += 1;
        let mut candidate = self.session.clone();
        let batch = json!({"doc": candidate.document().id, "base_revision": candidate.document().revision,
            "transaction": format!("t_lab{}", self.key), "commands": commands});
        match candidate.apply_json(&batch.to_string()) {
            Outcome::Committed { .. } => {}
            Outcome::Rejected { diagnostics } => return Err(Box::new(diagnostics[0].clone())),
            other => panic!("unexpected outcome {other:?}"),
        }
        let result = self
            .evaluator
            .evaluate(candidate.document(), Quality::Preview)?;
        self.session = candidate;
        Ok(result)
    }

    fn doc(&self) -> &Document {
        self.session.document()
    }

    fn id(&self, label: &str) -> String {
        let Some(node) = self
            .doc()
            .nodes
            .iter()
            .find(|n| n.label.as_deref() == Some(label))
        else {
            panic!("no node labelled {label}");
        };
        node.id.to_string()
    }
}

fn add(alias: &str, op: &str, params: &Value) -> Value {
    json!({"add_node": {"op": op, "params": params, "label": alias.trim_start_matches('$'), "alias": alias}})
}

fn connect(from: &str, to: &str, port: &str) -> Value {
    json!({"connect": {"from": {"node": from, "port": "out"}, "to": {"node": to, "port": port}}})
}

fn output(name: &str, from: &str) -> Value {
    json!({"set_output": {"name": name, "from": {"node": from, "port": "out"}}})
}

fn set(lab: &Lab, label: &str, param: &str, value: &Value) -> Value {
    json!({"set_param": {"node": lab.id(label), "param": param, "value": value}})
}

fn volume(result: &Evaluation) -> f64 {
    result.parts[0].volume
}

fn relative(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn rect(w: f64, h: f64) -> Value {
    json!([
        [-w / 2.0, -h / 2.0],
        [w / 2.0, -h / 2.0],
        [w / 2.0, h / 2.0],
        [-w / 2.0, h / 2.0]
    ])
}

/// One extruded profile, optionally offset first. Returns the lab and the evaluation.
fn extruded(
    points: &Value,
    offset: Option<f64>,
    extrude: &Value,
) -> Result<Evaluation, Box<Diagnostic>> {
    let mut lab = Lab::new();
    let mut commands = vec![add("$p", "profile.polygon@1", &json!({ "points": points }))];
    let source = if let Some(delta) = offset {
        commands.push(add("$o", "profile.offset@1", &json!({ "delta": delta })));
        commands.push(connect("$p", "$o", "in"));
        "$o"
    } else {
        "$p"
    };
    commands.push(add("$e", "solid.extrude@1", extrude));
    commands.push(connect(source, "$e", "in"));
    commands.push(output("part", "$e"));
    lab.commit(&commands)
}

#[test]
fn g3_07_offset_area_matches_the_analytic_value() {
    let result = extruded(&rect(0.2, 0.1), Some(-0.01), &json!({"height": 1.0})).unwrap();
    assert!(relative(volume(&result), 0.18 * 0.08) < 1e-6);
    let grown = extruded(&rect(0.2, 0.1), Some(0.02), &json!({"height": 1.0})).unwrap();
    assert!(relative(volume(&grown), 0.24 * 0.14) < 1e-6);
}

#[test]
fn g3_08_twisted_extrusion_keeps_volume_and_bounds() {
    let plain = extruded(&rect(1.0, 1.0), None, &json!({"height": 1.0})).unwrap();
    let twisted = extruded(
        &rect(1.0, 1.0),
        None,
        &json!({"height": 1.0, "divisions": 64, "twist": PI / 2.0}),
    )
    .unwrap();
    assert!(relative(volume(&twisted), volume(&plain)) < 1e-3);
    let ys: Vec<f32> = twisted.parts[0].positions.iter().map(|p| p[1]).collect();
    let (lo, hi) = ys
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
    assert!(lo.abs() < 1e-6 && (hi - 1.0).abs() < 1e-6, "{lo} {hi}");
}

fn revolved(segments: i64, angle: f64) -> f64 {
    let mut lab = Lab::new();
    let points = json!([[0.02, 0.0], [0.05, 0.0], [0.05, 0.1], [0.02, 0.1]]);
    let result = lab
        .commit(&[
            add("$p", "profile.polygon@1", &json!({"points": points})),
            add(
                "$r",
                "solid.revolve@1",
                &json!({"angle": angle, "segments": segments}),
            ),
            connect("$p", "$r", "in"),
            output("part", "$r"),
        ])
        .unwrap();
    volume(&result)
}

#[test]
fn g3_09_full_revolve_matches_the_faceted_and_circular_volume() {
    let ring = (0.05_f64.powi(2) - 0.02_f64.powi(2)) * 0.1;
    let faceted = |n: f64| n / 2.0 * (TAU / n).sin() * ring;
    let v64 = revolved(64, TAU);
    assert!(relative(v64, faceted(64.0)) < 1e-6);
    assert!(relative(v64, PI * ring) < 2e-3);
    assert!(relative(revolved(128, TAU), PI * ring) < 1e-3);
}

#[test]
fn g3_10_half_revolve_is_half_the_volume() {
    assert!(relative(revolved(64, PI), revolved(64, TAU) / 2.0) < 1e-3);
}

#[test]
fn g3_11_self_intersecting_profile_is_a_defined_error() {
    let bow_tie = json!([[0, 0], [1, 1], [1, 0], [0, 1]]);
    let error = extruded(&bow_tie, None, &json!({"height": 1.0})).unwrap_err();
    assert_eq!(error.code, "kernel.invalid_input");
    assert!(error.node.is_some());
}

#[test]
fn g3_12_zero_area_and_zero_height_are_distinct_outcomes() {
    let line = json!([[0, 0], [1, 0], [2, 0]]);
    assert_eq!(
        extruded(&line, None, &json!({"height": 1.0}))
            .unwrap_err()
            .code,
        "kernel.invalid_input"
    );
    let flat = extruded(&rect(1.0, 1.0), None, &json!({"height": 0.0})).unwrap_err();
    assert_eq!(flat.code, "param.range");
    assert_eq!(flat.param.as_deref(), Some("height"));
}

const OUTER: [[f64; 2]; 6] = [
    [0.0, 0.0],
    [0.06, 0.0],
    [0.11, 0.12],
    [0.07, 0.26],
    [0.05, 0.30],
    [0.0, 0.30],
];
const INNER: [[f64; 2]; 6] = [
    [0.0, 0.01],
    [0.056, 0.01],
    [0.104, 0.12],
    [0.064, 0.26],
    [0.040, 0.31],
    [0.0, 0.31],
];

fn vase() -> (Lab, Evaluation) {
    let mut lab = Lab::new();
    let result = lab
        .commit(&[
            add("$outer", "profile.polygon@1", &json!({"points": OUTER})),
            add("$inner", "profile.polygon@1", &json!({"points": INNER})),
            add("$wall", "check.min_wall@1", &json!({"min": 0.003})),
            connect("$outer", "$wall", "outer"),
            connect("$inner", "$wall", "inner"),
            add(
                "$body",
                "solid.revolve@1",
                &json!({"angle": TAU, "segments": 64}),
            ),
            connect("$outer", "$body", "in"),
            add(
                "$cavity",
                "solid.revolve@1",
                &json!({"angle": TAU, "segments": 64}),
            ),
            connect("$inner", "$cavity", "in"),
            add("$vase", "solid.difference@1", &json!({})),
            connect("$body", "$vase", "in"),
            connect("$cavity", "$vase", "in"),
            output("vase", "$vase"),
        ])
        .unwrap();
    (lab, result)
}

fn point(points: &[[f64; 2]; 6], index: usize, x: f64) -> Value {
    let mut copy = *points;
    copy[index][0] = x;
    json!(copy)
}

#[test]
fn g3_13_vase_builds_and_widening_both_profiles_passes_with_ids_kept() {
    let (mut lab, first) = vase();
    assert_eq!(lab.doc().nodes.len(), 6);
    assert_eq!(first.parts.len(), 1);
    let ids: Vec<_> = lab.doc().nodes.iter().map(|n| n.id.clone()).collect();
    let widen = [
        set(&lab, "outer", "points", &point(&OUTER, 4, 0.07)),
        set(&lab, "inner", "points", &point(&INNER, 4, 0.064)),
    ];
    let second = lab.commit(&widen).unwrap();
    assert!(volume(&second) > 0.0);
    let after: Vec<_> = lab.doc().nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(ids, after);
}

#[test]
fn g3_14_widening_only_the_inner_profile_breaks_the_wall_and_changes_nothing() {
    let (mut lab, _) = vase();
    let before = lab.doc().clone();
    let error = lab
        .commit(&[set(&lab, "inner", "points", &point(&INNER, 4, 0.064))])
        .unwrap_err();
    assert_eq!(error.code, "constraint.min_wall");
    assert_eq!(
        error.node.as_ref().map(ToString::to_string),
        Some(lab.id("wall"))
    );
    let related: Vec<_> = error.related.iter().map(ToString::to_string).collect();
    assert!(related.contains(&lab.id("outer")) && related.contains(&lab.id("inner")));
    assert_eq!(lab.doc(), &before);
}

#[test]
fn g3_14b_widening_only_the_outer_profile_thickens_the_wall() {
    let (mut lab, _) = vase();
    lab.commit(&[set(&lab, "outer", "points", &point(&OUTER, 4, 0.07))])
        .unwrap();
}

#[test]
fn g3_14c_a_thinner_requirement_is_enforced_with_measured_values() {
    let (mut lab, _) = vase();
    let error = lab
        .commit(&[set(&lab, "wall", "min", &json!(0.02))])
        .unwrap_err();
    assert_eq!(error.code, "constraint.min_wall");
    assert_eq!(error.limit, Some(0.02));
}

/// A hollow rectangular tube: outer 100 x 60 mm, inner 90 x 50 mm (a 5 mm wall), extruded.
fn tube(min: f64) -> Lab {
    let mut lab = Lab::new();
    lab.commit(&[
        add(
            "$outer",
            "profile.polygon@1",
            &json!({"points": rect(0.1, 0.06)}),
        ),
        add(
            "$inner",
            "profile.polygon@1",
            &json!({"points": rect(0.09, 0.05)}),
        ),
        add("$wall", "check.min_wall@1", &json!({ "min": min })),
        connect("$outer", "$wall", "outer"),
        connect("$inner", "$wall", "inner"),
        add("$ring", "profile.difference@1", &json!({})),
        connect("$outer", "$ring", "in"),
        connect("$inner", "$ring", "in"),
        add("$tube", "solid.extrude@1", &json!({"height": 0.05})),
        connect("$ring", "$tube", "in"),
        output("tube", "$tube"),
    ])
    .unwrap();
    lab
}

#[test]
fn g3_14d_the_wall_check_works_on_any_pair_of_profiles() {
    let mut lab = tube(0.003);
    // A 5 mm wall passes a 3 mm minimum and fails a 6 mm one, reporting the measured thickness.
    let error = lab
        .commit(&[set(&lab, "wall", "min", &json!(0.006))])
        .unwrap_err();
    assert_eq!(error.code, "constraint.min_wall");
    let measured = error.observed.unwrap();
    assert!((measured - 0.005).abs() < 2.0e-4, "measured {measured}");
    // Sliding the cavity sideways leaves a 1 mm wall on one side.
    let shifted = json!([
        [-0.041, -0.025],
        [0.049, -0.025],
        [0.049, 0.025],
        [-0.041, 0.025]
    ]);
    let error = lab
        .commit(&[set(&lab, "inner", "points", &shifted)])
        .unwrap_err();
    assert!(
        (error.observed.unwrap() - 0.001).abs() < 2.0e-4,
        "{:?}",
        error.observed
    );
    // A cavity that swallows the outline leaves no wall at all.
    let huge = json!(rect(0.2, 0.2));
    let error = lab
        .commit(&[set(&lab, "inner", "points", &huge)])
        .unwrap_err();
    assert_eq!(error.code, "constraint.min_wall");
    assert_eq!(error.observed, Some(0.0));
}

#[test]
fn g3_20_changing_segments_reruns_only_the_dependents() {
    let (mut lab, first) = vase();
    assert_eq!(first.evaluated_nodes, 6);
    let second = lab
        .commit(&[set(&lab, "body", "segments", &json!(96))])
        .unwrap();
    assert_eq!(second.evaluated_nodes, 2, "body and the difference re-run");
    assert_eq!(second.cache_hits, 4);
}

#[test]
fn g3_operators_in_the_provider_schema_are_ready_only_after_their_cases() {
    let registry = ve_graph::builtin_registry();
    for id in [
        "profile.polygon@1",
        "profile.offset@1",
        "profile.difference@1",
        "solid.extrude@1",
        "solid.revolve@1",
        "check.min_wall@1",
    ] {
        let spec = registry.get(&ve_graph::OpId::parse(id).unwrap()).unwrap();
        assert_eq!(spec.readiness, ve_graph::Readiness::Ready, "{id}");
    }
}
