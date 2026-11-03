//! The simulation: emitters give birth on their own clocks, forces and the world move particles in fixed steps, triggers spawn children.
use super::emit::{self, Rng};
use super::{Contact, Effect, Emitter, FxError, Reaction, SubTrigger, World, forces};

/// Fixed simulation step, seconds.
const STEP: f32 = 1.0 / 120.0;
const STEP64: f64 = 1.0 / 120.0;
/// A frame longer than this is cut, so a stalled page does not simulate minutes at once.
const MAX_FRAME: f32 = 10.0;
/// A hit slower than this (metres per second) is resting contact, not an impact: no bounce, no collide trigger.
const IMPACT: f32 = 0.5;

/// A live particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// Stable for the particle's life (look up its trail with it).
    pub id: u32,
    /// World position.
    pub position: [f32; 3],
    /// World velocity.
    pub velocity: [f32; 3],
    /// Seconds lived.
    pub age: f32,
    /// Seconds it will live.
    pub lifetime: f32,
}

struct Slot {
    particle: Particle,
    alive: bool,
    /// Recorded path, oldest first; the particle's own position is the newest point and is not stored.
    trail: Vec<[f32; 3]>,
}

struct State {
    slots: Vec<Slot>,
    /// Seconds since the emitter started (negative during its delay).
    local: f32,
    credit: f32,
    queued: u32,
    next_id: u32,
}

/// A running effect.
pub struct System {
    effect: Effect,
    rng: Rng,
    states: Vec<State>,
    time: f32,
    accumulator: f64,
    /// Children waiting to be born: emitter, position, count.
    pending: Vec<(usize, [f32; 3], u32)>,
    /// Where the effect is: a world position and a unit rotation (x, y, z, w) that births are placed and turned by.
    placement: ([f32; 3], [f32; 4]),
}

/// Rotates a vector by a unit quaternion (x, y, z, w).
fn rotate(turn: [f32; 4], vector: [f32; 3]) -> [f32; 3] {
    let axis = [turn[0], turn[1], turn[2]];
    let cross = |first: [f32; 3], second: [f32; 3]| {
        [
            first[1] * second[2] - first[2] * second[1],
            first[2] * second[0] - first[0] * second[2],
            first[0] * second[1] - first[1] * second[0],
        ]
    };
    let inner = cross(axis, vector);
    let inner = [
        inner[0] + turn[3] * vector[0],
        inner[1] + turn[3] * vector[1],
        inner[2] + turn[3] * vector[2],
    ];
    let outer = cross(axis, inner);
    [
        vector[0] + 2.0 * outer[0],
        vector[1] + 2.0 * outer[1],
        vector[2] + 2.0 * outer[2],
    ]
}

/// The active intervals, in the emitter's cycle time, within `from..to` of its clock.
fn windows(from: f32, to: f32, duration: f32, looping: bool) -> Vec<(f32, f32)> {
    let start = from.max(0.0);
    if to <= start {
        return Vec::new();
    }
    if duration <= 0.0 {
        return vec![(start, to)];
    }
    if !looping {
        let end = to.min(duration);
        return if end > start { vec![(start, end)] } else { Vec::new() };
    }
    let mut out = Vec::new();
    #[expect(clippy::cast_possible_truncation, reason = "a cycle index is small")]
    let first = (start / duration).floor() as i64;
    for cycle in first.. {
        #[expect(clippy::cast_precision_loss, reason = "a cycle index is small")]
        let base = cycle as f32 * duration;
        if base >= to {
            break;
        }
        let (s, e) = (start.max(base) - base, to.min(base + duration) - base);
        if e > s {
            out.push((s, e));
        }
    }
    out
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    dot([a[0] - b[0], a[1] - b[1], a[2] - b[2]], [a[0] - b[0], a[1] - b[1], a[2] - b[2]]).sqrt()
}

impl System {
    /// Starts an effect.
    ///
    /// # Errors
    /// Names the first setting of the effect that is out of range.
    pub fn new(effect: Effect, seed: u64) -> Result<Self, FxError> {
        effect.validate()?;
        let states = effect
            .emitters
            .iter()
            .map(|e| State {
                slots: Vec::new(),
                local: -e.spawn.delay,
                credit: 0.0,
                queued: 0,
                next_id: 0,
            })
            .collect();
        Ok(Self {
            effect,
            rng: Rng::new(seed),
            states,
            time: 0.0,
            accumulator: 0.0,
            pending: Vec::new(),
            placement: ([0.0; 3], [0.0, 0.0, 0.0, 1.0]),
        })
    }

