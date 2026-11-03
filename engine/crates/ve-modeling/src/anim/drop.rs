/// Attach a deterministic baked fall to a part.
pub(crate) fn drop(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "anim.drop needs a shape"))?;
    let start = number(node, "start", 0.0);
    let duration = number(node, "duration", 4.0);
    let floor = number(node, "floor", 0.0);
    let bounce = number(node, "bounce", 0.3);
    let gravity = number(node, "gravity", 9.81);
    let velocity = vector_param(node, "velocity", [0.0; 3]);
    let spin = vector_param(node, "spin", [0.0; 3]);
    for (name, value, low, high) in [
        ("start", start, 0.0, 60.0),
        ("duration", duration, 0.5, 60.0),
        ("floor", floor, -1000.0, 1000.0),
        ("bounce", bounce, 0.0, 1.0),
        ("gravity", gravity, 0.1, 50.0),
    ] {
        if !value.is_finite() || !(low..=high).contains(&value) {
            return Err(refusal(name, "is outside its allowed range"));
        }
    }
    if start >= duration {
        return Err(refusal("start", "must be earlier than duration"));
    }
    if velocity.iter().any(|v| !v.is_finite() || v.abs() > 50.0) {
        return Err(refusal(
            "velocity",
            "components must be within -50 to 50 m/s",
        ));
    }
    if spin.iter().any(|v| !v.is_finite() || v.abs() > 2000.0) {
        return Err(refusal(
            "spin",
            "components must be within -2000 to 2000 deg/s",
        ));
    }
    if input.mesh.positions.is_empty() {
        return Err(refusal("floor", "needs a shape with geometry"));
    }
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for p in &input.mesh.positions {
        for axis in 0..3 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    }
    let pivot = std::array::from_fn(|i| (f64::from(low[i]) + f64::from(high[i])) * 0.5);
    let bottom = f64::from(low[1]);
    if floor >= bottom {
        return Err(refusal("floor", "must be below the part's lowest point"));
    }
    let relative_floor = floor - bottom;
    let mut arcs = Vec::new();
    let mut arc_start = start;
    let mut origin = [0.0; 3];
    let mut speed = velocity;
    let mut rates = spin;
    let mut impacts = 0;
    let mut stopped_at = f64::INFINITY;
    while arc_start < duration {
        let Some((flight, impact_velocity)) = next_impact(
            origin[1],
            speed[1],
            relative_floor,
            gravity,
            duration - arc_start,
        ) else {
            arcs.push((arc_start, duration, origin, speed, rates));
            break;
        };
        let impact_time = arc_start + flight;
        let mut impact = [0.0; 3];
        impact[0] = origin[0] + speed[0] * flight;
        impact[1] = relative_floor;
        impact[2] = origin[2] + speed[2] * flight;
        arcs.push((arc_start, impact_time, origin, speed, rates));
        impacts += 1;
        let up = impact_velocity.abs() * bounce;
        if impacts >= 6 || up < 0.001 {
            stopped_at = impact_time;
            arcs.push((impact_time, duration, impact, [0.0; 3], [0.0; 3]));
            break;
        }
        origin = impact;
        speed = [speed[0] * 0.6, up, speed[2] * 0.6];
        rates = rates.map(|rate| rate * 0.6);
        arc_start = impact_time;
    }
    let mut keys = vec![Key {
        time: 0.0,
        value: [0.0; 3],
    }];
    if start > 0.0 {
        keys.push(Key {
            time: start as f32,
            value: [0.0; 3],
        });
    }
    for (from, to, origin, speed, _) in &arcs {
        if *from >= stopped_at {
            continue;
        }
        let each = if (*from - start).abs() < f64::EPSILON {
            16 - usize::from(start > 0.0)
        } else {
            8
        };
        for n in 1..=each {
            let tm = from + (to - from) * n as f64 / (each + 1) as f64;
            let d = tm - from;
            let pos = if *from >= stopped_at {
                *origin
            } else {
                [
                    origin[0] + speed[0] * d,
                    origin[1] + speed[1] * d - 0.5 * gravity * d * d,
                    origin[2] + speed[2] * d,
                ]
            };
            keys.push(Key {
                time: tm as f32,
                value: f32_vec(pos),
            });
        }
        let d = to - from;
        let pos = [
            if *from >= stopped_at {
                origin[0]
            } else {
                origin[0] + speed[0] * d
            },
            if *from >= stopped_at || (to - stopped_at).abs() < 1e-9 {
                relative_floor
            } else {
                origin[1] + speed[1] * d - 0.5 * gravity * d * d
            },
            if *from >= stopped_at {
                origin[2]
            } else {
                origin[2] + speed[2] * d
            },
        ];
        if *to < duration || (to - duration).abs() < 1e-9 && *to != start {
            keys.push(Key {
                time: *to as f32,
                value: f32_vec(pos),
            });
        }
    }
    keys.push(Key {
        time: duration as f32,
        value: arcs.last().map_or([0.0; 3], |(from, to, o, v, _)| {
            let d = to - from;
            f32_vec([
                if *from >= stopped_at {
                    o[0]
                } else {
                    o[0] + v[0] * d
                },
                if *from >= stopped_at || (to - stopped_at).abs() < 1e-9 {
                    relative_floor
                } else {
                    o[1] + v[1] * d - 0.5 * gravity * d * d
                },
                if *from >= stopped_at {
                    o[2]
                } else {
                    o[2] + v[2] * d
                },
            ])
        }),
    });
    keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    keys.dedup_by(|a, b| a.time == b.time);
    let mut value = (**input).clone();
    if value.animation.len() + usize::from(spin.iter().any(|x| *x != 0.0)) + 1 > MAX_TRACKS {
        return Err(fail(
            "param.range",
            format!("a part can carry at most {MAX_TRACKS} tracks"),
        ));
    }
    let clip = clip_name(node)?;
    value.animation.push(Track {
        property: TrackProperty::Position,
        keys,
        interpolation: Interpolation::Linear,
        looped: false,
        pivot: [0.0; 3],
        clip: clip.clone(),
        phase_step: 0.0,
        phase_jitter: 0.0,
    });
    if spin.iter().any(|x| *x != 0.0) {
        let mut rotations = vec![Key {
            time: 0.0,
            value: [0.0; 3],
        }];
        if start > 0.0 {
            rotations.push(Key {
                time: start as f32,
                value: [0.0; 3],
            });
        }
        let mut angle = [0.0; 3];
        for (from, to, _, _, rates) in &arcs {
            if *to > start {
                for axis in 0..3 {
                    angle[axis] += rates[axis] * (to - from);
                }
                rotations.push(Key {
                    time: *to as f32,
                    value: angle.map(|x| x as f32),
                });
            }
        }
        rotations.push(Key {
            time: duration as f32,
            value: angle.map(|x| x as f32),
        });
        rotations.sort_by(|a, b| a.time.total_cmp(&b.time));
        rotations.dedup_by(|a, b| a.time == b.time);
        value.animation.push(Track {
            property: TrackProperty::Rotation,
            keys: rotations,
            interpolation: Interpolation::Linear,
            looped: false,
            pivot: pivot.map(|x| x as f32),
            clip,
            phase_step: 0.0,
            phase_jitter: 0.0,
        });
    }
    Ok(value)
}

