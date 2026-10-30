//! Additional contract coverage for selectors, timers, and ordered rules.
use ve_game::{GameDef, HostEvent, Inputs, World};

#[test]
fn host_selectors_drive_velocity_and_property_actions() {
    let definition = GameDef::parse(
        "game name=selectors\nvar count=0\nkind crate size=0.2 tags=item\nspawn crate at=0,0,0\nrule on=host:grab a=tag:item do=setvel:a:1,0,0;setprop:a:held:1;add:count:1\n",
    )
    .unwrap();
    let mut world = World::new(&definition, 2);
    let id = world.entities()[0].id;
    world
        .step(&Inputs {
            host: vec![HostEvent {
                name: "grab".into(),
                a: Some(id),
                b: None,
            }],
            ..Inputs::default()
        })
        .unwrap();
    assert!((world.vars()["count"] - 1.0).abs() < f64::EPSILON);
    assert!((world.entities()[0].props["held"] - 1.0).abs() < f64::EPSILON);
    assert!(world.entities()[0].velocity[0] > 0.0);
}

#[test]
fn restoring_a_pending_timer_replays_it_at_the_same_tick() {
    let definition = GameDef::parse(
        "game name=timer\nvar fired=0\nrule on=start do=timer:bell:1.0\nrule on=timer:bell do=add:fired:1\n",
    )
    .unwrap();
    let mut world = World::new(&definition, 3);
    world.step(&Inputs::default()).unwrap();
    let saved = world.snapshot();
    for _ in 0..60 {
        world.step(&Inputs::default()).unwrap();
    }
    assert!((world.vars()["fired"] - 1.0).abs() < f64::EPSILON);
    world.restore(&saved).unwrap();
    for _ in 0..60 {
        world.step(&Inputs::default()).unwrap();
    }
    assert!((world.vars()["fired"] - 1.0).abs() < f64::EPSILON);
}

/// A door that slides up when a socket is filled is `move:kind:door:0,2.6,0`: the selector has a colon of its own, so the vector is what
/// follows the last colon (as `setvel` already reads it). Written before the fix; do not weaken.
#[test]
fn move_accepts_kind_and_tag_selectors_that_contain_a_colon() {
    let definition = GameDef::parse(
        "game name=doors\nvar n=0\nkind door size=0.5 tags=gate\nkind lamp size=0.1\nspawn door at=0,1,0\nspawn lamp at=5,1,0\nrule on=start do=move:kind:door:0,2.5,0;move:tag:gate:1,0,0\n",
    )
    .unwrap();
    let mut world = World::new(&definition, 1);
    world.step(&Inputs::default()).unwrap();
    let position = |kind: &str| {
        world
            .entities()
            .iter()
            .find(|e| e.kind == kind)
            .unwrap()
            .position
    };
    assert_eq!(
        position("door"),
        [1.0, 3.5, 0.0],
        "both moves reached the door"
    );
    assert_eq!(position("lamp"), [5.0, 1.0, 0.0], "and nothing else moved");
}

#[test]
fn rules_for_one_event_run_in_declaration_order() {
    let definition = GameDef::parse(
        "game name=order\nvar score=0\nrule on=start do=set:score:2\nrule on=start do=add:score:3\n",
    )
    .unwrap();
    let mut world = World::new(&definition, 4);
    world.step(&Inputs::default()).unwrap();
    assert!((world.vars()["score"] - 5.0).abs() < f64::EPSILON);
}
