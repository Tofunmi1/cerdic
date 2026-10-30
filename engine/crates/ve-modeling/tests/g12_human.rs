//! `body.human`: a person from named sliders, through the text language, into a surface with UVs (ADR 0055).
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps data that is known to be valid and reports actionable failures"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality, set_human_pack};

fn load_pack() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(path).unwrap()).unwrap();
}

fn evaluate(source: &str) -> Result<Evaluation, String> {
    load_pack();
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_human").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

fn height(source: &str) -> f32 {
    let result = evaluate(source).unwrap();
    let positions = &result.parts[0].positions;
    let (lo, hi) = positions.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p[1]), hi.max(p[1]))
    });
    hi - lo
}

#[test]
fn the_default_person_is_an_adult_standing_on_the_ground_with_uvs() {
    let result = evaluate("person = body.human@1()\noutput person = person\n").unwrap();
    let part = &result.parts[0];
    assert!(part.positions.len() > 13_000 && part.indices.len() / 3 == 27_420);
    let lowest = part.positions.iter().fold(f32::MAX, |m, p| m.min(p[1]));
    assert!(lowest.abs() < 1e-4, "the feet rest on y = 0: {lowest}");
    let highest = part.positions.iter().fold(f32::MIN, |m, p| m.max(p[1]));
    assert!((1.5..1.85).contains(&highest), "an adult: {highest}");
    assert_eq!(
        part.uvs.len(),
        part.indices.len(),
        "one UV per triangle corner"
    );
}

#[test]
fn the_sliders_change_the_person() {
    let tall = height("person = body.human@1(height=1)\noutput person = person\n");
    let short = height("person = body.human@1(height=0)\noutput person = person\n");
    let child = height("person = body.human@1(age=0.333)\noutput person = person\n");
    assert!(tall > short + 0.05, "{tall} against {short}");
    assert!(
        child < 1.5,
        "the child prototype is a pre-teen, well under adult height: {child}"
    );
}

#[test]
fn measurements_in_the_text_fit_the_body_to_them() {
    let stature = height("person = body.human@1(stature=1.92)\noutput person = person\n");
    assert!(
        (stature - 1.92).abs() < 0.015,
        "asked 1.92 m, got {stature}"
    );
    let source =
        "person = body.human@1(stature=1.75, waist=0.80, hip=0.98)\noutput person = person\n";
    let result = evaluate(source).unwrap();
    // Several measurements at once may conflict (a 1.75 m person of this age and gender has hips of about 1.0 m at that waist): a miss is reported, never silent.
    for warning in result
        .warnings
        .iter()
        .filter(|w| w.code == "human.fit_partial")
    {
        assert!(
            warning.message.contains("asked") && warning.message.contains("reached"),
            "{}",
            warning.message
        );
    }
    // An impossible height gets the nearest body and a warning that names what was reached.
    let result = evaluate("person = body.human@1(stature=2.9)\noutput person = person\n").unwrap();
    let warning = result
        .warnings
        .iter()
        .find(|w| w.code == "human.fit_partial")
        .unwrap();
    assert!(warning.message.contains("stature"), "{}", warning.message);
}

#[test]
fn a_person_is_placed_and_turned_with_their_eyes() {
    let centre = |source: &str, name: &str| {
        let result = evaluate(source).unwrap();
        let part = result.parts.iter().find(|p| p.name == name).unwrap();
        let n = part.positions.len() as f32;
        [0, 1, 2].map(|a| part.positions.iter().map(|p| p[a]).sum::<f32>() / n)
    };
    let home = centre(
        "p = body.human@1()\ne = body.eye@1(p)\noutput p = p\noutput e = e\n",
        "e",
    );
    let moved = centre(
        "p = body.human@1(at=[2, 0, 1])\ne = body.eye@1(p)\noutput p = p\noutput e = e\n",
        "e",
    );
    assert!(
        (moved[0] - home[0] - 2.0).abs() < 1e-3 && (moved[2] - home[2] - 1.0).abs() < 1e-3,
        "{home:?} {moved:?}"
    );
    // Turned by 90 degrees the person faces +x: the eyes (which sat in front of the origin, +z) are now at +x.
    let turned = centre(
        "p = body.human@1(turn=90)\ne = body.eye@1(p)\noutput p = p\noutput e = e\n",
        "e",
    );
    assert!(turned[0] > 0.04 && turned[2].abs() < 0.03, "{turned:?}");
}

#[test]
fn local_changes_and_face_actions_are_named_in_text() {
    let plain = evaluate("person = body.human@1()\noutput person = person\n").unwrap();
    let open =
        evaluate("person = body.human@1(face=\"jawOpen=1\")\noutput person = person\n").unwrap();
    assert_ne!(plain.parts[0].positions, open.parts[0].positions);
}