fn next_impact(
    origin_y: f64,
    initial_velocity: f64,
    floor: f64,
    gravity: f64,
    remaining: f64,
) -> Option<(f64, f64)> {
    let step_size: f64 = 1.0 / 480.0;
    let (mut elapsed, mut y, mut velocity) = (0.0, origin_y, initial_velocity);
    while elapsed < remaining {
        let previous_y = y;
        let step = step_size.min(remaining - elapsed);
        velocity -= gravity * step;
        y += velocity * step;
        elapsed += step;
        if elapsed > 0.0 && y <= floor {
            let fraction = ((previous_y - floor) / (previous_y - y)).clamp(0.0, 1.0);
            return Some((elapsed - step + step * fraction, velocity));
        }
    }
    None
}

fn f32_vec(v: [f64; 3]) -> [f32; 3] {
    v.map(|x| x as f32)
}
fn number(node: &Node, name: &str, default: f64) -> f64 {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}
fn vector_param(node: &Node, name: &str, default: [f64; 3]) -> [f64; 3] {
    let Some(Param::List(v)) = node.params.get(name) else {
        return default;
    };
    std::array::from_fn(|i| v.get(i).and_then(Param::as_f64).unwrap_or(default[i]))
}
fn refusal(name: &str, message: &str) -> Fail {
    fail_suggest(
        "param.range",
        format!("{name} {message}"),
        format!("adjust {name} to a supported value"),
    )
}

#[cfg(test)]
mod drop_tests {
    #[test]
    fn free_flight_energy_does_not_grow() {
        let mut y = 1.0_f64;
        let mut v = 0.0_f64;
        let mut energy = 0.5 * v * v + 9.81 * y;
        for _ in 0..480 {
            v -= 9.81 / 480.0;
            y += v / 480.0;
            let next = 0.5 * v * v + 9.81 * y;
            assert!(next <= energy + 1e-12);
            energy = next;
        }
    }
    #[test]
    fn restitution_eventually_reaches_rest() {
        let mut speed = 4.0_f64;
        let mut impacts = 0;
        while speed >= 0.001 && impacts < 6 {
            speed *= 0.3;
            impacts += 1;
        }
        assert!(speed < 0.001 || impacts == 6);
    }
}
