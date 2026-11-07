//! `value.point`: one coordinate of a bone's head or tail, or of the min, centre or max of a part's bounding box, as a number that links into any number
//! parameter (`by.y=hy`, `pivot.y=hy`). A part that is not a bone has no head or tail. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality, set_human_pack};

fn evaluate(source: &str) -> Result<Evaluation, (String, Option<String>)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_value_point").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.suggestion.clone()))
}

fn part<'a>(result: &'a Evaluation, name: &str) -> &'a ve_modeling::Part {
    result.parts.iter().find(|p| p.name == name).unwrap()
}

fn centre(part: &ve_modeling::Part) -> [f32; 3] {
    let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for p in &part.positions {
        for axis in 0..3 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    }
    [0, 1, 2].map(|a| (low[a] + high[a]) * 0.5)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

const ARM: &str = "body = body.human@1()\nbone = body.bone@1(body, bone=\"upperarm01_l\")\n";

#[test]
fn a_bones_head_and_tail_place_another_part() {
    let source = format!(
        "{ARM}hx = value.point@1(bone, at=\"head\", axis=\"x\")\nhy = value.point@1(bone, at=\"head\", axis=\"y\")\nhz = value.point@1(bone, at=\"head\", axis=\"z\")\ntx = value.point@1(bone, at=\"tail\", axis=\"x\")\nb = solid.box@1(size=[0.1, 0.1, 0.1])\nat_head = xform.move@1(b, by=[0, 0, 0], by.x=hx, by.y=hy, by.z=hz)\nat_tail = xform.move@1(b, by=[0, 0, 0], by.x=tx)\noutput arm = bone\noutput marker_head = at_head\noutput marker_tail = at_tail\n"
    );
    let result = evaluate(&source).unwrap();
    let segment = part(&result, "arm").bone.unwrap();
    let head = centre(part(&result, "marker_head"));
    for axis in 0..3 {
        assert!(
            near(head[axis], segment.head[axis]),
            "axis {axis}: {head:?} vs {:?}",
            segment.head
        );
    }
    assert!(near(
        centre(part(&result, "marker_tail"))[0],
        segment.tail[0]
    ));
    assert!(
        !near(segment.head[0], segment.tail[0]),
        "head and tail differ on x"
    );
}

#[test]
fn bounding_box_places_of_a_known_box_are_exact() {
    let reads = [
        ("min", "y", -2.0_f32),
        ("center", "y", 0.0),
        ("max", "y", 2.0),
        ("min", "x", -1.0),
        ("max", "z", 3.0),
    ];
    for (at, axis, expected) in reads {
        let source = format!(
            "b = solid.box@1(size=[2, 4, 6])\np = value.point@1(b, at=\"{at}\", axis=\"{axis}\")\nm = solid.box@1(size=[0.2, 0.2, 0.2])\nmoved = xform.move@1(m, by=[0, 0, 0], by.{axis}=p)\noutput o = moved\n"
        );
        let result = evaluate(&source).unwrap();
        let got = centre(part(&result, "o"));
        let index = usize::from(axis == "y") + 2 * usize::from(axis == "z");
        assert!(
            near(got[index], expected),
            "{at} {axis}: {got:?} wanted {expected}"
        );
    }
}

#[test]
fn a_part_that_is_not_a_bone_has_no_head_or_tail() {
    let (code, suggestion) = evaluate(
        "b = solid.box@1(size=[1, 1, 1])\np = value.point@1(b, at=\"head\", axis=\"y\")\nm = solid.box@1(size=[1, 1, 1])\nmoved = xform.move@1(m, by=[0, 0, 0], by.y=p)\noutput o = moved\n",
    )
    .unwrap_err();
    assert_eq!(code, "value.not_a_bone");
    assert!(suggestion.unwrap().contains("min, center or max"));
}

#[test]
fn a_bone_carried_through_a_parent_and_a_track_keeps_its_head() {
    let source = format!(
        "{ARM}fore = body.bone@1(body, bone=\"lowerarm01_l\")\nfp = anim.parent@1(fore, parent=\"arm\")\nft = anim.track@1(fp, property=\"rotation\", keys=\"0:0,0,0; 1:-20,0,0\", pivot_at=\"head\")\nhy = value.point@1(ft, at=\"head\", axis=\"y\")\nb = solid.box@1(size=[0.1, 0.1, 0.1])\nmarker = xform.move@1(b, by=[0, 0, 0], by.y=hy)\noutput arm = bone\noutput fore = ft\noutput marker = marker\n"
    );
    let result = evaluate(&source).unwrap();
    let head_y = part(&result, "fore").bone.unwrap().head[1];
    assert!(near(centre(part(&result, "marker"))[1], head_y));
}

#[test]
fn a_value_point_links_into_a_pivot_component() {
    let source = format!(
        "{ARM}hy = value.point@1(bone, at=\"head\", axis=\"y\")\nt = anim.track@1(bone, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,30\", pivot=[0, 0, 0], pivot.y=hy)\noutput arm = t\n"
    );
    let result = evaluate(&source).unwrap();
    let arm = part(&result, "arm");
    assert!(
        near(arm.animation[0].pivot[1], arm.bone.unwrap().head[1]),
        "{:?}",
        arm.animation[0].pivot
    );
    assert_eq!(arm.animation[0].pivot[0], 0.0);
}

#[test]
fn value_math_can_offset_a_point() {
    let source = "b = solid.box@1(size=[2, 4, 6])\ntop = value.point@1(b, at=\"max\", axis=\"y\")\nabove = value.math@1(op=\"add\", a=top, b=0.5)\nm = solid.box@1(size=[0.2, 0.2, 0.2])\nmoved = xform.move@1(m, by=[0, 0, 0], by.y=above)\noutput o = moved\n";
    let result = evaluate(source).unwrap();
    assert!(near(centre(part(&result, "o"))[1], 2.5));
}
