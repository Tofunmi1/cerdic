//! `phys.body`: the properties reach the part, and bodies that cannot work are dropped with a reason.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::physics::{BodyKind, BodyShape};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_phys").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { .. } = outcome {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{}: {}", e.code, e.message))
}

#[test]
fn a_body_carries_its_properties_and_defaults() {
    let e = evaluate("b = solid.sphere@1(radius=0.3)\nball = phys.body@1(b, restitution=0.8, density=2)\noutput ball = ball\n").unwrap();
    let body = e.parts[0].body.clone().unwrap();
    assert_eq!(
        (body.kind, body.shape),
        (BodyKind::Dynamic, BodyShape::Auto)
    );
    assert!((body.restitution - 0.8).abs() < 1e-12 && (body.density - 2.0).abs() < 1e-12);
    assert!((body.friction - 0.6).abs() < 1e-12 && body.linear_damping == 0.0);
    let floor = evaluate("f = solid.box@1(size=[10, 0.2, 10])\nfloor = phys.body@1(f, kind=\"static\", shape=\"mesh\")\noutput floor = floor\n").unwrap();
    assert_eq!(floor.parts[0].body.clone().unwrap().kind, BodyKind::Static);
    assert!(e.warnings.is_empty(), "{:?}", e.warnings);
}

#[test]
fn a_joint_is_kept_on_its_body_and_a_joint_to_nowhere_is_dropped() {
    let hinged = evaluate(
        "post = solid.box@1(size=[0.2, 2, 0.2])\npost_body = phys.body@1(post, kind=\"static\")\noutput post = post_body\n\
         door = solid.box@1(size=[1, 1.8, 0.05])\ndoor_at = xform.move@1(door, by=[0.6, 0, 0])\ndoor_body = phys.body@1(door_at, shape=\"box\")\n\
         swing = phys.joint@1(door_body, kind=\"hinge\", to=\"post\", anchor=[0.1, 0, 0], axis=[0, 1, 0], limited=true, low=0, high=100)\noutput door = swing\n",
    )
    .unwrap();
    let door = hinged
        .parts
        .iter()
        .find(|p| p.name == "door")
        .unwrap()
        .body
        .clone()
        .unwrap();
    assert_eq!(door.joints.len(), 1);
    assert_eq!(
        (
            door.joints[0].to.as_str(),
            door.joints[0].limited,
            door.joints[0].high
        ),
        ("post", true, 100.0)
    );
    assert!(hinged.warnings.is_empty(), "{:?}", hinged.warnings);
    let lost = evaluate("b = solid.box@1(size=[1, 1, 1])\nbody = phys.body@1(b)\nj = phys.joint@1(body, to=\"ghost\", anchor=[0, 0, 0])\noutput b = j\n").unwrap();
    assert!(lost.parts[0].body.as_ref().unwrap().joints.is_empty());
    assert!(
        lost.warnings.iter().any(|w| w.code == "phys.joint_dropped"),
        "{:?}",
        lost.warnings
    );
    let early = evaluate(
        "b = solid.box@1(size=[1, 1, 1])\nj = phys.joint@1(b, anchor=[0, 0, 0])\noutput b = j\n",
    )
    .unwrap_err();
    assert!(early.contains("after phys.body"), "{early}");
    let flat = evaluate(
        "b = solid.box@1(size=[1, 1, 1])\nbody = phys.body@1(b)\nj = phys.joint@1(body, kind=\"hinge\", axis=[0, 0, 0])\noutput b = j\n",
    );
    assert!(flat.is_err(), "a hinge needs an axis");
}

#[test]
fn bad_bodies_are_refused_or_dropped_with_a_reason() {
    let moving_mesh = evaluate(
        "b = solid.box@1(size=[1, 1, 1])\np = phys.body@1(b, shape=\"mesh\")\noutput p = p\n",
    )
    .unwrap_err();
    assert!(moving_mesh.contains("static"), "{moving_mesh}");
    let twice = evaluate(
        "b = solid.box@1(size=[1, 1, 1])\na = phys.body@1(b)\nc = phys.body@1(a)\noutput p = c\n",
    )
    .unwrap_err();
    assert!(twice.contains("only one"), "{twice}");
    let animated = evaluate("b = solid.box@1(size=[1, 1, 1])\nm = anim.track@1(b, property=\"position\", keys=\"0:0,0,0; 1:1,0,0\")\np = phys.body@1(m)\noutput p = p\n").unwrap();
    assert!(animated.parts[0].body.is_none());
    assert!(
        animated
            .warnings
            .iter()
            .any(|w| w.code == "phys.body_dropped"),
        "{:?}",
        animated.warnings
    );
    let set = evaluate(
        "b = solid.box@1(size=[0.1, 0.1, 0.1])\ns = inst.repeat@1(b, count=3, step=[1, 0, 0])\np = phys.body@1(s)\noutput p = p\n",
    );
    assert!(
        set.is_err() || set.unwrap().parts.iter().all(|p| p.body.is_none()),
        "an instance set cannot be a body"
    );
}
