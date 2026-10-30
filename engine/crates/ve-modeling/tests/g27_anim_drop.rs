//! `anim.drop` bakes a physical fall into keys: a part let go under gravity, bouncing on the floor and coming to rest. Closed-form checks for the flight
//! (s = v t + g t^2 / 2), the bounce (rise = e^2 x drop) and the rest. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::anim::{Track, TrackProperty};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g27_drop").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(("rejected".to_owned(), format!("{applied:?}")));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| {
            (
                e.code.clone(),
                format!("{} {}", e.message, e.suggestion.clone().unwrap_or_default()),
            )
        })
}

/// A 0.2 m tall box whose centre is 1 m up, so its lowest point is 0.9 m above the floor at y = 0.
fn dropped(params: &str) -> Result<Evaluation, (String, String)> {
    evaluate(&format!(
        "b = solid.box@1(size=[0.12, 0.2, 0.04])\nm = xform.move@1(b, by=[0, 1, 0])\nd = anim.drop@1(m{params})\noutput o = d\n"
    ))
}

fn track(result: &Evaluation, property: TrackProperty) -> Option<&Track> {
    result.parts[0]
        .animation
        .iter()
        .find(|t| t.property == property)
}

/// The track's value at `time`, linearly between keys, held before the first and after the last.
fn at(track: &Track, time: f32) -> [f32; 3] {
    let keys = &track.keys;
    if time <= keys[0].time {
        return keys[0].value;
    }
    for pair in keys.windows(2) {
        if time <= pair[1].time {
            let k = (time - pair[0].time) / (pair[1].time - pair[0].time);
            return std::array::from_fn(|i| {
                pair[0].value[i] + k * (pair[1].value[i] - pair[0].value[i])
            });
        }
    }
    keys[keys.len() - 1].value
}

const G: f32 = 9.81;

#[test]
fn free_fall_follows_the_closed_form() {
    let result = dropped("").unwrap();
    let position = track(&result, TrackProperty::Position).expect("a position track");
    assert!(!position.looped, "a fall plays once");
    for t in [0.1_f32, 0.2, 0.3, 0.4] {
        let y = at(position, t)[1];
        assert!(
            (y + 0.5 * G * t * t).abs() < 0.006,
            "at {t} s: {y} against {}",
            -0.5 * G * t * t
        );
        assert!(
            at(position, t)[0].abs() < 1e-6 && at(position, t)[2].abs() < 1e-6,
            "straight down"
        );
    }
}

#[test]
fn the_part_never_goes_below_the_floor() {
    for (floor, lowest) in [(0.0_f32, -0.9_f32), (0.5, -0.4)] {
        let result = dropped(&format!(", floor={floor}")).unwrap();
        let position = track(&result, TrackProperty::Position).unwrap();
        let min = position
            .keys
            .iter()
            .map(|k| k.value[1])
            .fold(f32::MAX, f32::min);
        assert!(min >= lowest - 1e-4, "floor {floor}: reached {min}");
        assert!(
            min <= lowest + 1e-3,
            "floor {floor}: it lands on the floor ({min} against {lowest})"
        );
    }
}

#[test]
fn the_first_bounce_rises_by_the_restitution_squared() {
    let result = dropped(", bounce=0.3").unwrap();
    let position = track(&result, TrackProperty::Position).unwrap();
    let landing = (2.0_f32 * 0.9 / G).sqrt();
    let peak = (0..200)
        .map(|i| landing + 0.002 + i as f32 * 0.002)
        .map(|t| at(position, t)[1] + 0.9)
        .fold(0.0_f32, f32::max);
    let expected = 0.3_f32 * 0.3 * 0.9;
    assert!(
        (peak - expected).abs() < 0.006,
        "rose {peak} m, expected {expected}"
    );
    let still = dropped(", bounce=0").unwrap();
    let after = at(
        track(&still, TrackProperty::Position).unwrap(),
        landing + 0.3,
    )[1] + 0.9;
    assert!(after.abs() < 1e-3, "no bounce stays down: {after}");
}

