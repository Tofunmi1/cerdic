//! Scripted flows for the interaction state machine, run against a tiny world of spheres.

mod common;

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use ve_interact::{Event, Interaction, Placement3, PointerFrame, PointerId};

use common::{BALL, LEFT, MOUSE, OTHER, RIGHT, Scene, controller, mouse, step};

#[test]
fn hover_events_follow_the_ray_on_and_off_a_target() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    let on = step(&mut machine, &mut scene, &[mouse(0.0, 0.0, false)]);
    assert_eq!(
        on.events,
        [Event::HoverStart {
            pointer: MOUSE,
            target: BALL
        }]
    );
    assert!(
        step(&mut machine, &mut scene, &[mouse(0.1, 0.0, false)])
            .events
            .is_empty()
    );
    let off = step(&mut machine, &mut scene, &[mouse(3.0, 0.0, false)]);
    assert_eq!(
        off.events,
        [Event::HoverEnd {
            pointer: MOUSE,
            target: BALL
        }]
    );
    assert_eq!(machine.hover_of(MOUSE), None);
}

#[test]
fn a_press_and_release_on_a_target_is_a_click_and_a_missed_release_is_not() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    // A controller trigger (no grab) pressed and released on the ball.
    let on = |trigger| {
        controller(
            LEFT,
            Vec3::new(0.0, 0.0, 2.0),
            Quat::IDENTITY,
            false,
            trigger,
        )
    };
    step(&mut machine, &mut scene, &[on(false)]);
    let down = step(&mut machine, &mut scene, &[on(true)]);
    assert!(down.events.contains(&Event::SelectStart {
        pointer: LEFT,
        target: BALL
    }));
    let up = step(&mut machine, &mut scene, &[on(false)]);
    assert!(up.events.contains(&Event::SelectEnd {
        pointer: LEFT,
        target: BALL,
        click: true
    }));

    // Pressed on the ball, then aimed away before releasing.
    step(&mut machine, &mut scene, &[on(true)]);
    let aimed_away = controller(LEFT, Vec3::new(3.0, 0.0, 2.0), Quat::IDENTITY, false, false);
    let up = step(&mut machine, &mut scene, &[aimed_away]);
    assert!(up.events.contains(&Event::SelectEnd {
        pointer: LEFT,
        target: BALL,
        click: false
    }));
}

#[test]
fn grabbing_with_a_controller_does_not_jump_and_then_follows_the_hand_rigidly() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    let at = |position: Vec3, rotation: Quat, squeeze: bool| {
        controller(LEFT, position, rotation, squeeze, false)
    };
    let start = Vec3::new(0.0, 0.0, 2.0);
    step(
        &mut machine,
        &mut scene,
        &[at(start, Quat::IDENTITY, false)],
    );
    let grab = step(&mut machine, &mut scene, &[at(start, Quat::IDENTITY, true)]);
    assert!(grab.events.contains(&Event::GrabStart {
        pointer: LEFT,
        target: BALL
    }));
    assert!(
        scene
            .placement_of(BALL)
            .approx_eq(&Placement3::IDENTITY, 1e-6),
        "the object jumped on grab"
    );

    step(
        &mut machine,
        &mut scene,
        &[at(start + Vec3::X, Quat::IDENTITY, true)],
    );
    assert!(
        scene
            .placement_of(BALL)
            .translation
            .abs_diff_eq(Vec3::X, 1e-5)
    );

    // The hand turns a quarter about y at its own position: the ball, two metres in front of it,
    // swings round to its side.
    step(
        &mut machine,
        &mut scene,
        &[at(start, Quat::from_rotation_y(FRAC_PI_2), true)],
    );
    let ball = scene.placement_of(BALL);
    assert!(
        ball.translation
            .abs_diff_eq(Vec3::new(-2.0, 0.0, 2.0), 1e-4),
        "{}",
        ball.translation
    );
    assert!(
        ball.rotation
            .abs_diff_eq(Quat::from_rotation_y(FRAC_PI_2), 1e-4)
    );

    let release = step(
        &mut machine,
        &mut scene,
        &[at(start, Quat::from_rotation_y(FRAC_PI_2), false)],
    );
    assert!(matches!(
        release.events.last(),
        Some(Event::GrabEnd { target: BALL, .. })
    ));
    assert!(machine.held_targets().is_empty());
}

