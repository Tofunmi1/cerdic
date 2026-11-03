//! The effects simulation (plan W4, `docs/plans/engine-libraries-adoption.md`): emitters with forces, collisions with a world, sub-emitters
//! and trails. Written before the bodies. The checks are physical (ballistics, terminal velocity, circulation, divergence, restitution,
//! conservation of counts), not golden numbers.
#![expect(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "counts and times are small; bit-equality is the determinism check"
)]
use ve_particles::fx::{
    Burst, Collide, Contact, Effect, Emitter, Force, FxError, Plane, Reaction, Shape, Spawn,
    SubEmit, SubTrigger, System, Trail, World, turbulence_field,
};

const DT: f32 = 1.0 / 120.0;

fn emitter() -> Emitter {
    Emitter {
        spawn: Spawn {
            rate: 0.0,
            bursts: vec![Burst {
                time: 0.0,
                count: 1,
            }],
            ..Spawn::default()
        },
        lifetime: [100.0, 100.0],
        speed: [0.0, 0.0],
        shape: Shape::Point,
        ..Emitter::default()
    }
}

fn run(system: &mut System, seconds: f32, world: &dyn World) {
    for _ in 0..(seconds / DT).round() as u32 {
        system.step(DT, world);
    }
}

fn one(emitter: Emitter) -> System {
    System::new(
        Effect {
            emitters: vec![emitter],
        },
        7,
    )
    .unwrap()
}

#[test]
fn a_ball_thrown_up_peaks_at_v_squared_over_two_g() {
    let (v, g) = (8.0_f32, 9.8_f32);
    let mut system = one(Emitter {
        speed: [v, v],
        direction: [0.0, 1.0, 0.0],
        forces: vec![Force::Gravity([0.0, -g, 0.0])],
        ..emitter()
    });
    let mut peak = 0.0_f32;
    for _ in 0..(2.0 / DT) as u32 {
        system.step(DT, &Plane { height: -1.0e6 });
        let p = system.alive(0);
        peak = peak.max(p[0].position[1]);
    }
    let expected = v * v / (2.0 * g);
    assert!(
        (peak - expected).abs() / expected < 0.02,
        "peak {peak} versus {expected}"
    );
}

#[test]
fn wind_drags_a_particle_to_the_wind_speed_and_plain_drag_slows_it() {
    let mut windy = one(Emitter {
        forces: vec![Force::Wind {
            velocity: [3.0, 0.0, 0.0],
            strength: 2.0,
        }],
        ..emitter()
    });
    run(&mut windy, 4.0, &Plane { height: -1.0e6 });
    let v = windy.alive(0)[0].velocity;
    assert!((v[0] - 3.0).abs() < 0.05 && v[1].abs() < 1e-4 && v[2].abs() < 1e-4, "{v:?}");

    let mut slowing = one(Emitter {
        speed: [10.0, 10.0],
        direction: [1.0, 0.0, 0.0],
        forces: vec![Force::Drag(1.5)],
        ..emitter()
    });
    let mut last = f32::MAX;
    for _ in 0..240 {
        slowing.step(DT, &Plane { height: -1.0e6 });
        let speed = slowing.alive(0)[0].velocity[0];
        assert!(speed < last && speed > 0.0, "drag must slow monotonically: {speed}");
        last = speed;
    }
    assert!(last < 10.0 * (-1.5_f32 * 2.0).exp() * 1.1 && last > 10.0 * (-1.5_f32 * 2.0).exp() * 0.9, "{last}");
}

