//! `fx.emitter`, `fx.force`, `fx.collide`, `fx.trail`, `fx.sub`: effects authored in the language and carried by the part they are put on
//! (plan W4). Written before the bodies.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};
use ve_particles::fx::{Blend, Force, Reaction, Render, Shape, SubTrigger};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_fx").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { .. } = outcome {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| e.message)
}

/// Evaluates `lines` after a small source shape and returns the effect on the output named `out`.
fn effect(lines: &str) -> ve_modeling::fx::PartFx {
    let text = format!("s = solid.sphere@1(radius=0.1)\n{lines}\noutput out = last\n");
    let evaluation = evaluate(&text).unwrap();
    evaluation
        .parts
        .iter()
        .find(|p| p.name == "out")
        .unwrap()
        .fx
        .clone()
        .expect("the part carries an effect")
}

#[test]
fn an_emitter_puts_an_effect_on_the_shape() {
    let fx = effect(
        "last = fx.emitter@1(s, name=\"smoke\", rate=30, lifetime=[1.5, 2.5], speed=[0.5, 1.2], direction=[0, 1, 0], cone=20, \
         shape=\"sphere\", extent=[0.2, 0.2, 0.2], max_particles=800, size=[0.2, 1.2], colour=[0.5, 0.5, 0.55], \
         colour_end=[0.2, 0.2, 0.2], alpha=[0.8, 0], glow=0, blend=\"alpha\")",
    );
    assert_eq!(fx.names, ["smoke"]);
    let e = &fx.effect.emitters[0];
    assert!((e.spawn.rate - 30.0).abs() < 1e-5);
    assert_eq!(e.lifetime, [1.5, 2.5]);
    assert_eq!(e.speed, [0.5, 1.2]);
    assert!(
        (e.cone - 20.0_f32.to_radians()).abs() < 1e-5,
        "angles are degrees in the language: {}",
        e.cone
    );
    assert_eq!(e.shape, Shape::Sphere { radius: 0.2 });
    assert_eq!(e.max_particles, 800);
    assert_eq!(e.look.size, [0.2, 1.2]);
    assert_eq!(e.look.colour, [[0.5, 0.5, 0.55], [0.2, 0.2, 0.2]]);
    assert_eq!(e.look.alpha, [0.8, 0.0]);
    assert_eq!(e.look.blend, Blend::Alpha);
    assert_eq!(e.look.render, Render::Sprite);
    assert!(fx.effect.validate().is_ok());
    assert_eq!(fx.ground, None);
}

#[test]
fn cube_particles_and_their_spin_reach_the_look() {
    let fx = effect("last = fx.emitter@1(s, render=\"cube\", spin=7, size=[0.1, 0.05])");
    let look = fx.effect.emitters[0].look;
    assert_eq!(look.render, Render::Cube);
    assert_eq!(look.spin, 7.0);
    assert_eq!(
        effect("last = fx.emitter@1(s)").effect.emitters[0]
            .look
            .render,
        Render::Sprite
    );
}

#[test]
fn bursts_delay_duration_and_loop_reach_the_spawn_settings() {
    let fx = effect(
        "last = fx.emitter@1(s, rate=0, burst=40, burst_at=0.5, delay=1, duration=2, loop=true)",
    );
    let spawn = &fx.effect.emitters[0].spawn;
    assert_eq!(spawn.bursts.len(), 1);
    assert_eq!((spawn.bursts[0].count, spawn.bursts[0].time), (40, 0.5));
    assert_eq!(
        (spawn.delay, spawn.duration, spawn.looping),
        (1.0, 2.0, true)
    );
    // No burst asked for, none made.
    let plain = effect("last = fx.emitter@1(s)");
    assert!(plain.effect.emitters[0].spawn.bursts.is_empty());
    assert_eq!(
        plain.names,
        ["e0"],
        "an unnamed emitter is called e0, e1, ..."
    );
}

#[test]
fn each_shape_maps_its_extent() {
    let case = |shape: &str| {
        effect(&format!(
            "last = fx.emitter@1(s, shape=\"{shape}\", extent=[0.3, 0.4, 0.5])"
        ))
        .effect
        .emitters[0]
            .shape
    };
    assert_eq!(case("point"), Shape::Point);
    assert_eq!(case("sphere"), Shape::Sphere { radius: 0.3 });
    assert_eq!(case("disc"), Shape::Disc { radius: 0.3 });
    assert_eq!(
        case("box"),
        Shape::Box {
            half_extents: [0.3, 0.4, 0.5]
        }
    );
    assert_eq!(
        case("line"),
        Shape::Line { length: 0.8 },
        "a line is twice the vertical half-extent long"
    );
}

