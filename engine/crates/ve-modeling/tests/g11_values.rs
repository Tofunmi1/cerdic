//! Value sockets: a number computed or measured by one node fills a number parameter of another, from the text a model
//! writes to the evaluated solid.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helpers unwrap fixtures that are known to be valid and report failures by panicking"
)]
use ve_graph::{
    CommandBatch, Document, Outcome, Session, TransactionId, builtin_registry, parse_text, to_text,
};
use ve_modeling::{Evaluator, Quality};

fn session(text: &str) -> Session {
    let batch = parse_text(&builtin_registry(), text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_values").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    session
}

fn evaluate_doc(doc: &Document) -> Result<f64, (String, Option<String>)> {
    Evaluator::default()
        .evaluate(doc, Quality::Final)
        .map(|e| e.parts[0].volume)
        .map_err(|e| (e.code.clone(), e.suggestion.clone()))
}

fn volume(text: &str) -> f64 {
    evaluate_doc(session(text).document()).unwrap_or_else(|(code, _)| panic!("{code}"))
}

fn refused(text: &str) -> (String, Option<String>) {
    evaluate_doc(session(text).document()).unwrap_err()
}

#[test]
fn arithmetic_on_values_drives_a_radius() {
    // A radius of 0.5 doubled by value.math is 1.0, the same sphere as typing radius=1.
    let linked = volume(
        "r = value.number@1(value=0.5)\n\
         d = value.math@1(op=\"mul\", a=r, b=2)\n\
         ball = solid.sphere@1(radius=d, segments=48)\n\
         output ball",
    );
    let typed = volume("ball = solid.sphere@1(radius=1, segments=48)\noutput ball");
    assert!(
        (linked - typed).abs() < 1.0e-9,
        "linked {linked}, typed {typed}"
    );
}

#[test]
fn one_component_of_a_vector_can_be_linked() {
    // size.y comes from a value; x and z stay as typed: 1 x 3 x 1.
    let v = volume(
        "h = value.number@1(value=3)\n\
         box = solid.box@1(size=[1, 1, 1], size.y=h)\n\
         output box",
    );
    assert!((v - 3.0).abs() < 1.0e-6, "volume {v}");
}

#[test]
fn a_measurement_of_one_part_sizes_another() {
    // The first box is 2 tall; the second takes its x from that height: 2 x 1 x 1.
    let v = volume(
        "a = solid.box@1(size=[1, 2, 3])\n\
         h = value.size@1(a, measure=\"height\")\n\
         b = solid.box@1(size=[1, 1, 1], size.x=h)\n\
         output b",
    );
    assert!((v - 2.0).abs() < 1.0e-6, "volume {v}");
}

#[test]
fn every_measure_reads_the_right_number() {
    // A 1 x 2 x 3 box: width 1, height 2, depth 3, longest 3, shortest 1, volume 6, area 22.
    for (measure, expected) in [
        ("width", 1.0),
        ("height", 2.0),
        ("depth", 3.0),
        ("longest", 3.0),
        ("shortest", 1.0),
        ("volume", 6.0),
        ("area", 22.0),
    ] {
        let v = volume(&format!(
            "a = solid.box@1(size=[1, 2, 3])\n\
             m = value.size@1(a, measure=\"{measure}\")\n\
             b = solid.box@1(size=[1, 1, 1], size.x=m)\n\
             output b"
        ));
        assert!(
            (v - expected).abs() < 1.0e-4,
            "{measure}: box volume {v}, expected {expected}"
        );
    }
}

#[test]
fn values_chain_through_several_nodes() {
    // (2 + 3) * 4 = 20.
    let v = volume(
        "s = value.math@1(op=\"add\", a=2, b=3)\n\
         p = value.math@1(op=\"mul\", a=s, b=4)\n\
         box = solid.box@1(size=[1, 1, 1], size.x=p)\n\
         output box",
    );
    assert!((v - 20.0).abs() < 1.0e-6, "volume {v}");
}

#[test]
fn a_whole_number_parameter_takes_a_whole_value_and_refuses_a_fraction() {
    let linked = volume(
        "n = value.number@1(value=24)\n\
         ball = solid.sphere@1(radius=0.5, segments=n)\n\
         output ball",
    );
    let typed = volume("ball = solid.sphere@1(radius=0.5, segments=24)\noutput ball");
    assert!((linked - typed).abs() < 1.0e-12, "{linked} vs {typed}");
    let (code, suggestion) = refused(
        "n = value.number@1(value=24.5)\n\
         ball = solid.sphere@1(radius=0.5, segments=n)\n\
         output ball",
    );
    assert_eq!(code, "value.not_whole");
    assert!(suggestion.is_some());
}

#[test]
fn a_linked_value_is_checked_against_the_parameters_own_range() {
    let (code, _) = refused(
        "r = value.number@1(value=-1)\n\
         ball = solid.sphere@1(radius=r, segments=24)\n\
         output ball",
    );
    assert_eq!(code, "param.range");
}

#[test]
fn dividing_by_zero_is_refused_with_a_fix() {
    let (code, suggestion) = refused(
        "z = value.number@1(value=0)\n\
         q = value.math@1(op=\"div\", a=1, b=z)\n\
         box = solid.box@1(size=[1, 1, 1], size.x=q)\n\
         output box",
    );
    assert_eq!(code, "value.divide_by_zero");
    assert!(suggestion.unwrap().contains("change b"));
}

#[test]
fn a_value_that_is_not_a_number_cannot_fill_a_socket() {
    // A solid linked into a number parameter is refused when the link is made.
    let batch = parse_text(
        &builtin_registry(),
        "a = solid.box@1(size=[1, 1, 1])\nball = solid.sphere@1(radius=a, segments=24)\noutput ball",
    );
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_values_bad").unwrap(),
        label: None,
        commands: batch.unwrap().commands,
    });
    assert!(!matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    // And a number linked into a geometry input is refused too.
    let batch = parse_text(
        &builtin_registry(),
        "n = value.number@1(value=1)\nb = solid.box@1(size=[1, 1, 1])\nu = solid.union@1(n, b)\noutput u",
    );
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse("t_g11_values_bad2").unwrap(),
        label: None,
        commands: batch.unwrap().commands,
    });
    assert!(!matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
}

