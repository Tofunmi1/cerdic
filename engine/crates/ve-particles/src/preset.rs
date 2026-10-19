use crate::{Colour, Curve, CurvePoint, EmitterSpec, Ground, Shape, Vec3};

/// Ready-made emitter configurations for common effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Rising, fading flame.
    Fire,
    /// Slow expanding plume.
    Smoke,
    /// Fast short-lived sparks.
    Sparks,
    /// Ground-level motes.
    Dust,
    /// Falling drops.
    Rain,
    /// Slow falling flakes.
    Snow,
    /// Bright magical motes.
    Magic,
    /// Short splash burst.
    Splash,
    /// Radial explosive burst.
    Explosion,
    /// Gentle rising vapour.
    Steam,
    /// Glowing falling cinders.
    Embers,
    /// Upward fountain.
    Fountain,
}
impl Preset {
    /// Create a validated-ready spec scaled in size, speed and rate, with all colours tinted.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive match keeps each named preset's complete settings together"
    )]
    pub fn spec(self, scale: f32, tint: Colour) -> EmitterSpec {
        let mut s = EmitterSpec::default();
        let q = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        match self {
            Self::Fire => {
                s.spawn_rate = 45.0;
                s.lifetime = [0.45, 1.2];
                s.speed = [0.3, 1.8];
                s.gravity = Vec3 {
                    x: 0.0,
                    y: 1.4,
                    z: 0.0,
                };
                s.size = curve(&[(0.0, 0.16), (1.0, 0.0)]);
            }
            Self::Smoke => {
                s.spawn_rate = 12.0;
                s.lifetime = [2.0, 4.0];
                s.speed = [0.2, 0.6];
                s.gravity = Vec3 {
                    x: 0.0,
                    y: 0.25,
                    z: 0.0,
                };
                s.drag = 0.2;
                s.size = curve(&[(0.0, 0.1), (1.0, 0.8)]);
            }
            Self::Sparks => {
                s.spawn_rate = 0.0;
                s.bursts = vec![80];
                s.lifetime = [0.3, 1.1];
                s.speed = [2.0, 8.0];
                s.cone_angle = 2.5;
                s.gravity = Vec3 {
                    x: 0.0,
                    y: -9.8,
                    z: 0.0,
                };
            }
            Self::Dust => {
                s.spawn_rate = 8.0;
                s.lifetime = [1.0, 3.0];
                s.speed = [0.05, 0.4];
                s.turbulence = 0.5;
            }
            Self::Rain => {
                s.spawn_rate = 180.0;
                s.lifetime = [1.0, 2.0];
                s.speed = [8.0, 13.0];
                s.direction = Vec3 {
                    x: 0.0,
                    y: -1.0,
                    z: 0.0,
                };
                s.gravity = Vec3 {
                    x: 0.0,
                    y: -9.8,
                    z: 0.0,
                };
                s.ground = Some(Ground {
                    height: 0.0,
                    bounce: 0.0,
                    kill_on_hit: true,
                });
            }
            Self::Snow => {
                s.spawn_rate = 25.0;
                s.lifetime = [3.0, 6.0];
                s.speed = [0.2, 0.8];
                s.direction = Vec3 {
                    x: 0.0,
                    y: -1.0,
                    z: 0.0,
                };
                s.turbulence = 0.25;
            }
            Self::Magic => {
                s.spawn_rate = 20.0;
                s.lifetime = [0.7, 2.0];
                s.speed = [0.3, 1.2];
                s.shape = Shape::Sphere { radius: 0.3 };
            }
            Self::Splash => {
                s.spawn_rate = 0.0;
                s.bursts = vec![50];
                s.lifetime = [0.4, 1.4];
                s.speed = [1.0, 5.0];
                s.cone_angle = 2.8;
                s.gravity = Vec3 {
                    x: 0.0,
                    y: -6.0,
                    z: 0.0,
                };
            }
            Self::Explosion => {
                s.spawn_rate = 0.0;
                s.bursts = vec![180];
                s.lifetime = [0.5, 2.0];
                s.speed = [2.0, 9.0];
                s.cone_angle = std::f32::consts::PI;
                s.gravity = Vec3 {
                    x: 0.0,
                    y: -2.0,
                    z: 0.0,
                };
            }
            Self::Steam => {
                s.spawn_rate = 14.0;
                s.lifetime = [1.5, 3.5];
                s.speed = [0.2, 0.7];
                s.gravity = Vec3 {
                    x: 0.0,
                    y: 0.35,
                    z: 0.0,
                };
                s.drag = 0.3;
            }
            Self::Embers => {
                s.spawn_rate = 10.0;
                s.lifetime = [1.0, 3.0];
                s.speed = [0.3, 1.5];
                s.gravity = Vec3 {
                    x: 0.0,
                    y: 0.7,
                    z: 0.0,
                };
            }
            Self::Fountain => {
                s.spawn_rate = 35.0;
                s.lifetime = [1.0, 2.5];
                s.speed = [3.0, 7.0];
                s.gravity = Vec3 {
                    x: 0.0,
                    y: -9.8,
                    z: 0.0,
                };
                s.cone_angle = 0.25;
            }
        }
        s.spawn_rate *= q;
        s.speed = [s.speed[0] * q.sqrt(), s.speed[1] * q.sqrt()];
        s.shape = scale_shape(s.shape, q);
        s.size = Curve {
            keys: s
                .size
                .keys
                .into_iter()
                .map(|k| CurvePoint {
                    age: k.age,
                    value: k.value * q,
                })
                .collect(),
        };
        s.colour = Curve {
            keys: vec![
                CurvePoint {
                    age: 0.0,
                    value: tint,
                },
                CurvePoint {
                    age: 1.0,
                    value: Colour {
                        r: tint.r,
                        g: tint.g,
                        b: tint.b,
                        a: 0.0,
                    },
                },
            ],
        };
        s.max_particles = 2000;
        s
    }
}
fn curve(points: &[(f32, f32)]) -> Curve<f32> {
    Curve {
        keys: points
            .iter()
            .map(|(age, value)| CurvePoint {
                age: *age,
                value: *value,
            })
            .collect(),
    }
}
fn scale_shape(shape: Shape, q: f32) -> Shape {
    match shape {
        Shape::Point => Shape::Point,
        Shape::Sphere { radius } => Shape::Sphere { radius: radius * q },
        Shape::Box { half_extents: v } => Shape::Box {
            half_extents: Vec3 {
                x: v.x * q,
                y: v.y * q,
                z: v.z * q,
            },
        },
        Shape::Disc { radius } => Shape::Disc { radius: radius * q },
        Shape::Line { length } => Shape::Line { length: length * q },
    }
}