    /// Moves and turns the effect: particles born from now on appear at `position` and leave in directions turned by `rotation`
    /// (x, y, z, w; normalised here, and the identity if it is zero or not finite). Particles already flying are not moved.
    pub fn place(&mut self, position: [f32; 3], rotation: [f32; 4]) {
        let length = rotation.iter().map(|c| c * c).sum::<f32>().sqrt();
        let rotation = if length.is_finite() && length > 1.0e-6 {
            rotation.map(|c| c / length)
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        let position = if position.iter().all(|c| c.is_finite()) { position } else { [0.0; 3] };
        self.placement = (position, rotation);
    }

    /// Seconds simulated.
    #[must_use]
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Queues `count` extra births for an emitter on the next step (a one-shot burst from the game).
    pub fn burst(&mut self, emitter: usize, count: u32) {
        if let Some(state) = self.states.get_mut(emitter) {
            state.queued = state.queued.saturating_add(count);
        }
    }

    /// The live particles of an emitter, in slot order.
    #[must_use]
    pub fn alive(&self, emitter: usize) -> Vec<Particle> {
        self.states
            .get(emitter)
            .map(|s| s.slots.iter().filter(|slot| slot.alive).map(|slot| slot.particle).collect())
            .unwrap_or_default()
    }

    /// Every live particle of an emitter with its recent path (oldest first, ending at the particle itself), in one pass.
    #[must_use]
    pub fn alive_with_trails(&self, emitter: usize) -> Vec<(Particle, Vec<[f32; 3]>)> {
        let Some(state) = self.states.get(emitter) else {
            return Vec::new();
        };
        state
            .slots
            .iter()
            .filter(|slot| slot.alive)
            .map(|slot| {
                let mut points = slot.trail.clone();
                points.push(slot.particle.position);
                (slot.particle, points)
            })
            .collect()
    }

    /// A particle's recent path, oldest first, ending at the particle itself. Empty for an unknown or dead particle.
    #[must_use]
    pub fn trail(&self, emitter: usize, id: u32) -> Vec<[f32; 3]> {
        let Some(slot) = self
            .states
            .get(emitter)
            .and_then(|s| s.slots.iter().find(|slot| slot.alive && slot.particle.id == id))
        else {
            return Vec::new();
        };
        let mut points = slot.trail.clone();
        points.push(slot.particle.position);
        points
    }

    /// Advances by `dt` seconds in fixed steps. A non-positive or non-finite `dt` does nothing.
    pub fn step(&mut self, dt: f32, world: &dyn World) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.accumulator += f64::from(dt.min(MAX_FRAME));
        while self.accumulator + 1.0e-7 >= STEP64 {
            self.tick(world);
            self.accumulator = (self.accumulator - STEP64).max(0.0);
        }
    }

    fn tick(&mut self, world: &dyn World) {
        for emitter in 0..self.states.len() {
            self.emit(emitter);
        }
        self.flush();
        for emitter in 0..self.states.len() {
            self.advance(emitter, world);
        }
        self.flush();
        self.time += STEP;
    }

