//! Property test: whatever a mix of pointers does, the interaction state machine keeps its promises.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;
use proptest::prelude::*;
use ve_interact::{Event, Interaction, PointerFrame, PointerId, TargetId};

use common::{OTHER, Scene, ray_at};

#[derive(Clone, Debug)]
struct FrameSpec {
    id: u32,
    present: bool,
    x: f32,
    y: f32,
    select: bool,
    squeeze: bool,
    reach: f32,
}

fn frame_spec() -> impl Strategy<Value = FrameSpec> {
    (
        0_u32..3,
        any::<bool>(),
        -1.0_f32..1.0,
        -1.0_f32..1.0,
        any::<bool>(),
        any::<bool>(),
        -0.5_f32..0.5,
    )
        .prop_map(|(id, present, x, y, select, squeeze, reach)| FrameSpec {
            id,
            present,
            x,
            y,
            select,
            squeeze,
            reach,
        })
}

/// What the test has seen the machine promise so far.
#[derive(Default)]
struct Ledger {
    hovering: BTreeMap<PointerId, TargetId>,
    /// Targets whose grab the events have opened and not yet closed. A grab opens with its first
    /// `GrabStart` (a second hand joining starts nothing new) and closes with its one `GrabEnd`.
    open_grabs: BTreeSet<TargetId>,
}

/// Runs one frame and checks everything that must hold after any frame.
fn run(
    frames: &[PointerFrame],
    machine: &mut Interaction,
    scene: &mut Scene,
    ledger: &mut Ledger,
) -> Result<(), TestCaseError> {
    let output = machine.update(frames, scene);
    scene.apply(&output);
    for event in &output.events {
        match *event {
            Event::HoverStart { pointer, target } => {
                prop_assert!(
                    ledger.hovering.insert(pointer, target).is_none(),
                    "hover started twice"
                );
            }
            Event::HoverEnd { pointer, target } => {
                prop_assert_eq!(ledger.hovering.remove(&pointer), Some(target));
            }
            Event::GrabStart { target, .. } => {
                ledger.open_grabs.insert(target);
            }
            Event::GrabEnd { target, .. } => {
                prop_assert!(
                    ledger.open_grabs.remove(&target),
                    "{target:?} ended a grab that was never started"
                );
            }
            Event::SelectStart { .. } | Event::SelectEnd { .. } => {}
        }
    }
    for (target, _) in &output.placements {
        prop_assert!(scene.placement_of(*target).is_finite());
    }
    for target in machine.held_targets() {
        prop_assert!(machine.holders_of(target).len() <= 2);
    }
    // What the events say is held is exactly what the machine says is held.
    let held: BTreeSet<TargetId> = machine.held_targets().into_iter().collect();
    prop_assert_eq!(&ledger.open_grabs, &held);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn random_input_keeps_every_invariant(
        script in prop::collection::vec(prop::collection::vec(frame_spec(), 0..4), 1..80),
    ) {
        let mut machine = Interaction::new(50.0);
        let mut scene = Scene::with_ball();
        scene.add(OTHER, Vec3::new(1.0, 0.0, 0.0), 0.4);
        let mut ledger = Ledger::default();

        for specs in script {
            let frames: Vec<PointerFrame> = specs
                .iter()
                .filter(|s| s.present)
                .map(|s| PointerFrame {
                    ray: ray_at(s.x, s.y),
                    select: s.select,
                    squeeze: s.squeeze,
                    reach: s.reach,
                    select_grabs: s.id == 0,
                    ..PointerFrame::idle(PointerId(s.id))
                })
                .collect();
            run(&frames, &mut machine, &mut scene, &mut ledger)?;
        }
        // Every pointer disappears: everything is released and every hover and grab is closed.
        run(&[], &mut machine, &mut scene, &mut ledger)?;
        prop_assert!(machine.held_targets().is_empty());
        prop_assert!(ledger.hovering.is_empty(), "hovers left open: {:?}", ledger.hovering);
        prop_assert!(ledger.open_grabs.is_empty(), "grabs left open: {:?}", ledger.open_grabs);
    }
}
