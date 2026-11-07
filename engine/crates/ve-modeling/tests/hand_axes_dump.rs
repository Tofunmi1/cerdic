//! Prints each hand's finger-curl axis (the line across the knuckles, index to little finger) and the Euler angles (x, y, z in degrees, the order the
//! rotation tracks use) of a turn of 60, 70 and 90 degrees about it, so the autorig's hand poses curl the fingers toward the palm. Run with --nocapture.
#![expect(
    clippy::unwrap_used,
    clippy::print_stderr,
    clippy::panic,
    reason = "fixture"
)]
use glam::{EulerRot, Quat, Vec3};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

#[test]
fn dump() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    ve_modeling::set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let mut text = "body = body.human@1()\n".to_owned();
    for side in ["l", "r"] {
        for bone in [
            "finger2_1",
            "finger5_1",
            "finger3_1",
            "finger3_3",
            "wrist",
            "finger1_1",
        ] {
            text.push_str(&format!("b_{bone}_{side} = body.bone@1(body, bone=\"{bone}_{side}\")\noutput {bone}_{side} = b_{bone}_{side}\n"));
        }
    }
    let commands = parse_text(&builtin_registry(), &text)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_axes").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }));
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    let seg = |name: &str| {
        result
            .parts
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .bone
            .unwrap()
    };
    for side in ["l", "r"] {
        let v = |a: [f32; 3]| Vec3::from(a);
        let across = (v(seg(&format!("finger5_1_{side}")).head)
            - v(seg(&format!("finger2_1_{side}")).head))
        .normalize();
        let finger = (v(seg(&format!("finger3_3_{side}")).tail)
            - v(seg(&format!("finger3_1_{side}")).head))
        .normalize();
        let thumb_side = (v(seg(&format!("finger1_1_{side}")).head)
            - v(seg(&format!("wrist_{side}")).head))
        .normalize();
        eprintln!(
            "AXIS {side}: across {across:?} finger {finger:?} thumb-from-wrist {thumb_side:?}"
        );
        for degrees in [60.0_f32, 70.0, 90.0] {
            let q = Quat::from_axis_angle(across, degrees.to_radians());
            let (y, x, z) = q.to_euler(EulerRot::YXZ);
            eprintln!(
                "EULER {side} {degrees}: {:.2},{:.2},{:.2}",
                x.to_degrees(),
                y.to_degrees(),
                z.to_degrees()
            );
        }
    }
}
