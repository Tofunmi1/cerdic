//! Layers and events on the animation state machine (plan W2): a wave over a walk (a masked overlay, either replacing the arm's motion or adding to it) and
//! footsteps at the right moments (events at times within a clip). Written before the bodies.
#![expect(clippy::unwrap_used, clippy::float_cmp, reason = "fixtures are known to be valid; exact numbers are exact")]
use std::collections::HashMap;

use ve_animstate::{Machine, StateMachine, Trs, apply_additive, apply_override, crossed};

fn lengths() -> HashMap<String, f32> {
    HashMap::from([("walk".to_owned(), 1.0), ("wave".to_owned(), 2.0), ("idle".to_owned(), 1.5)])
}

fn machine(text: &str) -> Machine {
    Machine::new(StateMachine::parse(text).unwrap(), &lengths())
}

const WALK: &str = "entry walk\nstate walk clip=walk\nevent walk footstep at=0.25\nevent walk footstep at=0.75\n";

// ---------- crossing events ----------

#[test]
fn an_event_fires_once_as_the_clip_time_passes_it() {
    let times = [0.25, 0.75];
    assert_eq!(crossed(&times, 1.0, 0.2, 0.3, false), [0]);
    assert!(crossed(&times, 1.0, 0.3, 0.7, false).is_empty());
    assert_eq!(crossed(&times, 1.0, 0.7, 0.8, false), [1]);
    // Both in one long step, in order.
    assert_eq!(crossed(&times, 1.0, 0.0, 1.0, false), [0, 1]);
    // An event exactly at the end of a step fires there and not again on the next step: the interval is (from, to].
    assert_eq!(crossed(&times, 1.0, 0.2, 0.25, false), [0]);
    assert!(crossed(&times, 1.0, 0.25, 0.3, false).is_empty());
}

#[test]
fn a_loop_wrap_checks_the_end_of_the_clip_and_then_its_start() {
    let times = [0.0, 0.5, 0.95];
    // 0.9 -> 0.1 through the end: the event at 0.95, then the one at 0.0 (the clip starting over).
    assert_eq!(crossed(&times, 1.0, 0.9, 0.1, true), [2, 0]);
    // Entering a clip for the first time (from before its start) fires an event at 0.
    assert_eq!(crossed(&times, 1.0, -1.0, 0.05, false), [0]);
    // Nothing for a zero-length step, or an empty list, or a bad length.
    assert!(crossed(&times, 1.0, 0.5, 0.5, false).is_empty());
    assert!(crossed(&[], 1.0, 0.0, 1.0, false).is_empty());
    assert!(crossed(&times, 0.0, 0.0, 1.0, false).is_empty());
    assert!(crossed(&times, f32::NAN, 0.0, 1.0, false).is_empty());
}

// ---------- events from a running machine ----------

#[test]
fn a_looping_walk_fires_its_footsteps_at_the_right_rate_and_never_twice() {
    let mut m = machine(WALK);
    let mut fired: Vec<(String, f32)> = Vec::new();
    let mut t = 0.0_f32;
    for _ in 0..(3.0 * 60.0) as u32 {
        let pose = m.step(1.0 / 60.0);
        t += 1.0 / 60.0;
        fired.extend(pose.events.iter().map(|e| (e.name.clone(), t)));
    }
    // Footsteps at 0.25, 0.75, 1.25, ... 2.75 seconds: six in three seconds.
    assert_eq!(fired.len(), 6, "{fired:?}");
    assert!(fired.iter().all(|(name, _)| name == "footstep"));
    for (i, (_, at)) in fired.iter().enumerate() {
        let expected = 0.25 + 0.5 * i as f32;
        assert!((at - expected).abs() < 0.03, "footstep {i} came at {at}, expected {expected}");
    }
}

#[test]
fn a_clip_that_is_fading_out_stops_making_noise_and_a_one_shot_fires_once() {
    // Walk fades into idle; footsteps must not keep sounding while walk is a faint remnant of the blend.
    let mut m = machine(
        "entry walk\nstate walk clip=walk\nstate idle clip=idle\nevent walk footstep at=0.25\nevent walk footstep at=0.75\n\
         transition walk -> idle when stop=true fade=1.0\n",
    );
    for _ in 0..30 {
        m.step(1.0 / 60.0);
    }
    m.set_bool("stop", true);
    let mut late_steps = 0;
    for i in 0..(4.0 * 60.0) as u32 {
        let pose = m.step(1.0 / 60.0);
        // After the fade is well advanced (walk's weight is under a quarter), no footstep may sound.
        if i > 50 {
            late_steps += pose.events.len();
        }
    }
    assert_eq!(late_steps, 0, "a faint clip makes no events");
    let mut once = machine("entry hit\nstate hit clip=wave once\nevent wave impact at=0.5\n");
    let total: usize = (0..(5.0 * 60.0) as u32).map(|_| once.step(1.0 / 60.0).events.len()).sum();
    assert_eq!(total, 1, "a one-shot clip's event fires once");
}

