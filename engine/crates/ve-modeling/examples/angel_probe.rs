//! Scratch probe for the angel: body bounds and joint positions after body.human places the feet on the ground.
#![expect(clippy::print_stdout, clippy::unwrap_used, reason = "scratch tool")]

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality, set_human_pack};

fn main() {
    let pack = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(pack).unwrap()).unwrap();
    let bones = [
        "root",
        "spine05",
        "spine01",
        "clavicle_l",
        "shoulder01_l",
        "upperarm01_l",
        "lowerarm01_l",
        "wrist_l",
        "neck01",
        "neck03",
        "head",
        "upperleg01_l",
        "lowerleg01_l",
        "foot_l",
    ];
    let mut text = String::from(
        "body = body.human@1(gender=1, age=0.667, height=0.53, weight=0.42, muscle=0.45, proportions=0.55)\noutput body_part = body\n",
    );
    for name in bones {
        text.push_str(&format!(
            "b_{name} = body.bone@1(body, bone=\"{name}\")\noutput {name} = b_{name}\n"
        ));
    }
    let commands = parse_text(&builtin_registry(), &text).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_angel_probe").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let evaluation = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    let body = evaluation
        .parts
        .iter()
        .find(|p| p.name == "body_part")
        .unwrap();
    let (lo, hi) = body
        .positions
        .iter()
        .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
            (
                std::array::from_fn(|i| lo[i].min(p[i])),
                std::array::from_fn(|i| hi[i].max(p[i])),
            )
        });
    println!("body bounds {lo:?} {hi:?} height {}", hi[1] - lo[1]);
    for (label, y) in [
        ("hip", 0.98),
        ("waist", 1.10),
        ("ribs", 1.22),
        ("bust", 1.32),
        ("chest", 1.40),
        ("neck", 1.52),
    ] {
        let ring: Vec<[f32; 3]> = body
            .positions
            .iter()
            .copied()
            .filter(|p| (p[1] - y).abs() < 0.012 && p[0].abs() < 0.3)
            .collect();
        if !ring.is_empty() {
            let lo = ring.iter().fold([f32::MAX; 3], |lo, p| {
                std::array::from_fn(|i| lo[i].min(p[i]))
            });
            let hi = ring.iter().fold([f32::MIN; 3], |hi, p| {
                std::array::from_fn(|i| hi[i].max(p[i]))
            });
            println!(
                "{label} y={y}: x {:?}..{:?} z {:?}..{:?}",
                lo[0], hi[0], lo[2], hi[2]
            );
        }
    }
    for out in evaluation
        .parts
        .iter()
        .filter(|p| p.bone.is_some() && p.name != "body_part")
    {
        let bone = out.bone.unwrap();
        println!("{} head={:?} tail={:?}", out.name, bone.head, bone.tail);
    }
}