#[test]
fn dragging_with_the_mouse_moves_the_object_with_the_cursor_and_is_not_a_click() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    step(&mut machine, &mut scene, &[mouse(0.0, 0.0, false)]);
    step(&mut machine, &mut scene, &[mouse(0.0, 0.0, true)]);
    assert_eq!(machine.holders_of(BALL), [MOUSE]);
    step(&mut machine, &mut scene, &[mouse(0.4, 0.2, true)]);
    step(&mut machine, &mut scene, &[mouse(1.0, 0.0, true)]);
    assert!(
        scene
            .placement_of(BALL)
            .translation
            .abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-5)
    );
    let release = step(&mut machine, &mut scene, &[mouse(1.0, 0.0, false)]);
    assert!(release.events.contains(&Event::SelectEnd {
        pointer: MOUSE,
        target: BALL,
        click: false
    }));
    assert!(machine.held_targets().is_empty());
}

#[test]
fn a_mouse_press_without_movement_is_a_click_and_the_wheel_pushes_and_pulls() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    step(&mut machine, &mut scene, &[mouse(0.0, 0.0, false)]);
    step(&mut machine, &mut scene, &[mouse(0.0, 0.0, true)]);
    let up = step(&mut machine, &mut scene, &[mouse(0.0, 0.0, false)]);
    assert!(up.events.contains(&Event::SelectEnd {
        pointer: MOUSE,
        target: BALL,
        click: true
    }));

    // Press again and push the object away along the ray with the wheel.
    step(&mut machine, &mut scene, &[mouse(0.0, 0.0, true)]);
    let pushed = PointerFrame {
        reach: 1.5,
        ..mouse(0.0, 0.0, true)
    };
    step(&mut machine, &mut scene, &[pushed]);
    assert!(
        scene
            .placement_of(BALL)
            .translation
            .abs_diff_eq(Vec3::new(0.0, 0.0, -1.5), 1e-5)
    );
}

#[test]
fn two_hands_scale_the_object_and_letting_go_of_one_does_not_make_it_jump() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    let hands = |spread: f32, left_squeeze: bool, right_squeeze: bool| {
        [
            controller(
                LEFT,
                Vec3::new(-spread, 0.0, 2.0),
                Quat::IDENTITY,
                left_squeeze,
                false,
            ),
            controller(
                RIGHT,
                Vec3::new(spread, 0.0, 2.0),
                Quat::IDENTITY,
                right_squeeze,
                false,
            ),
        ]
    };
    step(&mut machine, &mut scene, &hands(0.3, false, false));
    step(&mut machine, &mut scene, &hands(0.3, true, false));
    let second = step(&mut machine, &mut scene, &hands(0.3, true, true));
    assert!(second.events.contains(&Event::GrabStart {
        pointer: RIGHT,
        target: BALL
    }));
    assert_eq!(machine.holders_of(BALL), [LEFT, RIGHT]);
    assert!(
        scene
            .placement_of(BALL)
            .approx_eq(&Placement3::IDENTITY, 1e-5),
        "a second hand made it jump"
    );

    step(&mut machine, &mut scene, &hands(0.6, true, true));
    let scaled = scene.placement_of(BALL);
    assert!((scaled.scale - 2.0).abs() < 1e-4, "{}", scaled.scale);
    assert!(
        scaled.translation.abs_diff_eq(Vec3::ZERO, 1e-4),
        "the midpoint did not move"
    );

    // The left hand lets go; the right hand carries on from here, so nothing jumps.
    let one_let_go = step(&mut machine, &mut scene, &hands(0.6, false, true));
    assert!(
        !one_let_go
            .events
            .iter()
            .any(|e| matches!(e, Event::GrabEnd { .. }))
    );
    assert_eq!(machine.holders_of(BALL), [RIGHT]);
    assert!((scene.placement_of(BALL).scale - 2.0).abs() < 1e-4);
    assert!(
        scene
            .placement_of(BALL)
            .translation
            .abs_diff_eq(Vec3::ZERO, 1e-4)
    );

    let all_let_go = step(&mut machine, &mut scene, &hands(0.6, false, false));
    let ends = all_let_go
        .events
        .iter()
        .filter(|e| matches!(e, Event::GrabEnd { .. }))
        .count();
    assert_eq!(ends, 1);
    assert!(
        (scene.placement_of(BALL).scale - 2.0).abs() < 1e-4,
        "the scale is kept after release"
    );
}

