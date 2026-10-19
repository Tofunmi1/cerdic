use crate::{Colour, Curve, CurvePoint, EmitterSpec, ParticleSystem, Preset, Vec3};
use glam::Mat4;

fn spec() -> EmitterSpec {
    EmitterSpec {
        spawn_rate: 0.0,
        bursts: vec![10],
        gravity: Vec3 {
            x: 0.0,
            y: -9.8,
            z: 0.0,
        },
        seed: 45,
        ..EmitterSpec::default()
    }
}
fn run(s: EmitterSpec, n: usize, dt: f32) -> Vec<crate::Particle> {
    let mut p = ParticleSystem::new(s);
    for _ in 0..n {
        p.step(dt, Mat4::IDENTITY);
    }
    p.particles().to_vec()
}
#[test]
fn deterministic_and_seeded() {
    let a = run(spec(), 120, 1.0 / 120.0);
    assert_eq!(a, run(spec(), 120, 1.0 / 120.0));
    let mut other = spec();
    other.seed += 1;
    other.speed = [0.5, 2.0];
    assert_ne!(a, run(other, 120, 1.0 / 120.0));
}
#[test]
fn fixed_timestep_matches_frame_sequences() {
    let a = run(spec(), 1, 1.0);
    let b = run(spec(), 60, 1.0 / 60.0);
    assert_eq!(a, b);
}
#[test]
fn cap_and_burst_rate_are_bounded() {
    let mut s = spec();
    s.max_particles = 3;
    s.spawn_rate = 1000.0;
    let mut p = ParticleSystem::new(s);
    p.burst(100);
    p.step(0.1, Mat4::IDENTITY);
    assert!(p.particles().len() <= 3);
}
#[test]
fn lifetime_and_curves() {
    let mut s = spec();
    s.bursts = vec![1];
    s.lifetime = [0.5, 0.5];
    s.speed = [0.0, 0.0];
    s.gravity = Vec3::default();
    s.size = Curve {
        keys: vec![
            CurvePoint {
                age: 0.0,
                value: 0.0,
            },
            CurvePoint {
                age: 0.5,
                value: 2.0,
            },
            CurvePoint {
                age: 1.0,
                value: 0.0,
            },
        ],
    };
    s.colour = Curve {
        keys: vec![
            CurvePoint {
                age: 0.0,
                value: Colour {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            },
            CurvePoint {
                age: 0.5,
                value: Colour {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            },
            CurvePoint {
                age: 1.0,
                value: Colour {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                },
            },
        ],
    };
    let mut p = ParticleSystem::new(s);
    p.step(0.25, Mat4::IDENTITY);
    let i = p.instances()[0];
    assert!((i.size - 2.0).abs() < 0.02);
    assert!((i.colour.r - 1.0).abs() < 0.02);
    p.step(0.25, Mat4::IDENTITY);
    assert!(!p.particles()[0].alive, "particle: {:?}", p.particles()[0]);
    assert!((p.particles()[0].age - 0.5).abs() < 1.0e-6);
}
#[test]
fn gravity_drag_and_ground() {
    let mut s = spec();
    s.bursts = vec![1];
    s.lifetime = [5.0, 5.0];
    s.speed = [0.0, 0.0];
    s.gravity = Vec3 {
        x: 0.0,
        y: -10.0,
        z: 0.0,
    };
    let p = run(s.clone(), 60, 1.0 / 60.0);
    assert!((p[0].position.y + 5.0).abs() < 0.06, "particle: {:?}", p[0]);
    s.drag = 5.0;
    let q = run(s.clone(), 60, 1.0 / 60.0);
    assert!(q[0].position.y > p[0].position.y);
    s.ground = Some(crate::Ground {
        height: -0.1,
        bounce: 0.0,
        kill_on_hit: true,
    });
    let mut g = ParticleSystem::new(s);
    for _ in 0..120 {
        g.step(1.0 / 120.0, Mat4::IDENTITY);
    }
    assert!(!g.particles()[0].alive);
}
#[test]
fn sorted_back_to_front() {
    let mut s = spec();
    s.bursts = vec![2];
    s.gravity = Vec3::default();
    s.shape = crate::Shape::Line { length: 10.0 };
    let mut p = ParticleSystem::new(s);
    p.step(1.0 / 120.0, Mat4::IDENTITY);
    let v = p.sorted_instances(Vec3::default());
    assert!(v[0].position.y.abs() >= v[1].position.y.abs());
}
#[test]
fn presets_validate_produce_and_stay_finite() {
    let tint = Colour {
        r: 0.4,
        g: 0.7,
        b: 1.0,
        a: 1.0,
    };
    for preset in [
        Preset::Fire,
        Preset::Smoke,
        Preset::Sparks,
        Preset::Dust,
        Preset::Rain,
        Preset::Snow,
        Preset::Magic,
        Preset::Splash,
        Preset::Explosion,
        Preset::Steam,
        Preset::Embers,
        Preset::Fountain,
    ] {
        let s = preset.spec(1.0, tint);
        assert!(s.validate().is_ok());
        let mut p = ParticleSystem::new(s);
        for _ in 0..600 {
            p.step(1.0 / 60.0, Mat4::IDENTITY);
        }
        assert!(!p.particles().is_empty());
        assert!(p.particles().iter().all(|x| {
            [x.position.x, x.position.y, x.position.z, x.age]
                .iter()
                .all(|n| n.is_finite())
        }));
    }
}
#[test]
fn invalid_fields_are_named() {
    let mut s = spec();
    s.lifetime = [-1.0, 0.0];
    assert!(s.validate().unwrap_err().to_string().contains("lifetime"));
    s = spec();
    s.max_particles = 0;
    assert!(
        s.validate()
            .unwrap_err()
            .to_string()
            .contains("max_particles")
    );
    s = spec();
    s.max_particles = 20001;
    assert!(
        s.validate()
            .unwrap_err()
            .to_string()
            .contains("max_particles")
    );
    s = spec();
    s.drag = f32::NAN;
    assert!(
        s.validate()
            .unwrap_err()
            .to_string()
            .contains("gravity/drag")
    );
}
