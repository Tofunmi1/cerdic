//! Checks on an effect before it runs: every number in range, sub-emitters pointing at real emitters and never in a loop.
use super::{
    Effect, Emitter, FxError, MAX_CHILDREN, MAX_EMITTERS, MAX_PARTICLES, MAX_TRAIL, Reaction, Shape, forces,
};

fn invalid(field: &'static str, why: &'static str) -> FxError {
    FxError::Invalid { field, why }
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

fn shape_ok(shape: &Shape) -> bool {
    match shape {
        Shape::Point => true,
        Shape::Sphere { radius } | Shape::Disc { radius } => radius.is_finite() && *radius >= 0.0,
        Shape::Box { half_extents } => finite(half_extents) && half_extents.iter().all(|h| *h >= 0.0),
        Shape::Line { length } => length.is_finite() && *length >= 0.0,
    }
}

impl Emitter {
    fn validate(&self, own: usize, count: usize) -> Result<(), FxError> {
        let s = &self.spawn;
        if !(finite(&[s.rate, s.delay, s.duration]) && s.rate >= 0.0 && s.delay >= 0.0 && s.duration >= 0.0)
            || (s.looping && s.duration <= 0.0)
            || s.bursts.iter().any(|b| !b.time.is_finite() || b.time < 0.0)
        {
            return Err(invalid(
                "spawn",
                "rate, delay, duration and burst times must be finite and not negative; looping needs a duration above 0",
            ));
        }
        if !(finite(&self.lifetime) && self.lifetime[0] > 0.0 && self.lifetime[0] <= self.lifetime[1]) {
            return Err(invalid("lifetime", "use 0 < min <= max seconds"));
        }
        if !(finite(&self.speed) && self.speed[0] >= 0.0 && self.speed[0] <= self.speed[1]) {
            return Err(invalid("speed", "use 0 <= min <= max, finite"));
        }
        if !(finite(&self.direction) && self.direction.iter().any(|d| *d != 0.0)) {
            return Err(invalid("direction", "use a finite direction that is not all zero"));
        }
        if !((0.0..=1.5).contains(&self.cone) && finite(&self.origin)) {
            return Err(invalid("cone", "use a cone from 0 to 1.5 radians and a finite origin"));
        }
        if !shape_ok(&self.shape) {
            return Err(invalid("shape", "shape sizes must be finite and not negative"));
        }
        if !(1..=MAX_PARTICLES).contains(&self.max_particles) {
            return Err(invalid("max_particles", "use 1 to 100000"));
        }
        if !self.forces.iter().all(forces::valid) {
            return Err(invalid(
                "forces",
                "force numbers must be finite; radius above 0; vortex axis not zero; strengths not negative where noted",
            ));
        }
        if let Some(c) = &self.collide {
            let reaction_ok = match c.reaction {
                Reaction::Bounce {
                    restitution,
                    friction,
                } => (0.0..=1.0).contains(&restitution) && friction.is_finite() && friction >= 0.0,
                Reaction::Stick | Reaction::Kill => true,
            };
            if !(c.radius.is_finite() && c.radius >= 0.0 && reaction_ok) {
                return Err(invalid(
                    "collide",
                    "radius not negative, restitution 0 to 1, friction not negative",
                ));
            }
        }
        let look = &self.look;
        if !(finite(&look.size)
            && look.size.iter().all(|s| *s >= 0.0)
            && look.colour.iter().all(|c| finite(c))
            && finite(&look.alpha)
            && look.alpha.iter().all(|a| (0.0..=1.0).contains(a))
            && (0.0..=16.0).contains(&look.glow)
            && (0.0..=50.0).contains(&look.spin))
        {
            return Err(invalid(
                "look",
                "size not negative, colours finite, alpha 0 to 1, glow 0 to 16, spin 0 to 50",
            ));
        }
        if let Some(t) = &self.trail
            && !((2..=MAX_TRAIL).contains(&t.points) && t.min_step.is_finite() && t.min_step > 0.0)
        {
            return Err(invalid("trail", "use 2 to 64 points and a min_step above 0"));
        }
        for sub in &self.sub {
            if sub.emitter >= count || sub.emitter == own || sub.count > MAX_CHILDREN {
                return Err(invalid(
                    "sub",
                    "a sub-emitter names another emitter of the effect and spawns at most 1000",
                ));
            }
        }
        Ok(())
    }
}

impl Effect {
    /// Checks every emitter and that sub-emitters never form a loop.
    ///
    /// # Errors
    /// Names the first field that is out of range.
    pub fn validate(&self) -> Result<(), FxError> {
        if self.emitters.is_empty() || self.emitters.len() > MAX_EMITTERS {
            return Err(invalid("emitters", "use 1 to 16 emitters"));
        }
        for (index, emitter) in self.emitters.iter().enumerate() {
            emitter.validate(index, self.emitters.len())?;
        }
        // A sub-emitter loop (a spawns b spawns a) would never end: walk every chain.
        for start in 0..self.emitters.len() {
            let mut stack = vec![start];
            let mut steps = 0;
            while let Some(at) = stack.pop() {
                steps += 1;
                if steps > MAX_EMITTERS * MAX_EMITTERS {
                    return Err(invalid("sub", "sub-emitters must not spawn each other in a loop"));
                }
                stack.extend(self.emitters[at].sub.iter().map(|s| s.emitter));
            }
        }
        Ok(())
    }
}
