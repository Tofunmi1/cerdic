//! The geometry an effect is drawn with (plan W4): camera-facing soft sprites and ribbons along trails, as plain vertex arrays.
#![expect(clippy::float_cmp, reason = "alpha and indices are exact by construction")]
use ve_particles::fx::draw::{Batch, Camera, ribbons, sprites};
use ve_particles::fx::{Blend, Burst, Effect, Emitter, Look, Plane, Render, Shape, Spawn, System, Trail};

const DT: f32 = 1.0 / 120.0;
const FAR_BELOW: Plane = Plane { height: -1.0e6 };

fn camera() -> Camera {
    // Looking down -Z from z = 10: right is +X, up is +Y.
    Camera {
        position: [0.0, 0.0, 10.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
    }
}

fn emitter(look: Look) -> Emitter {
    Emitter {
        spawn: Spawn {
            rate: 0.0,
            bursts: vec![Burst {
                time: 0.0,
                count: 1,
            }],
            ..Spawn::default()
        },
        lifetime: [1.0, 1.0],
        speed: [0.0, 0.0],
        shape: Shape::Point,
        look,
        ..Emitter::default()
    }
}

fn run(emitters: Vec<Emitter>, seconds: f32) -> (System, Effect) {
    let effect = Effect { emitters };
    let mut system = System::new(effect.clone(), 1).unwrap();
    for _ in 0..(seconds / DT).round() as u32 {
        system.step(DT, &FAR_BELOW);
    }
    (system, effect)
}

fn check_indices(batch: &Batch) {
    assert_eq!(batch.positions.len(), batch.colours.len());
    assert_eq!(batch.positions.len(), batch.normals.len());
    assert_eq!(batch.indices.len() % 3, 0);
    assert!(batch.indices.iter().all(|i| (*i as usize) < batch.positions.len()));
}

#[test]
fn a_sprite_is_a_soft_round_fan_facing_the_camera() {
    let look = Look {
        size: [1.0, 1.0],
        colour: [[1.0, 0.5, 0.25]; 2],
        alpha: [0.8, 0.8],
        ..Look::default()
    };
    let (system, effect) = run(vec![emitter(look)], 0.1);
    let batch = sprites(&system, &effect, 0, &camera());
    check_indices(&batch);
    assert_eq!(batch.positions.len(), 9, "a centre and a ring of eight");
    assert_eq!(batch.indices.len(), 24);
    // The centre is opaque at the particle's alpha, the rim fades to nothing: a soft edge from vertex colours alone.
    assert!((batch.colours[0][3] - 0.8).abs() < 1e-4);
    assert!(batch.colours[1..].iter().all(|c| c[3] == 0.0));
    assert_eq!(&batch.colours[0][..3], &[1.0, 0.5, 0.25]);
    // Every vertex lies in the plane facing the camera (here z = particle's z = 0).
    assert!(batch.positions.iter().all(|p| p[2].abs() < 1e-5));
    // The rim is round: all ring vertices are the same distance from the centre.
    let c = batch.positions[0];
    let radii: Vec<f32> = batch.positions[1..]
        .iter()
        .map(|p| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt())
        .collect();
    assert!(radii.iter().all(|r| (r - radii[0]).abs() < 1e-5) && radii[0] > 0.2);
}

#[test]
fn a_sprites_rim_is_evenly_spaced_all_the_way_round() {
    // Neighbouring rim vertices are 45 degrees apart: the dot product of their unit directions is cos 45 = 0.7071. A repeated or missing
    // point leaves a lopsided wedge that shows as a shard on a large sprite.
    let (system, effect) = run(vec![emitter(Look::default())], 0.1);
    let batch = sprites(&system, &effect, 0, &camera());
    let c = batch.positions[0];
    let rim: Vec<[f32; 2]> = batch.positions[1..]
        .iter()
        .map(|p| {
            let (x, y) = (p[0] - c[0], p[1] - c[1]);
            let l = (x * x + y * y).sqrt();
            [x / l, y / l]
        })
        .collect();
    assert_eq!(rim.len(), 8);
    for i in 0..8 {
        let (a, b) = (rim[i], rim[(i + 1) % 8]);
        let dot = a[0] * b[0] + a[1] * b[1];
        assert!((dot - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4, "vertices {i} and {} are not 45 degrees apart ({dot})", (i + 1) % 8);
    }
    // Every triangle of the fan has the same winding (none is flipped or degenerate).
    let area = |t: &[u32]| {
        let (p, q, r) = (batch.positions[t[0] as usize], batch.positions[t[1] as usize], batch.positions[t[2] as usize]);
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    assert!(batch.indices.chunks(3).all(|t| area(t) > 1e-6));
}

#[test]
fn size_colour_and_alpha_follow_the_particles_age_and_glow_brightens_it() {
    let look = Look {
        size: [0.2, 0.6],
        colour: [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        alpha: [1.0, 0.0],
        glow: 3.0,
        ..Look::default()
    };
    let radius = |batch: &Batch| {
        let c = batch.positions[0];
        let p = batch.positions[1];
        ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt()
    };
    let (young, effect) = run(vec![emitter(look)], 0.05);
    let (old, _) = run(vec![emitter(look)], 0.5);
    let (a, b) = (sprites(&young, &effect, 0, &camera()), sprites(&old, &effect, 0, &camera()));
    assert!(radius(&b) > radius(&a) * 1.8, "grows with age: {} -> {}", radius(&a), radius(&b));
    assert!(a.colours[0][0] > a.colours[0][2], "young is red");
    assert!(b.colours[0][2] > b.colours[0][0] * 0.5, "old has turned towards blue: {:?}", b.colours[0]);
    assert!(b.colours[0][3] < a.colours[0][3], "fades");
    assert!(a.colours[0][0] > 3.0, "glow multiplies the colour above 1: {}", a.colours[0][0]);
}

#[test]
fn alpha_sprites_are_drawn_far_to_near_and_additive_ones_are_left_alone() {
    let mut alpha = emitter(Look::default());
    alpha.spawn.bursts = vec![Burst {
        time: 0.0,
        count: 40,
    }];
    alpha.shape = Shape::Line { length: 8.0 };
    alpha.direction = [0.0, 0.0, 1.0];
    alpha.max_particles = 100;
    let distance_of_triangles = |batch: &Batch| -> Vec<f32> {
        batch
            .indices
            .chunks(24)
            .map(|fan| {
                let p = batch.positions[fan[0] as usize];
                // Distance from the camera along z.
                10.0 - p[2]
            })
            .collect()
    };
    // Make the particles spread along z by launching them forward then letting them fly.
    alpha.speed = [0.0, 6.0];
    let (system, effect) = run(vec![alpha.clone()], 0.5);
    let batch = sprites(&system, &effect, 0, &camera());
    let d = distance_of_triangles(&batch);
    assert!(d.len() > 20);
    assert!(d.windows(2).all(|w| w[0] >= w[1] - 1e-4), "far to near for alpha blending: {d:?}");
    let mut additive = alpha;
    additive.look.blend = Blend::Additive;
    let (system, effect) = run(vec![additive], 0.5);
    let unsorted = distance_of_triangles(&sprites(&system, &effect, 0, &camera()));
    assert!(!unsorted.windows(2).all(|w| w[0] >= w[1] - 1e-4), "additive order does not matter, so it is not paid for");
}

#[test]
fn a_ribbon_follows_the_trail_tapers_to_the_tail_and_faces_the_camera() {
    let mut e = emitter(Look {
        render: Render::Ribbon,
        size: [0.4, 0.4],
        alpha: [1.0, 1.0],
        colour: [[1.0, 1.0, 1.0]; 2],
        ..Look::default()
    });
    e.speed = [3.0, 3.0];
    e.direction = [1.0, 0.0, 0.0];
    e.trail = Some(Trail {
        points: 8,
        min_step: 0.1,
    });
    let (system, effect) = run(vec![e], 0.6);
    let batch = ribbons(&system, &effect, 0, &camera());
    check_indices(&batch);
    let points = system.trail(0, system.alive(0)[0].id).len();
    assert!(points >= 4, "{points}");
    assert_eq!(batch.positions.len(), points * 2, "two vertices per trail point");
    assert_eq!(batch.indices.len(), (points - 1) * 6, "two triangles per segment");
    // The head is opaque and the tail has faded; the width tapers to nothing at the tail.
    let (tail_alpha, head_alpha) = (batch.colours[0][3], batch.colours[batch.colours.len() - 1][3]);
    assert!(tail_alpha < 0.05 && head_alpha > 0.95, "{tail_alpha} {head_alpha}");
    let width = |i: usize| {
        let (a, b) = (batch.positions[2 * i], batch.positions[2 * i + 1]);
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    assert!(width(0) < 0.05 && width(points - 1) > 0.3, "{} {}", width(0), width(points - 1));
    // The strip's sideways direction is perpendicular to both the path (+X) and the view (+Z): it spreads along Y.
    let (a, b) = (batch.positions[2 * (points - 1)], batch.positions[2 * (points - 1) + 1]);
    assert!((a[0] - b[0]).abs() < 1e-4 && (a[2] - b[2]).abs() < 1e-4 && (a[1] - b[1]).abs() > 0.3);
}

#[test]
fn sprites_and_ribbons_carry_a_normal_per_vertex_facing_the_viewer() {
    let (system, effect) = run(vec![emitter(Look::default())], 0.1);
    let batch = sprites(&system, &effect, 0, &camera());
    assert_eq!(batch.normals.len(), batch.positions.len());
    assert!(batch.normals.iter().all(|n| (n[2] - 1.0).abs() < 1e-5), "towards the camera at +Z");
}

#[test]
fn a_cube_particle_is_a_lit_box_with_outward_normals_that_tumbles_with_age() {
    use ve_particles::fx::draw::cubes;
    let look = Look {
        render: Render::Cube,
        size: [0.5, 0.5],
        colour: [[0.8, 0.4, 0.2]; 2],
        alpha: [1.0, 1.0],
        spin: 4.0,
        ..Look::default()
    };
    let (young, effect) = run(vec![emitter(look)], 0.1);
    let batch = cubes(&young, &effect, 0);
    check_indices(&batch);
    assert_eq!(batch.positions.len(), 24, "four vertices on each of six faces");
    assert_eq!(batch.indices.len(), 36);
    assert_eq!(batch.normals.len(), 24);
    let centre = young.alive(0)[0].position;
    for (p, n) in batch.positions.iter().zip(&batch.normals) {
        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((length - 1.0).abs() < 1e-4, "unit normals");
        let out = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
        assert!(out[0] * n[0] + out[1] * n[1] + out[2] * n[2] > 0.1, "a face's normal points away from the middle of the box");
        // Every corner of a 0.5 m box is 0.25 * sqrt(3) from its middle.
        let radius = (out[0] * out[0] + out[1] * out[1] + out[2] * out[2]).sqrt();
        assert!((radius - 0.25 * 3.0_f32.sqrt()).abs() < 1e-3, "{radius}");
    }
    assert!(batch.colours.iter().all(|c| (c[0] - 0.8).abs() < 1e-5 && c[3] == 1.0));
    // The box has turned a little later in its life.
    let (old, _) = run(vec![emitter(look)], 0.4);
    let later = cubes(&old, &effect, 0);
    let moved = batch
        .positions
        .iter()
        .zip(&later.positions)
        .map(|(a, b)| (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs())
        .fold(0.0_f32, f32::max);
    assert!(moved > 0.2, "a spinning cube is oriented differently later: {moved}");
    // Spin 0 keeps it still.
    let still = Look { spin: 0.0, ..look };
    let (a, still_effect) = run(vec![emitter(still)], 0.1);
    let (b, _) = run(vec![emitter(still)], 0.4);
    let (first, second) = (cubes(&a, &still_effect, 0), cubes(&b, &still_effect, 0));
    assert_eq!(first.normals, second.normals, "no spin, no tumbling");
}

#[test]
fn a_ribbon_emitter_without_a_trail_draws_nothing_rather_than_failing() {
    let e = emitter(Look {
        render: Render::Ribbon,
        ..Look::default()
    });
    let (system, effect) = run(vec![e], 0.2);
    assert_eq!(system.alive(0).len(), 1);
    assert!(ribbons(&system, &effect, 0, &camera()).positions.is_empty());
}

#[test]
fn nothing_alive_or_a_bad_camera_draws_nothing_and_cannot_panic() {
    let (system, effect) = run(vec![emitter(Look::default())], 2.0);
    assert!(system.alive(0).is_empty());
    assert!(sprites(&system, &effect, 0, &camera()).positions.is_empty());
    assert!(ribbons(&system, &effect, 0, &camera()).positions.is_empty());
    assert!(sprites(&system, &effect, 9, &camera()).positions.is_empty(), "an unknown emitter");
    let (alive, effect) = run(vec![emitter(Look::default())], 0.1);
    let bad = Camera {
        position: [f32::NAN, 0.0, 0.0],
        right: [0.0; 3],
        up: [0.0; 3],
    };
    let batch = sprites(&alive, &effect, 0, &bad);
    assert!(batch.positions.iter().all(|p| p.iter().all(|c| c.is_finite())), "no NaN reaches the GPU");
}
