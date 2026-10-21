//! A deterministic, pinned Verlet chain for secondary motion.

use thiserror::Error;

const MAX_SEGMENTS: usize = 64;
const MAX_ITERATIONS: usize = 32;
const MAX_SUBSTEP: f64 = 1.0 / 30.0;

/// Position and orientation of the pinned root. Rotation is an x, y, z, w quaternion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootPose {
    /// Root position in world space.
    pub position: [f64; 3],
    /// Root orientation as an x, y, z, w unit quaternion.
    pub rotation: [f64; 4],
}

/// Rest geometry and physical parameters for a chain.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainSpec {
    /// Segment offsets in the root's local frame.
    pub rest: Vec<[f64; 3]>,
    /// Per-second-step return strength in the range 0 through 1.
    pub stiffness: f64,
    /// Drag rate, in inverse seconds.
    pub damping: f64,
    /// World-space acceleration in metres per second squared.
    pub gravity: [f64; 3],
    /// Number of segment length-constraint passes per internal step.
    pub iterations: usize,
}

/// Errors found while constructing a chain.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum ChainError {
    /// No segments were supplied.
    #[error("chain must contain at least one segment")]
    Empty,
    /// A segment has no length.
    #[error("segment {index} has zero length")]
    ZeroLength {
        /// Index of the zero-length segment.
        index: usize,
    },
    /// A named input contains NaN or infinity.
    #[error("{0} must contain only finite numbers")]
    NotFinite(&'static str),
    /// The chain exceeds the supported segment count.
    #[error("chain exceeds the maximum of {max} segments")]
    TooLong {
        /// Maximum supported segment count.
        max: usize,
    },
    /// A parameter lies outside its supported range.
    #[error("{0} is outside its supported range")]
    OutOfRange(&'static str),
}

/// A root-pinned chain with Verlet state.
#[derive(Clone, Debug)]
pub struct Chain {
    spec: ChainSpec,
    lengths: Vec<f64>,
    points: Vec<[f64; 3]>,
    previous: Vec<[f64; 3]>,
    last_root: [f64; 3],
}

impl Chain {
    /// Creates a chain exactly in its rest pose at `root`.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the specification or initial pose is invalid.
    pub fn new(spec: ChainSpec, root: RootPose) -> Result<Self, ChainError> {
        validate(&spec, root)?;
        let rotation = normalized(root.rotation);
        let mut points = Vec::with_capacity(spec.rest.len() + 1);
        let mut lengths = Vec::with_capacity(spec.rest.len());
        let mut cursor = root.position;
        points.push(cursor);
        for offset in &spec.rest {
            lengths.push(magnitude(*offset));
            let world_offset = rotate(rotation, *offset);
            cursor = add(cursor, world_offset);
            points.push(cursor);
        }
        let previous = points.clone();
        Ok(Self {
            spec,
            lengths,
            points,
            previous,
            last_root: root.position,
        })
    }

    /// Advances the simulation. Positive time is split into up to eight steps of at most 1/30 s.
    /// Stiffness uses `1 - (1 - 0.05 * stiffness)^(60 * dt)` as the per-step rest-pose blend.
    /// Invalid time or root values are ignored because stepping is intentionally infallible.
    pub fn step(&mut self, root: RootPose, dt: f64) {
        if !dt.is_finite() || dt <= 0.0 || !finite3(root.position) || !finite4(root.rotation) {
            return;
        }
        let steps = substep_count(dt);
        let steps_f64 = f64::from(steps);
        let sub_dt = dt.min(MAX_SUBSTEP * 8.0) / steps_f64;
        let rotation = normalized(root.rotation);
        let start_root = self.last_root;
        for step in 0..steps {
            let fraction = f64::from(step + 1) / steps_f64;
            let step_root = add(start_root, scale(sub(root.position, start_root), fraction));
            let rest_points = self.rest_points(step_root, rotation);
            self.substep(step_root, &rest_points, sub_dt);
        }
        self.last_root = root.position;
    }

    /// Returns the root followed by each simulated segment endpoint.
    #[must_use]
    pub fn points(&self) -> &[[f64; 3]] {
        &self.points
    }

    fn rest_points(&self, position: [f64; 3], rotation: [f64; 4]) -> Vec<[f64; 3]> {
        let mut rest = Vec::with_capacity(self.points.len());
        let mut cursor = position;
        rest.push(cursor);
        for offset in &self.spec.rest {
            cursor = add(cursor, rotate(rotation, *offset));
            rest.push(cursor);
        }
        rest
    }

    fn substep(&mut self, root: [f64; 3], rest: &[[f64; 3]], dt: f64) {
        let drag = (-self.spec.damping * dt).exp();
        let blend = 1.0 - (1.0 - 0.05 * self.spec.stiffness).powf(60.0 * dt);
        for (index, rest_point) in rest.iter().enumerate().skip(1) {
            let current = self.points[index];
            let velocity = scale(sub(current, self.previous[index]), drag);
            self.previous[index] = current;
            let gravity = scale(self.spec.gravity, dt * dt);
            let pulled = add(current, scale(sub(*rest_point, current), blend));
            self.points[index] = add(add(pulled, velocity), gravity);
        }
        self.points[0] = root;
        for _ in 0..self.spec.iterations {
            self.points[0] = root;
            for index in 0..self.lengths.len() {
                let delta = sub(self.points[index + 1], self.points[index]);
                let distance = magnitude(delta);
                if distance > 0.0 {
                    self.points[index + 1] = add(
                        self.points[index],
                        scale(delta, self.lengths[index] / distance),
                    );
                }
            }
            for index in (0..self.lengths.len()).rev() {
                let delta = sub(self.points[index + 1], self.points[index]);
                let distance = magnitude(delta);
                if distance > 0.0 {
                    self.points[index + 1] = add(
                        self.points[index],
                        scale(delta, self.lengths[index] / distance),
                    );
                }
            }
        }
        self.points[0] = root;
    }
}

fn validate(spec: &ChainSpec, root: RootPose) -> Result<(), ChainError> {
    if spec.rest.is_empty() {
        return Err(ChainError::Empty);
    }
    if spec.rest.len() > MAX_SEGMENTS {
        return Err(ChainError::TooLong { max: MAX_SEGMENTS });
    }
    if !finite4(root.rotation) || !finite3(root.position) || !finite3(spec.gravity) {
        return Err(ChainError::NotFinite("root or gravity"));
    }
    if !spec.stiffness.is_finite()
        || !spec.damping.is_finite()
        || spec.rest.iter().any(|v| !finite3(*v))
    {
        return Err(ChainError::NotFinite("chain specification"));
    }
    for (index, offset) in spec.rest.iter().enumerate() {
        if magnitude(*offset) == 0.0 {
            return Err(ChainError::ZeroLength { index });
        }
    }
    if spec.stiffness < 0.0 || spec.stiffness > 1.0 {
        return Err(ChainError::OutOfRange("stiffness"));
    }
    if spec.damping < 0.0 {
        return Err(ChainError::OutOfRange("damping"));
    }
    if !(1..=MAX_ITERATIONS).contains(&spec.iterations) {
        return Err(ChainError::OutOfRange("iterations"));
    }
    if magnitude4(root.rotation) == 0.0 {
        return Err(ChainError::OutOfRange("rotation"));
    }
    Ok(())
}

fn substep_count(dt: f64) -> u32 {
    if dt >= MAX_SUBSTEP * 8.0 {
        8
    } else if dt >= MAX_SUBSTEP * 7.0 {
        7
    } else if dt >= MAX_SUBSTEP * 6.0 {
        6
    } else if dt >= MAX_SUBSTEP * 5.0 {
        5
    } else if dt >= MAX_SUBSTEP * 4.0 {
        4
    } else if dt >= MAX_SUBSTEP * 3.0 {
        3
    } else if dt >= MAX_SUBSTEP * 2.0 {
        2
    } else {
        1
    }
}

fn normalized(q: [f64; 4]) -> [f64; 4] {
    let norm = magnitude4(q);
    [q[0] / norm, q[1] / norm, q[2] / norm, q[3] / norm]
}

fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let u = [q[0], q[1], q[2]];
    let twice_cross = scale(cross(u, v), 2.0);
    add(v, add(scale(twice_cross, q[3]), cross(u, twice_cross)))
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(v: [f64; 3], factor: f64) -> [f64; 3] {
    [v[0] * factor, v[1] * factor, v[2] * factor]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn magnitude(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}
fn magnitude4(v: [f64; 4]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt()
}
fn finite3(v: [f64; 3]) -> bool {
    v.iter().all(|n| n.is_finite())
}
fn finite4(v: [f64; 4]) -> bool {
    v.iter().all(|n| n.is_finite())
}

#[cfg(test)]
mod tests {
    use super::rotate;

    #[test]
    fn quaternion_rotation_handles_identity_quarter_turn_and_inverse() {
        let vector = [0.0, 0.0, -1.0];
        let identity = [0.0, 0.0, 0.0, 1.0];
        let unchanged = rotate(identity, vector);
        assert!(unchanged[0].abs() < 1e-12);
        assert!(unchanged[1].abs() < 1e-12);
        assert!((unchanged[2] + 1.0).abs() < 1e-12);
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let quarter = [0.0, half, 0.0, half];
        let turned = rotate(quarter, vector);
        assert!(
            (turned[0] + 1.0).abs() < 1e-12 && turned[1].abs() < 1e-12 && turned[2].abs() < 1e-12
        );
        let inverse = [0.0, -half, 0.0, half];
        let restored = rotate(inverse, turned);
        assert!((restored[0] - vector[0]).abs() < 1e-12);
        assert!((restored[1] - vector[1]).abs() < 1e-12);
        assert!((restored[2] - vector[2]).abs() < 1e-12);
    }
}
