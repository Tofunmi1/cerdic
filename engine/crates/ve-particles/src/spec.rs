use serde::{Deserialize, Serialize};

/// Serializable three component vector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}
impl Vec3 {
    pub(crate) fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
    pub(crate) fn mul(self, n: f32) -> Self {
        Self {
            x: self.x * n,
            y: self.y * n,
            z: self.z * n,
        }
    }
    pub(crate) fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
    pub(crate) fn normalized(self) -> Self {
        let l = self.length();
        if l > 0.0 {
            self.mul(1.0 / l)
        } else {
            Self {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }
        }
    }
}
/// RGBA linear colour.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Colour {
    /// Red channel.
    pub r: f32,
    /// Green channel.
    pub g: f32,
    /// Blue channel.
    pub b: f32,
    /// Alpha channel.
    pub a: f32,
}
/// A curve point at a normalized particle age.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint<T> {
    /// Age from zero to one.
    pub age: f32,
    /// Value at this age.
    pub value: T,
}
/// A linearly interpolated particle property curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve<T> {
    /// Ordered keyframes, including endpoints where desired.
    pub keys: Vec<CurvePoint<T>>,
}
impl<T: Copy> Curve<T> {
    pub(crate) fn sample(&self, age: f32, blend: impl Fn(T, T, f32) -> T) -> T {
        if age <= self.keys[0].age {
            return self.keys[0].value;
        }
        for pair in self.keys.windows(2) {
            if age <= pair[1].age {
                let t = (age - pair[0].age) / (pair[1].age - pair[0].age);
                return blend(pair[0].value, pair[1].value, t);
            }
        }
        self.keys[self.keys.len() - 1].value
    }
}
/// Supported particle spawn geometries.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    /// Spawn at the emitter origin.
    Point,
    /// Uniform-volume sphere radius.
    Sphere {
        /// Sphere radius.
        radius: f32,
    },
    /// Axis aligned box half extents.
    Box {
        /// Box half extents.
        half_extents: Vec3,
    },
    /// Flat disc radius in local XZ.
    Disc {
        /// Disc radius.
        radius: f32,
    },
    /// Segment from origin to endpoint.
    Line {
        /// Segment length.
        length: f32,
    },
}
/// Optional horizontal collision plane.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    /// World Y height.
    pub height: f32,
    /// Vertical velocity multiplier after impact.
    pub bounce: f32,
    /// Remove a particle at impact.
    pub kill_on_hit: bool,
}
/// A validated, serializable particle emitter description.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmitterSpec {
    /// Continuous particle births per second.
    pub spawn_rate: f32,
    /// One-shot birth counts, consumed when the system is created.
    pub bursts: Vec<u32>,
    /// Minimum and maximum lifetime in seconds.
    pub lifetime: [f32; 2],
    /// Initial speed range.
    pub speed: [f32; 2],
    /// Cone half angle in radians.
    pub cone_angle: f32,
    /// Local emission direction.
    pub direction: Vec3,
    /// Birth shape.
    pub shape: Shape,
    /// Acceleration in world units per second squared.
    pub gravity: Vec3,
    /// Linear drag per second.
    pub drag: f32,
    /// Deterministic turbulence magnitude.
    pub turbulence: f32,
    /// Spatial frequency of turbulence.
    pub turbulence_scale: f32,
    /// Spin radians per second range.
    pub spin: [f32; 2],
    /// Size by normalized age.
    pub size: Curve<f32>,
    /// Colour by normalized age.
    pub colour: Curve<Colour>,
    /// Hard particle limit, at most 20000.
    pub max_particles: usize,
    /// Random stream seed.
    pub seed: u64,
    /// Optional ground collision.
    pub ground: Option<Ground>,
}
impl Default for EmitterSpec {
    fn default() -> Self {
        Self {
            spawn_rate: 10.0,
            bursts: Vec::new(),
            lifetime: [1.0, 1.0],
            speed: [1.0, 1.0],
            cone_angle: 0.0,
            direction: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            shape: Shape::Point,
            gravity: Vec3::default(),
            drag: 0.0,
            turbulence: 0.0,
            turbulence_scale: 1.0,
            spin: [0.0, 0.0],
            size: Curve {
                keys: vec![
                    CurvePoint {
                        age: 0.0,
                        value: 1.0,
                    },
                    CurvePoint {
                        age: 1.0,
                        value: 1.0,
                    },
                ],
            },
            colour: Curve {
                keys: vec![
                    CurvePoint {
                        age: 0.0,
                        value: Colour {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                    },
                    CurvePoint {
                        age: 1.0,
                        value: Colour {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 0.0,
                        },
                    },
                ],
            },
            max_particles: 1000,
            seed: 1,
            ground: None,
        }
    }
}
/// Emitter specification validation failure.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum Error {
    /// Field is invalid; message names the field and correction.
    #[error("invalid {0}: {1}")]
    Invalid(&'static str, &'static str),
}
impl EmitterSpec {
    /// Validate ranges, finite numeric values, and curve ordering; errors explain the field to change.
    ///
    /// # Errors
    /// Returns [`Error::Invalid`] with the field and corrective range when a value is invalid.
    #[expect(
        clippy::too_many_lines,
        reason = "validation order mirrors the public emitter fields so errors stay specific"
    )]
    pub fn validate(&self) -> Result<(), Error> {
        let finite = |n: f32| n.is_finite();
        if !finite(self.spawn_rate) || self.spawn_rate < 0.0 {
            return Err(Error::Invalid(
                "spawn_rate",
                "use a finite non-negative rate",
            ));
        }
        for (field, r) in [
            ("lifetime", self.lifetime),
            ("speed", self.speed),
            ("spin", self.spin),
        ] {
            if !finite(r[0]) || !finite(r[1]) || r[0] > r[1] || (field == "lifetime" && r[0] <= 0.0)
            {
                return Err(Error::Invalid(
                    field,
                    "use a finite ordered range (lifetime must be positive)",
                ));
            }
        }
        if self.max_particles == 0 || self.max_particles > 20000 {
            return Err(Error::Invalid(
                "max_particles",
                "choose a value from 1 to 20000",
            ));
        }
        if !finite(self.cone_angle) || !(0.0..=std::f32::consts::PI).contains(&self.cone_angle) {
            return Err(Error::Invalid(
                "cone_angle",
                "use a finite angle from 0 to pi",
            ));
        }
        if ![
            self.direction.x,
            self.direction.y,
            self.direction.z,
            self.gravity.x,
            self.gravity.y,
            self.gravity.z,
            self.drag,
            self.turbulence,
            self.turbulence_scale,
        ]
        .iter()
        .all(|v| finite(*v))
        {
            return Err(Error::Invalid(
                "gravity/drag/turbulence/direction",
                "use finite values",
            ));
        }
        if self.drag < 0.0 {
            return Err(Error::Invalid("drag", "use a non-negative value"));
        }
        if self.turbulence < 0.0 || self.turbulence_scale <= 0.0 {
            return Err(Error::Invalid(
                "turbulence",
                "use non-negative strength and positive scale",
            ));
        }
        match self.shape {
            Shape::Sphere { radius } | Shape::Disc { radius }
                if !finite(radius) || radius < 0.0 =>
            {
                return Err(Error::Invalid(
                    "shape.radius",
                    "use a finite non-negative radius",
                ));
            }
            Shape::Line { length } if !finite(length) || length < 0.0 => {
                return Err(Error::Invalid(
                    "shape.length",
                    "use a finite non-negative length",
                ));
            }
            Shape::Box { half_extents: v }
                if ![v.x, v.y, v.z].iter().all(|x| finite(*x) && *x >= 0.0) =>
            {
                return Err(Error::Invalid(
                    "shape.half_extents",
                    "use finite non-negative extents",
                ));
            }
            _ => (),
        }
        for (field, ages) in [
            (
                "size",
                self.size.keys.iter().map(|k| k.age).collect::<Vec<_>>(),
            ),
            ("colour", self.colour.keys.iter().map(|k| k.age).collect()),
        ] {
            if ages.is_empty()
                || ages[0].abs() > f32::EPSILON
                || (ages.last().unwrap_or(&0.0) - 1.0).abs() > f32::EPSILON
                || ages.iter().any(|a| !finite(*a) || !(0.0..=1.0).contains(a))
                || ages.windows(2).any(|w| w[0] >= w[1])
            {
                return Err(Error::Invalid(
                    field,
                    "provide ordered finite keyframes from age 0 to 1",
                ));
            }
        }
        if self
            .size
            .keys
            .iter()
            .any(|k| !finite(k.value) || k.value < 0.0)
        {
            return Err(Error::Invalid("size", "use finite non-negative key values"));
        }
        if self.colour.keys.iter().any(|k| {
            ![k.value.r, k.value.g, k.value.b, k.value.a]
                .iter()
                .all(|v| finite(*v))
        }) {
            return Err(Error::Invalid("colour", "use finite RGBA key values"));
        }
        if let Some(g) = self.ground
            && (!finite(g.height) || !finite(g.bounce) || !(0.0..=1.0).contains(&g.bounce))
        {
            return Err(Error::Invalid(
                "ground",
                "use finite height and bounce from 0 to 1",
            ));
        }
        Ok(())
    }
}