// ---------- layers ----------

#[test]
fn a_layer_follows_its_parameter_smoothly_and_carries_its_mask_and_mode() {
    let mut m = machine(&format!("{WALK}layer wave clip=wave weight=waving mask=arm_l,arm_r fade=0.5\n"));
    let first = m.step(1.0 / 60.0);
    assert!(first.overlays.is_empty() || first.overlays[0].weight == 0.0, "an unused layer has no weight");
    m.set_float("waving", 1.0);
    let mut last = 0.0;
    for _ in 0..60 {
        let pose = m.step(1.0 / 60.0);
        let overlay = &pose.overlays[0];
        assert!(overlay.weight >= last - 1e-6, "it fades in monotonically");
        last = overlay.weight;
    }
    let pose = m.step(1.0 / 60.0);
    let wave = &pose.overlays[0];
    assert!((wave.weight - 1.0).abs() < 1e-3, "fully in after the fade: {}", wave.weight);
    assert_eq!((wave.layer.as_str(), wave.clip.as_str()), ("wave", "wave"));
    assert_eq!(wave.mask, ["arm_l", "arm_r"]);
    assert!(!wave.additive);
    // Its clip time runs and wraps with the clip's length (2 s).
    let mut seen_wrap = false;
    let mut before = wave.time;
    for _ in 0..(3.0 * 60.0) as u32 {
        let t = m.step(1.0 / 60.0).overlays[0].time;
        assert!((0.0..2.0).contains(&t));
        seen_wrap |= t < before;
        before = t;
    }
    assert!(seen_wrap, "the wave loops");
    // Dropping the parameter fades it out again.
    m.set_float("waving", 0.0);
    for _ in 0..90 {
        m.step(1.0 / 60.0);
    }
    let pose = m.step(1.0 / 60.0);
    assert!(pose.overlays.iter().all(|o| o.weight < 1e-3));
}

#[test]
fn a_fixed_weight_additive_layer_is_always_on_and_the_base_blend_is_untouched() {
    let mut m = machine(&format!("{WALK}layer sway clip=wave weight=0.5 additive\n"));
    let pose = m.step(1.0 / 60.0);
    let sway = &pose.overlays[0];
    assert!(sway.additive && sway.mask.is_empty());
    assert!((sway.weight - 0.5).abs() < 1e-3, "{}", sway.weight);
    let total: f32 = pose.layers.iter().map(|l| l.weight).sum();
    assert!((total - 1.0).abs() < 1e-5, "base clip weights still sum to one");
}

// ---------- text ----------

#[test]
fn layers_and_events_survive_a_text_round_trip() {
    let source = format!("{WALK}layer wave clip=wave weight=waving additive mask=arm_l,arm_r speed=1.5 once fade=0.3\nlayer hold clip=idle weight=0.25\n");
    let parsed = StateMachine::parse(&source).unwrap();
    assert_eq!(parsed.extras.layers.len(), 2);
    assert_eq!(parsed.extras.events.len(), 2);
    let again = StateMachine::parse(&parsed.to_text()).unwrap();
    assert_eq!(again, parsed);
    let layer = &parsed.extras.layers[0];
    assert!(layer.additive && !layer.looping && (layer.speed - 1.5).abs() < 1e-6 && (layer.fade - 0.3).abs() < 1e-6);
    assert_eq!(layer.mask, ["arm_l", "arm_r"]);
}