#[test]
fn the_text_round_trips_with_its_socket_links() {
    let program = "h = value.number@1(value=3)\n\
                   d = value.math@1(op=\"mul\", a=h, b=0.5)\n\
                   box = solid.box@1(size=[1, 1, 1], size.y=h, size.z=d)\n\
                   output box";
    let first = session(program);
    let printed = to_text(&builtin_registry(), first.document()).unwrap();
    assert!(printed.contains("size.y=h"), "{printed}");
    assert!(printed.contains("size.z=d"), "{printed}");
    assert!(printed.contains("a=h"), "{printed}");
    let again = volume(&printed);
    let original = evaluate_doc(first.document()).unwrap();
    // 1 x 3 x 1.5.
    assert!((original - 4.5).abs() < 1.0e-6, "volume {original}");
    assert!((again - original).abs() < 1.0e-12, "{again} vs {original}");
}

#[test]
fn changing_the_value_changes_the_part() {
    let program = |h: &str| {
        format!(
            "h = value.number@1(value={h})\nbox = solid.box@1(size=[1, 1, 1], size.y=h)\noutput box"
        )
    };
    let (two, five) = (volume(&program("2")), volume(&program("5")));
    assert!(
        (two - 2.0).abs() < 1.0e-6 && (five - 5.0).abs() < 1.0e-6,
        "{two} {five}"
    );
}

#[test]
fn an_unknown_port_names_the_sockets_that_exist() {
    let errors = parse_text(
        &builtin_registry(),
        "n = value.number@1(value=1)\nbox = solid.box@1(size=[1, 1, 1], height=n)\noutput box",
    )
    .unwrap_err();
    let message = errors[0].suggestion.clone().unwrap_or_default();
    assert!(
        message.contains("size.x") && message.contains("size.y"),
        "{message}"
    );
}