#[test]
fn emitters_chain_and_forces_go_to_the_named_emitter_or_the_last() {
    let fx = effect(
        "a = fx.emitter@1(s, name=\"fire\")\nb = fx.emitter@1(a, name=\"smoke\")\n\
         c = fx.force@1(b, emitter=\"fire\", kind=\"gravity\")\n\
         d = fx.force@1(c, kind=\"wind\", vector=[2, 0, 0], strength=1.5)\n\
         e = fx.force@1(d, kind=\"drag\", strength=0.8)\n\
         f = fx.force@1(e, kind=\"vortex\", centre=[0, 0, 0], vector=[0, 1, 0], strength=3, radius=4)\n\
         g = fx.force@1(f, kind=\"attractor\", centre=[1, 2, 3], strength=9, radius=6)\n\
         last = fx.force@1(g, kind=\"turbulence\", strength=2, scale=0.7, speed=0.4)",
    );
    assert_eq!(fx.names, ["fire", "smoke"]);
    assert_eq!(fx.effect.emitters.len(), 2);
    assert_eq!(
        fx.effect.emitters[0].forces,
        [Force::Gravity([0.0, -9.8, 0.0])],
        "a gravity with no vector is earth's"
    );
    assert_eq!(
        fx.effect.emitters[1].forces,
        [
            Force::Wind {
                velocity: [2.0, 0.0, 0.0],
                strength: 1.5
            },
            Force::Drag(0.8),
            Force::Vortex {
                centre: [0.0, 0.0, 0.0],
                axis: [0.0, 1.0, 0.0],
                strength: 3.0,
                radius: 4.0
            },
            Force::Attractor {
                centre: [1.0, 2.0, 3.0],
                strength: 9.0,
                radius: 6.0
            },
            Force::Turbulence {
                strength: 2.0,
                scale: 0.7,
                speed: 0.4
            },
        ]
    );
}

#[test]
fn collide_trail_and_sub_configure_the_emitters() {
    let fx = effect(
        "a = fx.emitter@1(s, name=\"spark\", rate=0, burst=5)\nb = fx.emitter@1(a, name=\"flash\", rate=0)\n\
         c = fx.collide@1(b, emitter=\"spark\", reaction=\"bounce\", radius=0.03, restitution=0.5, friction=0.2, ground=1.5)\n\
         d = fx.trail@1(c, emitter=\"spark\", points=10, min_step=0.04)\n\
         last = fx.sub@1(d, parent=\"spark\", child=\"flash\", on=\"collide\", count=2)",
    );
    let spark = &fx.effect.emitters[0];
    let collide = spark.collide.unwrap();
    assert_eq!(collide.radius, 0.03);
    assert_eq!(
        collide.reaction,
        Reaction::Bounce {
            restitution: 0.5,
            friction: 0.2
        }
    );
    assert_eq!(fx.ground, Some(1.5));
    let trail = spark.trail.unwrap();
    assert_eq!((trail.points, trail.min_step), (10, 0.04));
    assert_eq!(spark.sub.len(), 1);
    assert_eq!(
        (spark.sub[0].on, spark.sub[0].emitter, spark.sub[0].count),
        (SubTrigger::Collide, 1, 2)
    );
    assert!(fx.effect.validate().is_ok());
    // The other reactions.
    let stick = effect("a = fx.emitter@1(s)\nlast = fx.collide@1(a, reaction=\"stick\")");
    assert_eq!(
        stick.effect.emitters[0].collide.unwrap().reaction,
        Reaction::Stick
    );
    let kill = effect("a = fx.emitter@1(s)\nlast = fx.collide@1(a, reaction=\"kill\")");
    assert_eq!(
        kill.effect.emitters[0].collide.unwrap().reaction,
        Reaction::Kill
    );
}

#[test]
fn an_effect_is_part_of_the_evaluated_part_and_serialises_with_it() {
    let text =
        "s = solid.sphere@1(radius=0.1)\ne = fx.emitter@1(s, name=\"smoke\")\noutput torch = e\n";
    let evaluation = evaluate(text).unwrap();
    let json = serde_json::to_value(&evaluation.parts[0]).unwrap();
    let fx = &json["fx"];
    assert_eq!(fx["names"][0], "smoke");
    assert!(
        fx["effect"]["emitters"][0]["spawn"]["rate"].is_number(),
        "{fx}"
    );
    // A part without an effect has none.
    let none = evaluate("s = solid.sphere@1(radius=0.1)\noutput plain = s\n").unwrap();
    assert!(none.parts[0].fx.is_none());
}