#[test]
fn bad_layers_and_events_are_refused_with_the_line() {
    let refuse = |extra: &str| StateMachine::parse(&format!("{WALK}{extra}\n")).unwrap_err();
    assert_eq!(refuse("layer wave").line, 5, "a layer needs a clip");
    assert!(refuse("layer wave clip=wave weight=").message.contains("weight"));
    assert!(refuse("layer wave clip=wave weight=2").message.contains("weight"), "a fixed weight is 0 to 1");
    assert!(refuse("layer wave clip=wave speed=0").message.contains("speed"));
    assert!(refuse("layer wave clip=wave fade=-1").message.contains("fade"));
    assert!(refuse("layer wave clip=wave bogus=1").message.contains("bogus"));
    assert!(refuse("layer wave clip=wave\nlayer wave clip=idle").message.contains("wave"), "layer names are unique");
    assert!(refuse("event walk").message.contains("at="), "an event needs a time");
    assert!(refuse("event walk step at=-1").message.contains("at"));
    assert!(refuse("event walk bad-name! at=0.5").message.contains("name"));
    let many_layers = (0..6).map(|i| format!("layer l{i} clip=wave")).collect::<Vec<_>>().join("\n");
    assert!(refuse(&many_layers).message.contains("layers"), "at most 4 layers");
    let many_events = (0..70).map(|i| format!("event walk e{i} at=0.5")).collect::<Vec<_>>().join("\n");
    assert!(refuse(&many_events).message.contains("events"), "at most 64 events");
}

// ---------- the arithmetic the runtime blends with ----------

fn trs(t: [f32; 3], q: [f32; 4], s: [f32; 3]) -> Trs {
    Trs {
        translation: t,
        rotation: q,
        scale: s,
    }
}

const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[test]
fn additive_adds_the_change_from_the_reference_and_scales_with_the_weight() {
    let base = trs([1.0, 0.0, 0.0], IDENTITY, [1.0; 3]);
    let reference = trs([0.0, 1.0, 0.0], IDENTITY, [1.0; 3]);
    let current = trs([0.0, 3.0, 0.0], IDENTITY, [1.0; 3]);
    let full = apply_additive(&base, &reference, &current, 1.0);
    assert_eq!(full.translation, [1.0, 2.0, 0.0], "the clip moved +2 in y from its reference");
    let half = apply_additive(&base, &reference, &current, 0.5);
    assert_eq!(half.translation, [1.0, 1.0, 0.0]);
    assert_eq!(apply_additive(&base, &reference, &current, 0.0).translation, base.translation);
    // A clip that is still at its reference changes nothing.
    assert_eq!(apply_additive(&base, &reference, &reference, 1.0), base);
    // Rotation: a quarter turn about Z in the clip becomes a quarter turn on top of the base's own, half weight an eighth.
    let quarter = [0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2];
    let turned = apply_additive(&base, &trs([0.0; 3], IDENTITY, [1.0; 3]), &trs([0.0; 3], quarter, [1.0; 3]), 1.0);
    assert!((turned.rotation[2] - quarter[2]).abs() < 1e-5 && (turned.rotation[3] - quarter[3]).abs() < 1e-5);
    let eighth = apply_additive(&base, &trs([0.0; 3], IDENTITY, [1.0; 3]), &trs([0.0; 3], quarter, [1.0; 3]), 0.5);
    let length = eighth.rotation.iter().map(|c| c * c).sum::<f32>().sqrt();
    assert!((length - 1.0).abs() < 1e-5, "the result is a unit rotation");
    assert!(eighth.rotation[2] > 0.33 && eighth.rotation[2] < 0.43, "half of a quarter turn is an eighth, sin(22.5 deg) = 0.38: {:?}", eighth.rotation);
    // Scale multiplies by the clip's ratio to its reference; a zero reference scale cannot divide.
    let scaled = apply_additive(&base, &trs([0.0; 3], IDENTITY, [1.0; 3]), &trs([0.0; 3], IDENTITY, [2.0, 1.0, 1.0]), 1.0);
    assert_eq!(scaled.scale, [2.0, 1.0, 1.0]);
    let safe = apply_additive(&base, &trs([0.0; 3], IDENTITY, [0.0; 3]), &trs([0.0; 3], IDENTITY, [1.0; 3]), 1.0);
    assert!(safe.scale.iter().all(|s| s.is_finite()));
}

#[test]
fn an_override_blends_from_the_base_to_the_clip_by_the_weight() {
    let base = trs([0.0; 3], IDENTITY, [1.0; 3]);
    let clip = trs([2.0, 0.0, 0.0], IDENTITY, [3.0; 3]);
    assert_eq!(apply_override(&base, &clip, 0.0), base);
    let full = apply_override(&base, &clip, 1.0);
    assert!((full.translation[0] - 2.0).abs() < 1e-5 && (full.scale[0] - 3.0).abs() < 1e-5);
    let half = apply_override(&base, &clip, 0.5);
    assert!((half.translation[0] - 1.0).abs() < 1e-5 && (half.scale[0] - 2.0).abs() < 1e-5);
    assert!(apply_override(&base, &clip, f32::NAN).translation.iter().all(|v| v.is_finite()));
}