#[test]
fn a_third_pointer_cannot_join_a_target_two_hands_already_hold() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    let third = PointerId(7);
    let frames = |third_squeeze: bool| {
        [
            controller(LEFT, Vec3::new(-0.2, 0.0, 2.0), Quat::IDENTITY, true, false),
            controller(RIGHT, Vec3::new(0.2, 0.0, 2.0), Quat::IDENTITY, true, false),
            controller(
                third,
                Vec3::new(0.0, 0.1, 2.0),
                Quat::IDENTITY,
                third_squeeze,
                false,
            ),
        ]
    };
    step(&mut machine, &mut scene, &frames(false));
    step(&mut machine, &mut scene, &frames(false));
    step(&mut machine, &mut scene, &frames(true));
    assert_eq!(machine.holders_of(BALL).len(), 2);
}

#[test]
fn a_pointer_that_vanishes_lets_go_and_stops_hovering() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    let held = controller(LEFT, Vec3::new(0.0, 0.0, 2.0), Quat::IDENTITY, true, false);
    step(&mut machine, &mut scene, &[held]);
    step(&mut machine, &mut scene, &[held]);
    assert_eq!(machine.holders_of(BALL), [LEFT]);
    let gone = step(&mut machine, &mut scene, &[]);
    assert!(gone.events.iter().any(|e| matches!(
        e,
        Event::GrabEnd {
            pointer: LEFT,
            target: BALL,
            ..
        }
    )));
    assert!(gone.events.contains(&Event::HoverEnd {
        pointer: LEFT,
        target: BALL
    }));
    assert!(machine.held_targets().is_empty());
}

#[test]
fn the_snap_hook_runs_once_per_release_and_its_answer_is_reported() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    let socket = Placement3 {
        translation: Vec3::new(0.0, 1.0, 0.0),
        ..Placement3::IDENTITY
    };
    scene.snap_to = Some(socket);
    let at = |squeeze: bool| {
        controller(
            LEFT,
            Vec3::new(0.0, 0.0, 2.0),
            Quat::IDENTITY,
            squeeze,
            false,
        )
    };
    step(&mut machine, &mut scene, &[at(false)]);
    step(&mut machine, &mut scene, &[at(true)]);
    step(&mut machine, &mut scene, &[at(true)]);
    assert!(
        scene.snaps.borrow().is_empty(),
        "snapping must wait for the release"
    );
    let release = step(&mut machine, &mut scene, &[at(false)]);
    assert_eq!(scene.snaps.borrow().len(), 1);
    assert!(matches!(
        release.events.last(),
        Some(Event::GrabEnd { snapped: Some(snapped), .. }) if *snapped == socket
    ));
}

#[test]
fn objects_that_cannot_be_grabbed_are_only_hovered() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    scene.objects.get_mut(&BALL).unwrap().grabbable = false;
    step(&mut machine, &mut scene, &[mouse(0.0, 0.0, false)]);
    step(&mut machine, &mut scene, &[mouse(0.0, 0.0, true)]);
    assert!(machine.held_targets().is_empty());
}

#[test]
fn a_target_that_disappears_while_held_is_released() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    let held = controller(LEFT, Vec3::new(0.0, 0.0, 2.0), Quat::IDENTITY, true, false);
    step(&mut machine, &mut scene, &[held]);
    step(&mut machine, &mut scene, &[held]);
    scene.objects.remove(&BALL);
    let output = step(&mut machine, &mut scene, &[held]);
    assert!(
        output
            .events
            .iter()
            .any(|e| matches!(e, Event::GrabEnd { target: BALL, .. }))
    );
    assert!(machine.held_targets().is_empty());
}

#[test]
fn the_nearer_of_two_targets_is_hovered() {
    let (mut machine, mut scene) = (Interaction::new(50.0), Scene::with_ball());
    scene.add(OTHER, Vec3::new(0.0, 0.0, -3.0), 0.5);
    step(&mut machine, &mut scene, &[mouse(0.0, 0.0, false)]);
    assert_eq!(machine.hover_of(MOUSE), Some(BALL));
}