    /// Births due this step: bursts, the rate, and anything queued by the game.
    fn emit(&mut self, index: usize) {
        let spawn = &self.effect.emitters[index].spawn;
        let state = &mut self.states[index];
        let (from, to) = (state.local, state.local + STEP);
        state.local = to;
        let mut births = std::mem::take(&mut state.queued);
        for (start, end) in windows(from, to, spawn.duration, spawn.looping) {
            state.credit += spawn.rate * (end - start);
            for burst in &spawn.bursts {
                if burst.time >= start && burst.time < end {
                    births = births.saturating_add(burst.count);
                }
            }
        }
        let whole = state.credit.floor();
        state.credit -= whole;
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "credit is a small non-negative count")]
        let whole = whole as u32;
        births = births.saturating_add(whole);
        let at = self.placement.0;
        for _ in 0..births {
            if !self.spawn(index, at) {
                break;
            }
        }
    }

    /// Gives birth to one particle of `emitter` where a trigger happened (`at`, plus the emitter's own origin). False when it is full.
    fn spawn(&mut self, index: usize, at: [f32; 3]) -> bool {
        let emitter = &self.effect.emitters[index];
        let state = &mut self.states[index];
        let free = state.slots.iter().position(|s| !s.alive);
        if free.is_none() && state.slots.len() >= emitter.max_particles {
            return false;
        }
        let lifetime = self.rng.between(emitter.lifetime);
        let speed = self.rng.between(emitter.speed);
        let turn = self.placement.1;
        let offset = emit::offset(&emitter.shape, &mut self.rng);
        let offset = rotate(turn, [
            emitter.origin[0] + offset[0],
            emitter.origin[1] + offset[1],
            emitter.origin[2] + offset[2],
        ]);
        let direction = rotate(turn, emit::direction(emitter.direction, emitter.cone, &mut self.rng));
        let position = [at[0] + offset[0], at[1] + offset[1], at[2] + offset[2]];
        let particle = Particle {
            id: state.next_id,
            position,
            velocity: direction.map(|c| c * speed),
            age: 0.0,
            lifetime,
        };
        state.next_id = state.next_id.wrapping_add(1);
        let slot = Slot {
            particle,
            alive: true,
            trail: if emitter.trail.is_some() { vec![position] } else { Vec::new() },
        };
        match free {
            Some(i) => state.slots[i] = slot,
            None => state.slots.push(slot),
        }
        for sub in emitter.sub.iter().filter(|s| s.on == SubTrigger::Birth) {
            self.pending.push((sub.emitter, position, sub.count));
        }
        true
    }

    /// Births the children triggers asked for (their own birth triggers are queued and handled too; the effect has no loops).
    fn flush(&mut self) {
        let mut guard = 0_u32;
        while let Some((emitter, at, count)) = self.pending.pop() {
            for _ in 0..count {
                if !self.spawn(emitter, at) {
                    break;
                }
            }
            guard += 1;
            if guard > 100_000 {
                self.pending.clear();
            }
        }
    }

    fn advance(&mut self, index: usize, world: &dyn World) {
        let emitter = &self.effect.emitters[index];
        let time = self.time;
        for slot in &mut self.states[index].slots {
            if !slot.alive {
                continue;
            }
            let p = &mut slot.particle;
            for force in &emitter.forces {
                p.velocity = forces::apply(force, p.position, p.velocity, STEP, time);
            }
            for axis in 0..3 {
                p.position[axis] += p.velocity[axis] * STEP;
            }
            if let Some(collide) = emitter.collide
                && let Some(contact) = world.contact(p.position, collide.radius)
            {
                let impact = collide_with(p, contact, collide.reaction);
                if matches!(collide.reaction, Reaction::Kill) {
                    slot.alive = false;
                }
                if impact || matches!(collide.reaction, Reaction::Kill) {
                    for sub in emitter.sub.iter().filter(|s| s.on == SubTrigger::Collide) {
                        self.pending.push((sub.emitter, p.position, sub.count));
                    }
                }
            }
            p.age += STEP;
            if p.age >= p.lifetime && slot.alive {
                slot.alive = false;
                for sub in emitter.sub.iter().filter(|s| s.on == SubTrigger::Death) {
                    self.pending.push((sub.emitter, p.position, sub.count));
                }
            }
            if slot.alive {
                record_trail(slot, emitter);
            }
        }
    }
}

/// Moves a particle out of the surface and applies the reaction. True for an impact (not resting contact).
fn collide_with(p: &mut Particle, contact: Contact, reaction: Reaction) -> bool {
    let n = contact.normal;
    for (c, d) in p.position.iter_mut().zip(n) {
        *c += d * contact.depth;
    }
    let vn = dot(p.velocity, n);
    if vn >= 0.0 {
        return false;
    }
    let impact = -vn > IMPACT;
    match reaction {
        Reaction::Kill => {}
        Reaction::Stick => p.velocity = [0.0; 3],
        Reaction::Bounce {
            restitution,
            friction,
        } => {
            let e = if impact { restitution } else { 0.0 };
            let j = -(1.0 + e) * vn;
            for (v, d) in p.velocity.iter_mut().zip(n) {
                *v += d * j;
            }
            // Coulomb friction: the slide loses at most `friction` times the normal impulse.
            let along = dot(p.velocity, n);
            let tangent = [
                p.velocity[0] - n[0] * along,
                p.velocity[1] - n[1] * along,
                p.velocity[2] - n[2] * along,
            ];
            let speed = dot(tangent, tangent).sqrt();
            if speed > 1.0e-6 {
                let lost = (friction * j).min(speed) / speed;
                for (v, t) in p.velocity.iter_mut().zip(tangent) {
                    *v -= t * lost;
                }
            }
        }
    }
    impact
}

/// Records a trail point once the particle has moved `min_step` from the last one, keeping the newest `points - 1` (the head is the particle).
fn record_trail(slot: &mut Slot, emitter: &Emitter) {
    let Some(trail) = emitter.trail else {
        return;
    };
    let moved = slot.trail.last().is_none_or(|last| distance(*last, slot.particle.position) >= trail.min_step);
    if moved {
        slot.trail.push(slot.particle.position);
        if slot.trail.len() > trail.points - 1 {
            slot.trail.remove(0);
        }
    }
}