#[test]
fn it_comes_to_rest_and_stays() {
    let result = dropped(", duration=5").unwrap();
    let position = track(&result, TrackProperty::Position).unwrap();
    let last = position.keys.last().unwrap();
    assert!(
        (last.time - 5.0).abs() < 1e-5,
        "the track ends at the duration, not earlier: {}",
        last.time
    );
    assert!(
        (last.value[1] + 0.9).abs() < 1e-3,
        "lying on the floor: {}",
        last.value[1]
    );
    for pair in position.keys.windows(2) {
        assert!(pair[1].time > pair[0].time, "keys strictly increase");
    }
    assert!(
        position.keys.len() <= 64,
        "within the key limit: {}",
        position.keys.len()
    );
    let late = position
        .keys
        .iter()
        .filter(|k| k.time > 4.0)
        .map(|k| k.value)
        .collect::<Vec<_>>();
    assert!(
        late.windows(2).all(|w| w[0] == w[1]),
        "nothing moves in the last second"
    );
}

#[test]
fn a_throw_carries_sideways_and_slows_on_the_floor() {
    let result = dropped(", velocity=[1, 2, 0], duration=5").unwrap();
    let position = track(&result, TrackProperty::Position).unwrap();
    let p = at(position, 0.2);
    assert!((p[0] - 0.2).abs() < 0.003, "x at 0.2 s: {}", p[0]);
    assert!(
        (p[1] - (2.0 * 0.2 - 0.5 * G * 0.04)).abs() < 0.006,
        "y at 0.2 s: {}",
        p[1]
    );
    let end = at(position, 5.0);
    assert!(
        end[0] > 0.3 && end[0] < 4.0 && end[0].is_finite(),
        "it slid a little and stopped: {}",
        end[0]
    );
    assert!((end[1] + 0.9).abs() < 1e-3);
}

#[test]
fn start_delays_the_fall() {
    let result = dropped(", start=1").unwrap();
    let position = track(&result, TrackProperty::Position).unwrap();
    assert_eq!(at(position, 0.9), [0.0, 0.0, 0.0], "still before the start");
    assert!(
        (at(position, 1.3)[1] + 0.5 * G * 0.09).abs() < 0.006,
        "0.3 s after release: {}",
        at(position, 1.3)[1]
    );
}

#[test]
fn spin_adds_a_rotation_track_only_when_asked() {
    assert!(
        track(&dropped("").unwrap(), TrackProperty::Rotation).is_none(),
        "no spin, no rotation track"
    );
    let result = dropped(", spin=[0, 0, 180], duration=5").unwrap();
    let rotation = track(&result, TrackProperty::Rotation).expect("a rotation track");
    assert!(
        (at(rotation, 0.2)[2] - 36.0).abs() < 1.5,
        "180 deg/s for 0.2 s: {:?}",
        at(rotation, 0.2)
    );
    let late = rotation
        .keys
        .iter()
        .filter(|k| k.time > 4.0)
        .map(|k| k.value)
        .collect::<Vec<_>>();
    assert!(
        late.windows(2).all(|w| w[0] == w[1]),
        "it stops turning once it rests"
    );
    assert!(
        (rotation.pivot[1] - 1.0).abs() < 1e-4,
        "it turns about the middle of the part: {:?}",
        rotation.pivot
    );
}

#[test]
fn the_geometry_is_kept_and_the_bake_is_deterministic() {
    let plain = evaluate("b = solid.box@1(size=[0.12, 0.2, 0.04])\nm = xform.move@1(b, by=[0, 1, 0])\noutput o = m\n").unwrap();
    let a = dropped(", velocity=[1, 2, 0], spin=[90, 0, 45]").unwrap();
    let b = dropped(", velocity=[1, 2, 0], spin=[90, 0, 45]").unwrap();
    assert_eq!(plain.parts[0].positions, a.parts[0].positions);
    assert_eq!(a.parts[0].animation, b.parts[0].animation);
}

#[test]
fn bad_values_are_refused_with_the_parameter_named() {
    for (params, name) in [
        (", bounce=1.5", "bounce"),
        (", gravity=0", "gravity"),
        (", duration=0", "duration"),
        (", start=-1", "start"),
        (", start=9, duration=4", "start"),
        (", floor=2", "floor"),
    ] {
        let error = dropped(params)
            .err()
            .unwrap_or_else(|| panic!("{params} should be refused"));
        assert!(error.1.contains(name), "{params}: {error:?}");
    }
}
