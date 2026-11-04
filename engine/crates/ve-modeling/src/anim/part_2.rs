
/// Reads `keys` text: `time:x,y,z` entries separated by `;`. Every entry names all three values.
pub(crate) fn parse_keys(text: &str, property: TrackProperty) -> Result<Vec<Key>, Fail> {
    let mut keys = Vec::new();
    for entry in text.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let bad = || {
            fail(
                "param.range",
                format!("anim.track key \"{entry}\" must be time:x,y,z"),
            )
        };
        let (time, rest) = entry.split_once(':').ok_or_else(bad)?;
        let time: f32 = time.trim().parse().map_err(|_| bad())?;
        let numbers: Vec<f32> = rest
            .split(',')
            .map(|n| n.trim().parse::<f32>())
            .collect::<Result<_, _>>()
            .map_err(|_| bad())?;
        // A glow key is one number: `time:factor`.
        let [x, y, z] = match (property, &numbers[..]) {
            (TrackProperty::Glow | TrackProperty::Visible, [factor]) => [*factor, 0.0, 0.0],
            (_, [x, y, z]) => [*x, *y, *z],
            _ => return Err(bad()),
        };
        let limit = match property {
            TrackProperty::Position | TrackProperty::Scale => 1000.0,
            TrackProperty::Rotation => 36_000.0,
            TrackProperty::Glow => 100.0,
            TrackProperty::Visible | TrackProperty::Color => 1.0,
        };
        if ![time, x, y, z].iter().all(|v| v.is_finite())
            || !(0.0..=MAX_DURATION).contains(&time)
            || [x, y, z].iter().any(|v| v.abs() > limit)
            || (property == TrackProperty::Scale && [x, y, z].iter().any(|v| *v < 0.001))
            || (matches!(property, TrackProperty::Glow | TrackProperty::Visible) && x < 0.0)
            || (property == TrackProperty::Color && [x, y, z].iter().any(|v| *v < 0.0))
        {
            return Err(fail(
                "param.range",
                format!("anim.track key \"{entry}\" is outside the allowed range"),
            ));
        }
        keys.push(Key {
            time,
            value: [x, y, z],
        });
    }
    if keys.len() < 2 || keys.len() > MAX_KEYS {
        return Err(fail(
            "param.range",
            format!("anim.track needs 2 to {MAX_KEYS} keys, got {}", keys.len()),
        ));
    }
    if keys.windows(2).any(|pair| pair[1].time <= pair[0].time) {
        return Err(fail(
            "param.range",
            "anim.track key times must strictly increase",
        ));
    }
    Ok(keys)
}

pub(crate) fn pivot(node: &Node) -> [f32; 3] {
    let Some(Param::List(items)) = node.params.get("pivot") else {
        return [0.0; 3];
    };
    let mut out = [0.0_f32; 3];
    for (slot, item) in out.iter_mut().zip(items) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "pivot coordinates are bounded metres"
        )]
        {
            *slot = item.as_f64().unwrap_or(0.0) as f32;
        }
    }
    out
}

/// The point a track turns about: the typed `pivot`, or with `pivot_at` the head or tail of the bone the input is.
pub(crate) fn pivot_for(node: &Node, input: &Value) -> Result<[f32; 3], Fail> {
    let at = match node.params.get("pivot_at") {
        Some(Param::Str(at)) => at.as_str(),
        _ => "none",
    };
    match at {
        "none" => Ok(pivot(node)),
        "head" | "tail" => {
            let segment = input.bone.ok_or_else(|| {
                fail_suggest(
                    "anim.pivot_needs_bone",
                    format!("pivot_at={at:?} needs the part to be a bone"),
                    "put the track on a body.bone or rig.bone, or type the pivot",
                )
            })?;
            Ok(if at == "head" { segment.head } else { segment.tail })
        }
        other => Err(fail(
            "param.range",
            format!("pivot_at {other:?} is none, head or tail"),
        )),
    }
}

/// Attaches the node's track to the input's tracks.
pub(crate) fn track(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "anim.track needs a shape"))?;
    let (Some(Param::Str(property)), Some(Param::Str(keys)), Some(Param::Str(interpolation))) = (
        node.params.get("property"),
        node.params.get("keys"),
        node.params.get("interpolation"),
    ) else {
        return Err(fail(
            "param.type",
            "anim.track needs property, keys and interpolation",
        ));
    };
    let property = match property.as_str() {
        "position" => TrackProperty::Position,
        "rotation" => TrackProperty::Rotation,
        "scale" => TrackProperty::Scale,
        "glow" => TrackProperty::Glow,
        "visible" => TrackProperty::Visible,
        "color" => TrackProperty::Color,
        other => {
            return Err(fail(
                "param.range",
                format!("unknown anim.track property \"{other}\""),
            ));
        }
    };
    let interpolation = match interpolation.as_str() {
        "linear" => Interpolation::Linear,
        "smooth" => Interpolation::Smooth,
        "ease_in" => Interpolation::EaseIn,
        "ease_out" => Interpolation::EaseOut,
        "bezier" => Interpolation::Bezier,
        "back" => Interpolation::Back,
        "bounce" => Interpolation::Bounce,
        "elastic" => Interpolation::Elastic,
        "step" => Interpolation::Step,
        other => {
            return Err(fail(
                "param.range",
                format!("unknown anim.track interpolation \"{other}\""),
            ));
        }
    };
    let looped = !matches!(node.params.get("loop"), Some(Param::Bool(false)));
    let keys = parse_keys(keys, property)?;
    let mut value = (**input).clone();
    if value.animation.len() >= MAX_TRACKS {
        return Err(fail(
            "param.range",
            format!("a part can carry at most {MAX_TRACKS} tracks"),
        ));
    }
    let (phase_step, phase_jitter) = phases(node, input)?;
    value.animation.push(Track {
        property,
        keys,
        interpolation,
        looped,
        pivot: pivot_for(node, input)?,
        clip: clip_name(node)?,
        phase_step,
        phase_jitter,
    });
    Ok(value)
}