#[test]
fn a_vortex_circulates_around_its_axis_and_an_attractor_pulls_inward() {
    // A particle 2 m from the axis, at rest, in the XZ plane around +Y.
    let start = Shape::Box {
        half_extents: [0.0, 0.0, 0.0],
    };
    let mut swirl = one(Emitter {
        shape: start,
        forces: vec![Force::Vortex {
            centre: [-2.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            strength: 3.0,
            radius: 6.0,
        }],
        ..emitter()
    });
    run(&mut swirl, 0.3, &Plane { height: -1.0e6 });
    let p = swirl.alive(0)[0];
    // Right-handed about +Y at a point on +X of the axis: the push is towards -Z.
    assert!(p.velocity[2] < -0.5, "circulates the right-handed way: {:?}", p.velocity);
    assert!(p.velocity[1].abs() < 1e-3, "stays in the plane");

    let mut pull = one(Emitter {
        forces: vec![Force::Attractor {
            centre: [3.0, 0.0, 0.0],
            strength: 20.0,
            radius: 10.0,
        }],
        ..emitter()
    });
    run(&mut pull, 1.0, &Plane { height: -1.0e6 });
    let p = pull.alive(0)[0];
    assert!(p.velocity[0] > 1.0 && p.position[0] > 0.5 && p.position[1].abs() < 1e-4, "{p:?}");
    // Sitting exactly on the centre must not blow up.
    let mut at_centre = one(Emitter {
        forces: vec![Force::Attractor {
            centre: [0.0, 0.0, 0.0],
            strength: 50.0,
            radius: 5.0,
        }],
        ..emitter()
    });
    run(&mut at_centre, 0.5, &Plane { height: -1.0e6 });
    assert!(at_centre.alive(0)[0].position.iter().all(|c| c.is_finite()));
}

#[test]
fn the_turbulence_field_is_swirly_not_a_source_or_a_sink() {
    // Curl noise has zero divergence: the average |div| is far below the average rate of change of the field.
    let h = 0.05_f32;
    let (mut div, mut change, mut magnitude) = (0.0_f32, 0.0_f32, 0.0_f32);
    let mut n = 0.0;
    for i in 0..300 {
        let t = i as f32;
        let p = [(t * 0.37).fract() * 8.0, (t * 0.61).fract() * 8.0, (t * 0.83).fract() * 8.0];
        let f = |q: [f32; 3]| turbulence_field(1.0, 0.7, 0.3, q);
        let (px, mx) = (f([p[0] + h, p[1], p[2]]), f([p[0] - h, p[1], p[2]]));
        let (py, my) = (f([p[0], p[1] + h, p[2]]), f([p[0], p[1] - h, p[2]]));
        let (pz, mz) = (f([p[0], p[1], p[2] + h]), f([p[0], p[1], p[2] - h]));
        div += ((px[0] - mx[0]) + (py[1] - my[1]) + (pz[2] - mz[2])).abs() / (2.0 * h);
        change += (px[0] - mx[0]).abs() / (2.0 * h) + (py[1] - my[1]).abs() / (2.0 * h) + (pz[2] - mz[2]).abs() / (2.0 * h);
        let v = f(p);
        magnitude += (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        n += 1.0;
    }
    assert!(magnitude / n > 0.2, "the field moves things: {}", magnitude / n);
    assert!(div / n < 0.1 * change / n, "divergence {} against change {}", div / n, change / n);
    // Strength scales it, and time moves it.
    let a = turbulence_field(1.0, 0.7, 0.3, [1.0, 2.0, 3.0]);
    let b = turbulence_field(2.0, 0.7, 0.3, [1.0, 2.0, 3.0]);
    assert!((b[0] - 2.0 * a[0]).abs() < 1e-4 && (b[2] - 2.0 * a[2]).abs() < 1e-4);
    assert_ne!(a, turbulence_field(1.0, 0.7, 5.0, [1.0, 2.0, 3.0]));
}

#[test]
fn a_bouncing_particle_loses_energy_by_the_restitution_and_never_sinks_into_the_ground() {
    let e = 0.5_f32;
    // Dropped from rest at y = 2 (the helper's restitution is 0.5).
    let mut system = one(system_emitter_at_height(2.0));
    let ground = Plane { height: 0.0 };
    let (mut last_vy, mut bounces, mut peak_after) = (0.0_f32, 0, 0.0_f32);
    for _ in 0..(3.0 / DT) as u32 {
        system.step(DT, &ground);
        let p = system.alive(0)[0];
        assert!(p.position[1] >= -1e-4, "sank to {}", p.position[1]);
        if last_vy < -1.0 && p.velocity[1] > 0.0 {
            bounces += 1;
            // Speed after is e times the speed before (a step of slack for the discrete impact).
            assert!((p.velocity[1] / -last_vy - e).abs() < 0.08, "{} after {}", p.velocity[1], last_vy);
            if bounces == 1 {
                peak_after = p.velocity[1] * p.velocity[1] / (2.0 * 9.8);
            }
        }
        last_vy = p.velocity[1];
    }
    assert!(bounces >= 2, "it keeps bouncing: {bounces}");
    assert!(peak_after < 2.0 * e * e * 1.2 && peak_after > 2.0 * e * e * 0.8, "first rebound height {peak_after}");
}

/// A resting particle at a given height with only the ground to hit.
fn system_emitter_at_height(height: f32) -> Emitter {
    Emitter {
        spawn: Spawn {
            rate: 0.0,
            bursts: vec![Burst {
                time: 0.0,
                count: 1,
            }],
            ..Spawn::default()
        },
        lifetime: [100.0, 100.0],
        speed: [0.0, 0.0],
        origin: [0.0, height, 0.0],
        forces: vec![Force::Gravity([0.0, -9.8, 0.0])],
        collide: Some(Collide {
            radius: 0.0,
            reaction: Reaction::Bounce {
                restitution: 0.5,
                friction: 0.0,
            },
        }),
        ..Emitter::default()
    }
}

#[test]
fn stick_stops_kill_removes_and_friction_slows_the_slide() {
    let ground = Plane { height: 0.0 };
    let make = |reaction| {
        let mut e = system_emitter_at_height(1.0);
        e.speed = [3.0, 3.0];
        e.direction = [1.0, 0.0, 0.0];
        e.collide = Some(Collide {
            radius: 0.0,
            reaction,
        });
        one(e)
    };
    let mut stick = make(Reaction::Stick);
    run(&mut stick, 2.0, &ground);
    let p = stick.alive(0)[0];
    assert!(p.velocity.iter().all(|v| v.abs() < 1e-3) && p.position[1] < 1e-3, "{p:?}");
    let mut kill = make(Reaction::Kill);
    run(&mut kill, 2.0, &ground);
    assert!(kill.alive(0).is_empty());
    let mut slippery = make(Reaction::Bounce {
        restitution: 0.0,
        friction: 0.0,
    });
    let mut grippy = make(Reaction::Bounce {
        restitution: 0.0,
        friction: 0.8,
    });
    run(&mut slippery, 2.0, &ground);
    run(&mut grippy, 2.0, &ground);
    assert!(
        grippy.alive(0)[0].position[0] < slippery.alive(0)[0].position[0] - 0.3,
        "friction must shorten the slide"
    );
}

struct Ball {
    centre: [f32; 3],
    radius: f32,
}

impl World for Ball {
    fn contact(&self, p: [f32; 3], r: f32) -> Option<Contact> {
        let d = [p[0] - self.centre[0], p[1] - self.centre[1], p[2] - self.centre[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
        let depth = self.radius + r - len;
        (depth > 0.0).then(|| Contact {
            normal: [d[0] / len, d[1] / len, d[2] / len],
            depth,
        })
    }
}

#[test]
fn particles_hit_any_world_shape_and_none_end_up_inside_it() {
    let ball = Ball {
        centre: [0.0, 0.0, 0.0],
        radius: 1.0,
    };
    let mut e = system_emitter_at_height(4.0);
    e.spawn = Spawn {
        rate: 0.0,
        bursts: vec![Burst {
            time: 0.0,
            count: 60,
        }],
        ..Spawn::default()
    };
    e.shape = Shape::Disc { radius: 0.8 };
    e.collide = Some(Collide {
        radius: 0.02,
        reaction: Reaction::Bounce {
            restitution: 0.4,
            friction: 0.1,
        },
    });
    let mut system = one(e);
    for _ in 0..(2.5 / DT) as u32 {
        system.step(DT, &ball);
        for p in system.alive(0) {
            let d = (p.position[0].powi(2) + p.position[1].powi(2) + p.position[2].powi(2)).sqrt();
            assert!(d >= 1.0 + 0.02 - 0.03, "particle at {d} is inside the ball");
        }
    }
    assert_eq!(system.alive(0).len(), 60);
}

fn parent_child(trigger: SubTrigger, count: u32) -> Effect {
    let mut parent = system_emitter_at_height(1.0);
    parent.spawn = Spawn {
        rate: 0.0,
        bursts: vec![Burst {
            time: 0.0,
            count: 5,
        }],
        ..Spawn::default()
    };
    parent.lifetime = [0.5, 0.5];
    parent.collide = None;
    parent.forces = Vec::new();
    parent.sub = vec![SubEmit {
        on: trigger,
        emitter: 1,
        count,
    }];
    let mut child = emitter();
    child.spawn = Spawn {
        rate: 0.0,
        bursts: Vec::new(),
        ..Spawn::default()
    };
    child.lifetime = [10.0, 10.0];
    Effect {
        emitters: vec![parent, child],
    }
}

#[test]
fn sub_emitters_spawn_children_where_and_when_the_trigger_fires() {
    // On death: 5 parents x 3 children, at the parents' position, after the parents' 0.5 s.
    let mut system = System::new(parent_child(SubTrigger::Death, 3), 1).unwrap();
    run(&mut system, 0.3, &Plane { height: -1.0e6 });
    assert_eq!(system.alive(1).len(), 0, "nothing yet");
    run(&mut system, 0.4, &Plane { height: -1.0e6 });
    assert_eq!(system.alive(0).len(), 0);
    let children = system.alive(1);
    assert_eq!(children.len(), 15);
    assert!(children.iter().all(|c| (c.position[1] - 1.0).abs() < 1e-4), "born where the parent died");
    // On birth: children exist at once.
    let mut system = System::new(parent_child(SubTrigger::Birth, 2), 1).unwrap();
    system.step(DT, &Plane { height: -1.0e6 });
    assert_eq!(system.alive(1).len(), 10);
    // On collision: children at the contact point, one batch per particle that hits.
    let mut effect = parent_child(SubTrigger::Collide, 4);
    effect.emitters[0].lifetime = [5.0, 5.0];
    effect.emitters[0].forces = vec![Force::Gravity([0.0, -9.8, 0.0])];
    effect.emitters[0].collide = Some(Collide {
        radius: 0.0,
        reaction: Reaction::Kill,
    });
    let mut system = System::new(effect, 1).unwrap();
    run(&mut system, 1.0, &Plane { height: 0.0 });
    let children = system.alive(1);
    assert_eq!(children.len(), 20);
    assert!(children.iter().all(|c| c.position[1].abs() < 0.05), "born on the ground");
}

#[test]
fn child_counts_respect_the_childs_own_cap() {
    let mut effect = parent_child(SubTrigger::Birth, 100);
    effect.emitters[1].max_particles = 12;
    let mut system = System::new(effect, 1).unwrap();
    system.step(DT, &Plane { height: -1.0e6 });
    assert_eq!(system.alive(1).len(), 12);
}

#[test]
fn trails_keep_the_recent_path_and_start_fresh_for_a_reused_slot() {
    let mut e = emitter();
    e.speed = [4.0, 4.0];
    e.direction = [1.0, 0.0, 0.0];
    e.lifetime = [1.0, 1.0];
    e.trail = Some(Trail {
        points: 8,
        min_step: 0.1,
    });
    let mut system = one(e);
    run(&mut system, 0.5, &Plane { height: -1.0e6 });
    let p = system.alive(0)[0];
    let trail = system.trail(0, p.id);
    assert!(trail.len() >= 2 && trail.len() <= 8, "{}", trail.len());
    let head = trail[trail.len() - 1];
    assert!((head[0] - p.position[0]).abs() < 0.11, "the newest point is at the particle: {head:?} vs {:?}", p.position);
    for pair in trail.windows(2).take(trail.len().saturating_sub(2)) {
        let gap = (pair[1][0] - pair[0][0]).abs();
        assert!(gap >= 0.09, "points are at least min_step apart: {gap}");
        assert!(pair[1][0] > pair[0][0], "oldest first");
    }
    // After it dies and a new particle takes its place, the new one's trail starts at its own position.
    run(&mut system, 1.0, &Plane { height: -1.0e6 });
    assert!(system.alive(0).is_empty());
    system.burst(0, 1);
    system.step(DT, &Plane { height: -1.0e6 });
    let fresh = system.alive(0)[0];
    let trail = system.trail(0, fresh.id);
    assert!(trail.iter().all(|t| (t[0] - fresh.position[0]).abs() < 0.2), "no stale points: {trail:?}");
}

#[test]
fn spawn_rate_bursts_delay_duration_and_loops_follow_the_clock() {
    let mut e = emitter();
    e.spawn = Spawn {
        rate: 100.0,
        bursts: Vec::new(),
        ..Spawn::default()
    };
    e.max_particles = 10_000;
    let mut system = one(e.clone());
    run(&mut system, 1.0, &Plane { height: -1.0e6 });
    let born = system.alive(0).len() as i64;
    assert!((born - 100).abs() <= 1, "100 per second for 1 s gave {born}");

    let mut delayed = e.clone();
    delayed.spawn.delay = 0.5;
    let mut system = one(delayed);
    run(&mut system, 0.45, &Plane { height: -1.0e6 });
    assert!(system.alive(0).is_empty(), "nothing before the delay");
    run(&mut system, 0.55, &Plane { height: -1.0e6 });
    assert!(system.alive(0).len() > 40, "emits after it");

    let mut timed = e.clone();
    timed.spawn.duration = 0.5;
    let mut system = one(timed);
    run(&mut system, 1.5, &Plane { height: -1.0e6 });
    let born = system.alive(0).len() as i64;
    assert!((born - 50).abs() <= 1, "stops after its duration: {born}");

    let mut looped = e.clone();
    looped.spawn.duration = 0.5;
    looped.spawn.looping = true;
    let mut system = one(looped);
    run(&mut system, 1.5, &Plane { height: -1.0e6 });
    let born = system.alive(0).len() as i64;
    assert!((born - 150).abs() <= 2, "a looping emitter keeps going: {born}");

    let mut bursting = emitter();
    bursting.spawn.bursts = vec![
        Burst {
            time: 0.2,
            count: 7,
        },
        Burst {
            time: 0.6,
            count: 3,
        },
    ];
    let mut system = one(bursting);
    run(&mut system, 0.1, &Plane { height: -1.0e6 });
    assert_eq!(system.alive(0).len(), 0);
    run(&mut system, 0.2, &Plane { height: -1.0e6 });
    assert_eq!(system.alive(0).len(), 7, "first burst once");
    run(&mut system, 0.6, &Plane { height: -1.0e6 });
    assert_eq!(system.alive(0).len(), 10, "second burst once, no repeats");
}

#[test]
fn the_cap_holds_and_a_burst_over_the_cap_is_truncated() {
    let mut e = emitter();
    e.spawn.bursts = vec![Burst {
        time: 0.0,
        count: 5000,
    }];
    e.max_particles = 300;
    let mut system = one(e);
    run(&mut system, 0.1, &Plane { height: -1.0e6 });
    assert_eq!(system.alive(0).len(), 300);
}

#[test]
fn the_same_seed_gives_bit_identical_systems_and_another_seed_a_different_one() {
    let build = |seed| {
        let mut e = emitter();
        e.spawn.rate = 60.0;
        e.speed = [1.0, 3.0];
        e.cone = 0.6;
        e.shape = Shape::Sphere { radius: 0.3 };
        e.lifetime = [0.5, 2.0];
        e.forces = vec![
            Force::Gravity([0.0, -3.0, 0.0]),
            Force::Turbulence {
                strength: 2.0,
                scale: 0.8,
                speed: 0.5,
            },
        ];
        e.trail = Some(Trail {
            points: 6,
            min_step: 0.05,
        });
        e.max_particles = 500;
        let mut system = System::new(Effect { emitters: vec![e] }, seed).unwrap();
        run(&mut system, 2.0, &Plane { height: -1.0e6 });
        system
    };
    let (a, b, c) = (build(5), build(5), build(6));
    let key = |s: &System| -> Vec<u32> {
        s.alive(0)
            .iter()
            .flat_map(|p| p.position.iter().chain(p.velocity.iter()).map(|v| v.to_bits()))
            .collect()
    };
    assert_eq!(key(&a), key(&b));
    assert_ne!(key(&a), key(&c));
    let first = a.alive(0)[0].id;
    assert_eq!(a.trail(0, first), b.trail(0, first));
}

#[test]
fn bad_effects_are_refused_with_the_field_to_fix() {
    let invalid = |effect: Effect, field: &str| {
        let Err(FxError::Invalid { field: got, .. }) = System::new(effect, 1) else {
            panic!("{field} should be refused");
        };
        assert_eq!(got, field);
    };
    let mut e = emitter();
    e.lifetime = [2.0, 1.0];
    invalid(Effect { emitters: vec![e] }, "lifetime");
    let mut e = emitter();
    e.max_particles = 0;
    invalid(Effect { emitters: vec![e] }, "max_particles");
    let mut e = emitter();
    e.speed = [f32::NAN, 1.0];
    invalid(Effect { emitters: vec![e] }, "speed");
    let mut e = emitter();
    e.trail = Some(Trail {
        points: 1,
        min_step: 0.1,
    });
    invalid(Effect { emitters: vec![e] }, "trail");
    let mut e = emitter();
    e.sub = vec![SubEmit {
        on: SubTrigger::Death,
        emitter: 5,
        count: 1,
    }];
    invalid(Effect { emitters: vec![e] }, "sub");
    invalid(Effect { emitters: Vec::new() }, "emitters");
    // A and B spawning each other would never end.
    let mut a = emitter();
    let mut b = emitter();
    a.sub = vec![SubEmit {
        on: SubTrigger::Death,
        emitter: 1,
        count: 1,
    }];
    b.sub = vec![SubEmit {
        on: SubTrigger::Death,
        emitter: 0,
        count: 1,
    }];
    invalid(Effect { emitters: vec![a, b] }, "sub");
}

#[test]
fn an_emitter_rides_on_a_placement_that_moves_and_turns_its_births() {
    let mut e = emitter();
    e.speed = [2.0, 2.0];
    e.direction = [0.0, 1.0, 0.0];
    e.spawn.bursts = Vec::new();
    e.spawn.rate = 0.0;
    let mut system = one(e);
    // At (5, 0, 0), turned a quarter turn about +Z: "up" becomes "-x".
    let quarter = std::f32::consts::FRAC_1_SQRT_2;
    system.place([5.0, 0.0, 0.0], [0.0, 0.0, quarter, quarter]);
    system.burst(0, 1);
    system.step(DT, &Plane { height: -1.0e6 });
    let p = system.alive(0)[0];
    assert!((p.position[0] - 5.0).abs() < 0.05, "born at the placement: {:?}", p.position);
    assert!(p.velocity[0] < -1.99 && p.velocity[1].abs() < 0.01, "its direction turned with it: {:?}", p.velocity);
    // Moving the placement later moves where new particles appear, not the ones already flying.
    system.place([-3.0, 2.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
    system.burst(0, 1);
    system.step(DT, &Plane { height: -1.0e6 });
    let alive = system.alive(0);
    assert!((alive[0].position[0] - 5.0).abs() < 0.2, "the first keeps flying: {:?}", alive[0].position);
    assert!((alive[1].position[0] + 3.0).abs() < 0.05 && (alive[1].position[1] - 2.0).abs() < 0.1);
    // A rotation that is not a unit quaternion is normalised, a zero one is ignored (identity).
    system.place([0.0; 3], [0.0, 0.0, 0.0, 0.0]);
    system.burst(0, 1);
    system.step(DT, &Plane { height: -1.0e6 });
    assert!(system.alive(0).iter().all(|p| p.position.iter().all(|c| c.is_finite())));
}

#[test]
fn how_a_particle_looks_is_checked() {
    use ve_particles::fx::{Blend, Look, Render};
    let bad = |look: Look| {
        let mut e = emitter();
        e.trail = Some(Trail {
            points: 4,
            min_step: 0.1,
        });
        e.look = look;
        let Err(FxError::Invalid { field, .. }) = (Effect { emitters: vec![e] }).validate() else {
            panic!("a bad look must be refused");
        };
        assert_eq!(field, "look");
    };
    bad(Look {
        size: [-1.0, 1.0],
        ..Look::default()
    });
    bad(Look {
        alpha: [0.0, 2.0],
        ..Look::default()
    });
    bad(Look {
        glow: 100.0,
        ..Look::default()
    });
    // A ribbon with no trail is allowed (the trail may be added later); it simply draws nothing until it has one.
    let mut ribbon = emitter();
    ribbon.look = Look {
        render: Render::Ribbon,
        blend: Blend::Additive,
        ..Look::default()
    };
    assert!(Effect { emitters: vec![ribbon.clone()] }.validate().is_ok());
    ribbon.trail = Some(Trail {
        points: 8,
        min_step: 0.05,
    });
    assert!(Effect { emitters: vec![ribbon] }.validate().is_ok());
}

#[test]
fn an_effect_round_trips_through_json_and_omitted_fields_take_their_defaults() {
    let mut e = emitter();
    e.forces = vec![
        Force::Gravity([0.0, -9.8, 0.0]),
        Force::Turbulence {
            strength: 1.0,
            scale: 0.5,
            speed: 0.2,
        },
    ];
    e.collide = Some(Collide {
        radius: 0.05,
        reaction: Reaction::Bounce {
            restitution: 0.4,
            friction: 0.2,
        },
    });
    e.sub = vec![SubEmit {
        on: SubTrigger::Death,
        emitter: 0,
        count: 2,
    }];
    let effect = Effect {
        emitters: vec![e, emitter()],
    };
    let json = serde_json::to_string(&effect).unwrap();
    assert_eq!(serde_json::from_str::<Effect>(&json).unwrap(), effect);
    // A minimal description fills in the rest.
    let minimal: Effect = serde_json::from_str(r#"{"emitters":[{"spawn":{"rate":5.0}}]}"#).unwrap();
    assert_eq!(minimal.emitters[0].spawn.rate, 5.0);
    assert_eq!(minimal.emitters[0].max_particles, Emitter::default().max_particles);
    assert!(minimal.validate().is_ok());
}

#[test]
fn a_huge_time_step_cannot_run_away() {
    let mut e = emitter();
    e.spawn.rate = 50.0;
    let mut system = one(e);
    system.step(1.0e6, &Plane { height: -1.0e6 });
    assert!(system.alive(0).len() <= 1000);
    system.step(f32::NAN, &Plane { height: -1.0e6 });
    system.step(-1.0, &Plane { height: -1.0e6 });
}