#[test]
fn a_misspelt_name_is_refused_with_the_valid_names() {
    let error =
        evaluate("person = body.human@1(face=\"jawOpn=1\")\noutput person = person\n").unwrap_err();
    assert!(
        error.contains("jawOpn") && error.contains("jawOpen"),
        "{error}"
    );
    let error = evaluate("person = body.human@1(local=\"nonsense\")\noutput person = person\n")
        .unwrap_err();
    assert!(error.contains("name=value"), "{error}");
}

#[test]
fn the_eyes_sit_in_the_sockets_of_the_body_they_are_built_from() {
    let result = evaluate(
        "person = body.human@1()\neyes = body.eye@1(person, iris_color=[0.3, 0.2, 0.1])\noutput person = person\noutput eyes = eyes\n",
    )
    .unwrap();
    let eyes = result.parts.iter().find(|p| p.name == "eyes").unwrap();
    assert!(eyes.positions.len() > 1000 && !eyes.indices.is_empty());
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &eyes.positions {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    // Two balls about 3 cm across, a pupil distance (5 to 7 cm) apart, at eye height.
    assert!(
        (0.05..0.09).contains(&(hi[0] - lo[0] - 0.03)) || (0.06..0.11).contains(&(hi[0] - lo[0])),
        "{lo:?} {hi:?}"
    );
    assert!(
        (1.4..1.8).contains(&((lo[1] + hi[1]) / 2.0)),
        "eye height {lo:?} {hi:?}"
    );
    assert!(
        hi[1] - lo[1] < 0.04,
        "each eye is about 3 cm tall: {}",
        hi[1] - lo[1]
    );
}

#[test]
fn the_person_is_an_editable_polygon_mesh_for_every_poly_node() {
    // poly.* nodes need a polygon mesh as input: the person's quads (and their UVs) go through them like any other.
    let result = evaluate(
        "person = body.human@1()\nsmooth = poly.subdivide@1(person, levels=1)\noutput smooth = smooth\n",
    )
    .unwrap();
    let smooth = result.parts.iter().find(|p| p.name == "smooth").unwrap();
    // One level of subdivision turns each quad into four.
    let triangles = smooth.indices.len() / 3;
    assert!(
        (109_000..110_500).contains(&triangles),
        "{triangles} triangles"
    );
    assert_eq!(
        smooth.uvs.len(),
        smooth.indices.len(),
        "the UVs survive the edit"
    );
}

#[test]
fn hair_grows_on_the_head_and_follows_its_length_and_the_person() {
    let hair = |params: &str| {
        let source =
            format!("p = body.human@1(at=[1, 0, 0])\nh = body.hair@1(p{params})\noutput h = h\n");
        let result = evaluate(&source).unwrap();
        result
            .parts
            .iter()
            .find(|p| p.name == "h")
            .unwrap()
            .positions
            .clone()
    };
    let bob = hair(", length=0.12, density=0.4");
    let long = hair(", length=0.35, density=0.4");
    assert!(bob.len() > 5_000, "{} vertices", bob.len());
    let low = bob.iter().fold(f32::MAX, |m, p| m.min(p[1]));
    let high = bob.iter().fold(f32::MIN, |m, p| m.max(p[1]));
    // On the head of an adult person: the crown is near 1.75 m, the hair starts above the shoulders, and it stands where the person does (x about 1).
    assert!((1.55..1.95).contains(&high), "crown {high}");
    assert!(low < high - 0.1, "{low} {high}");
    let mean_x = bob.iter().map(|p| p[0]).sum::<f32>() / bob.len() as f32;
    assert!((mean_x - 1.0).abs() < 0.08, "{mean_x}");
    let lowest_long = long.iter().fold(f32::MAX, |m, p| m.min(p[1]));
    assert!(
        lowest_long < low - 0.1,
        "long hair hangs lower: {lowest_long} against {low}"
    );
}

#[test]
fn eyes_need_a_body_as_input_and_say_so() {
    let error = evaluate("g = poly.grid@1(width=1, height=1, columns=2, rows=2)\neyes = body.eye@1(g)\noutput eyes = eyes\n").unwrap_err();
    assert!(error.contains("body.human"), "{error}");
}

#[test]
fn the_text_language_refuses_a_slider_out_of_range() {
    load_pack();
    let refused = parse_text(&builtin_registry(), "person = body.human@1(age=1.5)\n");
    assert!(format!("{refused:?}").contains("param.range"));
}