pub use crate::reaction::{
    Action, Behavior, MAX_REACTIVE_PARTS, RADIUS_RANGE, Reaction, STEP, Trigger, timer_due,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn track(interpolation: Interpolation, looped: bool) -> Track {
        Track {
            property: TrackProperty::Position,
            keys: parse_keys("1:0,0,0; 3:10,20,30", TrackProperty::Position).unwrap_or_default(),
            interpolation,
            looped,
            pivot: [0.0; 3],
            clip: String::new(),
            phase_step: 0.0,
            phase_jitter: 0.0,
        }
    }

    #[test]
    fn sampling_follows_keys_holds_ends_and_loops() {
        let linear = track(Interpolation::Linear, false);
        assert_eq!(linear.sample(0.0), [0.0; 3], "before the first key");
        assert_eq!(linear.sample(2.0), [5.0, 10.0, 15.0], "halfway");
        assert_eq!(
            linear.sample(99.0),
            [10.0, 20.0, 30.0],
            "after the last key"
        );
        let smooth = track(Interpolation::Smooth, false);
        assert_eq!(
            smooth.sample(2.0),
            [5.0, 10.0, 15.0],
            "smoothstep is half at the middle"
        );
        assert!(
            smooth.sample(1.5)[0] < 2.5,
            "eased: less than linear early on"
        );
        let step = track(Interpolation::Step, false);
        assert_eq!(step.sample(2.9), [0.0; 3], "held until the next key");
        assert_eq!(step.sample(3.0), [10.0, 20.0, 30.0]);
        let looped = track(Interpolation::Linear, true);
        assert_eq!(
            looped.sample(4.0),
            looped.sample(2.0),
            "wraps after two seconds"
        );
        assert_eq!(looped.sample(1.0), [0.0; 3]);
    }

    #[test]
    fn jitter_matches_the_typescript_copy_the_exporter_uses() {
        // Values printed by `jitterFraction` in src/engine/animation.ts for the same indices.
        let expected = [
            (0_u32, 0.392_125_13_f32),
            (1, 0.367_875_52),
            (2, 0.032_725_81),
            (3, 0.553_469_2),
            (7, 0.893_695_65),
            (100, 0.524_453_1),
            (4095, 0.813_762_4),
        ];
        for (index, value) in expected {
            assert!(
                (jitter_fraction(index) - value).abs() < 1e-7,
                "index {index}: {} vs {value}",
                jitter_fraction(index)
            );
        }
    }

    #[test]
    fn phase_offsets_are_fixed_spread_out_and_in_range() {
        let fractions: Vec<f32> = (0..200).map(jitter_fraction).collect();
        assert!(
            fractions.iter().all(|f| (0.0..1.0).contains(f)),
            "always in 0 to 1"
        );
        assert_eq!(
            fractions,
            (0..200).map(jitter_fraction).collect::<Vec<_>>(),
            "the same every time"
        );
        let mean = fractions.iter().sum::<f32>() / 200.0;
        assert!((0.4..0.6).contains(&mean), "spread evenly: mean {mean}");
        let neighbours_equal = fractions
            .windows(2)
            .filter(|w| (w[0] - w[1]).abs() < 0.01)
            .count();
        assert!(
            neighbours_equal < 10,
            "neighbouring copies do not follow each other"
        );
        let mut t = track(Interpolation::Linear, true);
        (t.phase_step, t.phase_jitter) = (0.25, 2.0);
        assert!(
            (t.phase(4) - (1.0 + 2.0 * jitter_fraction(4))).abs() < 1e-6,
            "step times index plus jitter"
        );
        assert!(t.phase(0) < 2.0 && t.phase(0) >= 0.0);
    }

    #[test]
    fn keys_parse_in_order_and_refuse_bad_text() {
        let keys = parse_keys("0:0,0,0; 1.5:0,90,0", TrackProperty::Rotation).unwrap_or_default();
        assert_eq!(keys.len(), 2);
        assert!((keys[1].time - 1.5).abs() < 1e-6 && (keys[1].value[1] - 90.0).abs() < 1e-6);
        for bad in [
            "",
            "0:0,0,0",
            "0:0,0; 1:1,1,1",
            "1:0,0,0; 1:1,1,1",
            "1:0,0,0; 0:1,1,1",
            "0:0,0,x; 1:0,0,0",
            "0:nan,0,0; 1:0,0,0",
        ] {
            assert!(
                parse_keys(bad, TrackProperty::Position).is_err(),
                "{bad:?} is refused"
            );
        }
        assert!(
            parse_keys("0:1,1,0; 1:1,1,1", TrackProperty::Scale).is_err(),
            "a scale of zero is refused"
        );
    }
}