#[test]
fn mistakes_are_refused_and_say_how_to_fix_them() {
    let refuse = |lines: &str| {
        evaluate(&format!(
            "s = solid.sphere@1(radius=0.1)\n{lines}\noutput out = last\n"
        ))
        .unwrap_err()
    };
    let message = refuse("last = fx.force@1(s, kind=\"wind\")");
    assert!(
        message.contains("fx.emitter"),
        "a force needs an emitter first: {message}"
    );
    let message = refuse(
        "a = fx.emitter@1(s, name=\"fire\")\nlast = fx.force@1(a, emitter=\"flame\", kind=\"drag\")",
    );
    assert!(
        message.contains("fire"),
        "an unknown emitter lists the known ones: {message}"
    );
    let message = refuse("a = fx.emitter@1(s, name=\"x\")\nlast = fx.emitter@1(a, name=\"x\")");
    assert!(
        message.contains('x') && message.contains("name"),
        "duplicate names: {message}"
    );
    let message = refuse("last = fx.emitter@1(s, lifetime=[3, 1])");
    assert!(
        message.contains("lifetime"),
        "an effect that cannot run is refused when it is made: {message}"
    );
    let message = refuse(
        "a = fx.emitter@1(s, name=\"a\")\nb = fx.emitter@1(a, name=\"b\")\nc = fx.sub@1(b, parent=\"a\", child=\"b\", on=\"death\")\nlast = fx.sub@1(c, parent=\"b\", child=\"a\", on=\"death\")",
    );
    assert!(
        message.contains("sub"),
        "sub-emitters that spawn each other in a loop: {message}"
    );
}

#[test]
fn a_ribbon_without_a_trail_warns_with_the_fix_and_the_effect_still_runs() {
    // Refining nodes may come before the trail, so this is a warning on the result, not a refusal.
    let text = "s = solid.sphere@1(radius=0.1)\na = fx.emitter@1(s, name=\"sparks\", render=\"ribbon\")\nb = fx.collide@1(a)\noutput out = b\n";
    let evaluation = evaluate(text).unwrap();
    assert!(evaluation.parts[0].fx.is_some());
    let warning = evaluation
        .warnings
        .iter()
        .find(|w| w.code == "fx.ribbon_without_trail")
        .unwrap();
    assert!(
        warning.message.contains("fx.trail") && warning.message.contains("sparks"),
        "{}",
        warning.message
    );
    let fixed = format!(
        "{}t = fx.trail@1(b, emitter=\"sparks\")\n",
        text.replace("output out = b\n", "")
    );
    let evaluation = evaluate(&format!("{fixed}output out = t\n")).unwrap();
    assert!(
        evaluation
            .warnings
            .iter()
            .all(|w| w.code != "fx.ribbon_without_trail")
    );
}

#[test]
fn parts_beyond_the_budget_lose_their_effect_with_a_warning() {
    let mut text = String::from("s = solid.sphere@1(radius=0.1)\n");
    for i in 0..66 {
        text.push_str(&format!("e{i} = fx.emitter@1(s)\noutput p{i} = e{i}\n"));
    }
    let evaluation = evaluate(&text).unwrap();
    let with = evaluation.parts.iter().filter(|p| p.fx.is_some()).count();
    assert_eq!(with, 64, "the scene keeps 64 emitting parts");
    assert_eq!(
        evaluation
            .warnings
            .iter()
            .filter(|w| w.code == "fx.budget")
            .count(),
        2
    );
}

#[test]
fn an_effect_survives_material_motion_and_parenting_nodes_after_it() {
    // Appearance and motion nodes keep what is attached (geometry nodes such as xform.* rebuild the shape from the kernel solid and, like
    // rig.ik and anim.*, do not: put fx.* after them).
    let text = "s = solid.sphere@1(radius=0.1)\nmv = xform.move@1(s, by=[0, 1, 0])\ne = fx.emitter@1(mv)\nm = solid.material@1(e, color=[1, 0.4, 0.1])\n\
                a = anim.track@1(m, property=\"position\", keys=\"0:0,0,0; 1:1,0,0\")\np = anim.parent@1(a, parent=\"body\")\noutput out = p\n";
    let evaluation = evaluate(text).unwrap();
    assert!(evaluation.parts[0].fx.is_some());
}
