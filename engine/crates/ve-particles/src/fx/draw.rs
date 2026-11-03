//! The geometry an effect is drawn with: soft camera-facing sprites and ribbons along trails, as plain vertex arrays a renderer uploads.
//!
//! No texture is needed: a sprite is a centre vertex at the particle's opacity fanned out to a ring at zero opacity, so the vertex
//! colours alone give a soft round edge. Colours are linear; `glow` multiplies the colour above 1 for bloom to pick up.
use super::{Blend, Effect, Particle, System};

/// Where the viewer is: a position and the directions that are right and up on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// World position.
    pub position: [f32; 3],
    /// Unit vector to the right on screen.
    pub right: [f32; 3],
    /// Unit vector up on screen.
    pub up: [f32; 3],
}

/// Vertex arrays for one draw: positions, linear RGBA colours and triangle indices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch {
    /// Vertex positions, world space.
    pub positions: Vec<[f32; 3]>,
    /// Linear RGBA per vertex.
    pub colours: Vec<[f32; 4]>,
    /// Unit normal per vertex: towards the viewer for sprites and ribbons (they are drawn flat), outward for cubes (which are lit).
    pub normals: Vec<[f32; 3]>,
    /// Triangles, three indices each.
    pub indices: Vec<u32>,
}

type V = [f32; 3];

/// The unit circle in eight steps (cosine, sine).
const RING: [(f32, f32); 8] = [
    (1.0, 0.0),
    (std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2),
    (0.0, 1.0),
    (-std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2),
    (-1.0, 0.0),
    (-std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2),
    (0.0, -1.0),
    (std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2),
];

/// A sprite reaches this far past half its size, so the fading rim still reads as the size asked for.
const REACH: f32 = 0.75;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn finite(v: V) -> bool {
    v.iter().all(|c| c.is_finite())
}

fn camera_ok(camera: &Camera) -> bool {
    finite(camera.position) && finite(camera.right) && finite(camera.up)
}

fn dot(a: V, b: V) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A particle's size, colour (with glow applied) and opacity at its age.
fn appearance(effect: &Effect, emitter: usize, particle: &Particle) -> (f32, [f32; 3], f32) {
    let look = &effect.emitters[emitter].look;
    let age = (particle.age / particle.lifetime).clamp(0.0, 1.0);
    let boost = 1.0 + look.glow;
    let colour = [0, 1, 2].map(|c| lerp(look.colour[0][c], look.colour[1][c], age) * boost);
    (lerp(look.size[0], look.size[1], age), colour, lerp(look.alpha[0], look.alpha[1], age))
}

/// One soft round sprite per live particle of `emitter`, facing the camera; far to near when the emitter blends with alpha.
#[must_use]
pub fn sprites(system: &System, effect: &Effect, emitter: usize, camera: &Camera) -> Batch {
    let mut batch = Batch::default();
    if emitter >= effect.emitters.len() || !camera_ok(camera) {
        return batch;
    }
    let mut particles = system.alive(emitter);
    if effect.emitters[emitter].look.blend == Blend::Alpha {
        // Sprites are flat and face the screen, so what must be right is their depth along the view direction (which is -(right x up)).
        let toward_viewer = cross(camera.right, camera.up);
        let depth = |p: V| -dot([p[0] - camera.position[0], p[1] - camera.position[1], p[2] - camera.position[2]], toward_viewer);
        particles.sort_by(|a, b| depth(b.position).total_cmp(&depth(a.position)));
    }
    let toward_viewer = unit(cross(camera.right, camera.up));
    for particle in &particles {
        let (size, colour, alpha) = appearance(effect, emitter, particle);
        let reach = size * REACH;
        let base = u32::try_from(batch.positions.len()).unwrap_or(u32::MAX);
        batch.positions.push(particle.position);
        batch.colours.push([colour[0], colour[1], colour[2], alpha]);
        batch.normals.push(toward_viewer);
        for (cosine, sine) in RING {
            batch.positions.push([0, 1, 2].map(|a| {
                particle.position[a] + (camera.right[a] * cosine + camera.up[a] * sine) * reach
            }));
            batch.colours.push([colour[0], colour[1], colour[2], 0.0]);
            batch.normals.push(toward_viewer);
        }
        for step in 0..8_u32 {
            batch.indices.extend([base, base + 1 + step, base + 1 + (step + 1) % 8]);
        }
    }
    batch
}

fn normalise(v: V) -> Option<V> {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (length.is_finite() && length > 1.0e-9).then(|| v.map(|c| c / length))
}

fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A ribbon along each live particle's trail, wide at the head and thin and clear at the tail, turned to face the camera.
#[must_use]
pub fn ribbons(system: &System, effect: &Effect, emitter: usize, camera: &Camera) -> Batch {
    let mut batch = Batch::default();
    if emitter >= effect.emitters.len() || !camera_ok(camera) {
        return batch;
    }
    for (particle, points) in system.alive_with_trails(emitter) {
        let count = points.len();
        if count < 2 {
            continue;
        }
        let (width, colour, alpha) = appearance(effect, emitter, &particle);
        let base = u32::try_from(batch.positions.len()).unwrap_or(u32::MAX);
        for (i, point) in points.iter().enumerate() {
            let (before, after) = (points[i.saturating_sub(1)], points[(i + 1).min(count - 1)]);
            let along = [after[0] - before[0], after[1] - before[1], after[2] - before[2]];
            let to_camera = [
                camera.position[0] - point[0],
                camera.position[1] - point[1],
                camera.position[2] - point[2],
            ];
            // Sideways is perpendicular to the path and to the view, so the strip shows its face.
            let side = normalise(cross(along, to_camera)).unwrap_or(camera.right);
            #[expect(clippy::cast_precision_loss, reason = "a trail has at most 64 points")]
            let along_trail = i as f32 / (count - 1) as f32;
            let half = width * 0.5 * along_trail;
            for sign in [-1.0_f32, 1.0] {
                batch.positions.push([0, 1, 2].map(|a| point[a] + side[a] * half * sign));
                batch.colours.push([colour[0], colour[1], colour[2], alpha * along_trail]);
                batch.normals.push(unit(to_camera));
            }
        }
        for segment in 0..u32::try_from(count - 1).unwrap_or(0) {
            let (a, b) = (base + segment * 2, base + segment * 2 + 2);
            batch.indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    batch
}

/// A unit vector in the direction of `v`, or +Z when it has none.
fn unit(v: V) -> V {
    normalise(v).unwrap_or([0.0, 0.0, 1.0])
}

/// The faces of a cube as (outward normal, two edge directions).
const FACES: [(V, V, V); 6] = [
    ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
    ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
    ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
    ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
];

/// Rotates `v` by `angle` about the unit `axis` (Rodrigues).
fn turn(v: V, axis: V, angle: f32) -> V {
    let (sine, cosine) = angle.sin_cos();
    let along = dot(axis, v) * (1.0 - cosine);
    let across = cross(axis, v);
    [0, 1, 2].map(|a| v[a] * cosine + across[a] * sine + axis[a] * along)
}

/// One lit box per live particle of `emitter`: its size and colour follow its age, and it tumbles at the look's `spin` about an axis that is its own
/// (taken from its id, so every piece turns differently and the same piece always turns the same way). Colour is the vertex colour; the normals are
/// outward, so a renderer's lights shade the faces.
#[must_use]
pub fn cubes(system: &System, effect: &Effect, emitter: usize) -> Batch {
    let mut batch = Batch::default();
    if emitter >= effect.emitters.len() {
        return batch;
    }
    let spin = effect.emitters[emitter].look.spin;
    for particle in system.alive(emitter) {
        let (size, colour, alpha) = appearance(effect, emitter, &particle);
        // A distinct, stable axis and starting angle from the id (golden-ratio steps spread them evenly).
        let id = f32::from(u16::try_from(particle.id % 4096).unwrap_or(0));
        let seed = |step: f32| (id * step).fract() * 2.0 - 1.0;
        let axis = unit([seed(0.754_877_7), seed(0.569_840_3), seed(0.430_159_7) + 0.1]);
        let angle = seed(0.618_034) * std::f32::consts::PI + particle.age * spin;
        let half = size * 0.5;
        for (normal, u, v) in FACES {
            let base = u32::try_from(batch.positions.len()).unwrap_or(u32::MAX);
            let world_normal = turn(normal, axis, angle);
            for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = [0, 1, 2].map(|a| (normal[a] + u[a] * su + v[a] * sv) * half);
                let turned = turn(corner, axis, angle);
                batch.positions.push([0, 1, 2].map(|a| particle.position[a] + turned[a]));
                batch.colours.push([colour[0], colour[1], colour[2], alpha]);
                batch.normals.push(world_normal);
            }
            // Wound so the face looks outward whichever way the edge directions run.
            let flip = dot(cross(u, v), normal) < 0.0;
            if flip {
                batch.indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
            } else {
                batch.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }
    batch
}
