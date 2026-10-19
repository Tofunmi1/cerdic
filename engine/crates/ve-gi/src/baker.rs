//! The incremental baker: passes over the probe grid, each tracing every probe against the previous pass's result.

use crate::{
    GiError, GridSpec, PI, ProbeGrid, ProbeSh, Scene, Settings, add, fix_invalid, length, mul, neg,
    norm, project, sample_grid, sky_irradiance, sky_radiance,
};
use std::f32::consts::TAU;
use ve_measure::{Bvh, Mesh};

/// How close behind a back face (in grid spacings) a ray must hit for the probe to count as inside a solid.
const INSIDE_RANGE: f32 = 2.0;
/// The distance a shadow ray starts above the surface it leaves, in metres.
const SHADOW_BIAS: f32 = 1.0e-3;

/// Incremental deterministic baker: call [`Baker::step`] until it returns true, then [`Baker::finish`].
pub struct Baker {
    scene: Scene,
    spec: GridSpec,
    settings: Settings,
    bvh: Option<Bvh>,
    pass: u32,
    cursor: usize,
    /// The result of the last finished pass (the open sky before the first), which surfaces take their indirect light from.
    previous: Vec<ProbeSh>,
    /// The pass being traced.
    current: Vec<ProbeSh>,
    valid: Vec<bool>,
    dirs: Vec<[f32; 3]>,
    /// The sky's average radiance, the lookup's answer where no probe is usable.
    sky: [f32; 3],
    /// The open sky's irradiance as a probe, what an enclosed invalid probe falls back to.
    open_sky: ProbeSh,
}

/// Whether the inputs describe a bake that can run.
fn validate(scene: &Scene, spec: &GridSpec, settings: Settings) -> Result<(), GiError> {
    let finite = |values: &[f32]| values.iter().all(|v| v.is_finite());
    let bad_grid = spec.dims.contains(&0)
        || !spec.spacing.is_finite()
        || spec.spacing <= 0.0
        || !finite(&spec.origin);
    let bad_scene = !scene.indices.len().is_multiple_of(3)
        || scene.triangle_material.len() != scene.indices.len() / 3
        || scene
            .triangle_material
            .iter()
            .any(|&m| m as usize >= scene.materials.len())
        || scene.positions.iter().any(|p| !finite(p))
        || scene
            .indices
            .iter()
            .any(|&i| i as usize >= scene.positions.len())
        || scene
            .materials
            .iter()
            .any(|m| !finite(&m.albedo) || !finite(&m.emissive));
    if bad_grid || bad_scene || settings.rays_per_probe == 0 {
        return Err(GiError::BadScene(
            "invalid geometry, material, grid or settings".into(),
        ));
    }
    if let Some(sun) = scene.sun
        && (!finite(&sun.direction) || !finite(&sun.irradiance) || length(sun.direction) < 1e-8)
    {
        return Err(GiError::BadScene("invalid sun".into()));
    }
    Ok(())
}

/// `count` unit directions spread evenly over the sphere (a spherical Fibonacci set), the same for every probe.
#[expect(
    clippy::cast_precision_loss,
    reason = "a ray count is far below 2^23 and indices are exact in f32 up to that"
)]
pub(crate) fn sphere_directions(count: u32) -> Vec<[f32; 3]> {
    let golden = (1.0 + 5.0_f32.sqrt()) * 0.5;
    (0..count)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
            let ring = (1.0 - y * y).sqrt();
            let angle = TAU * i as f32 / golden;
            [ring * angle.cos(), y, ring * angle.sin()]
        })
        .collect()
}

impl Baker {
    /// Validates the inputs and prepares the first pass.
    ///
    /// # Errors
    /// [`GiError::BadScene`] when the grid, geometry, materials, sun or settings are malformed.
    pub fn new(scene: Scene, spec: GridSpec, settings: Settings) -> Result<Self, GiError> {
        validate(&scene, &spec, settings)?;
        let bvh = if scene.indices.is_empty() {
            None
        } else {
            let mesh = Mesh::new(&scene.positions, &scene.indices);
            Some(Bvh::build(&mesh).map_err(|e| GiError::BadScene(e.to_string()))?)
        };
        let dirs = sphere_directions(settings.rays_per_probe);
        // Before any pass every probe holds the open sky's own irradiance (gradient included), so the first bounce sees sky
        // light at surfaces.
        let open_sky: Vec<_> = dirs
            .iter()
            .map(|d| (*d, sky_radiance(scene.sky, *d)))
            .collect();
        let count = spec.probe_count();
        let open_sky_probe = project(&open_sky);
        Ok(Self {
            sky: sky_irradiance(scene.sky),
            open_sky: open_sky_probe,
            previous: vec![open_sky_probe; count],
            current: vec![ProbeSh::default(); count],
            valid: vec![true; count],
            scene,
            spec,
            settings,
            bvh,
            pass: 0,
            cursor: 0,
            dirs,
        })
    }

    /// Traces at most `max_probes` probes of the pass in progress; true once every pass is done.
    pub fn step(&mut self, max_probes: usize) -> bool {
        let count = self.spec.probe_count();
        if self.pass >= self.settings.bounces {
            return true;
        }
        let end = self.cursor.saturating_add(max_probes).min(count);
        while self.cursor < end {
            let index = self.cursor;
            let (sh, valid) = self.trace(index);
            self.current[index] = sh;
            self.valid[index] = valid;
            self.cursor += 1;
        }
        if self.cursor == count {
            fix_invalid(&mut self.current, &self.valid, self.spec, self.open_sky);
            std::mem::swap(&mut self.previous, &mut self.current);
            self.pass += 1;
            self.cursor = 0;
        }
        self.pass >= self.settings.bounces
    }

    /// The fraction of all passes done, from 0 to 1.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "probe counts and passes are far below 2^23"
    )]
    pub fn progress(&self) -> f32 {
        (self.pass as f32 + self.cursor as f32 / self.spec.probe_count() as f32)
            / self.settings.bounces.max(1) as f32
    }

    /// Hands over the finished grid.
    ///
    /// # Errors
    /// [`GiError::BadScene`] when called before [`Baker::step`] has returned true.
    pub fn finish(self) -> Result<ProbeGrid, GiError> {
        if self.pass < self.settings.bounces {
            return Err(GiError::BadScene("bake is incomplete".into()));
        }
        Ok(ProbeGrid {
            spec: self.spec,
            probes: self.previous,
            valid: self.valid,
            sky: self.sky,
        })
    }

    /// The radiance leaving the surface a ray hit, back toward the ray's origin: its emission, plus its albedo times the sun and
    /// the light the previous pass left there.
    fn leaving(&self, mesh: &Mesh, hit: ve_measure::RayHit, facing: [f32; 3]) -> [f32; 3] {
        let material =
            self.scene.materials[self.scene.triangle_material[hit.triangle as usize] as usize];
        let direct = self.scene.sun.map_or([0.0; 3], |sun| {
            let toward = norm(sun.direction);
            let lit = crate::dot(facing, toward);
            let start = add(hit.point, mul(facing, SHADOW_BIAS));
            let blocked = self
                .bvh
                .as_ref()
                .and_then(|bvh| bvh.raycast(mesh, start, toward))
                .is_some();
            if lit <= 0.0 || blocked {
                [0.0; 3]
            } else {
                sun.irradiance.map(|e| e * lit)
            }
        });
        let indirect = sample_grid(
            &self.spec,
            &self.previous,
            None,
            self.sky,
            add(hit.point, mul(facing, self.spec.spacing * 0.5)),
            facing,
        );
        // The sun's irradiance is divided by pi to give radiance; the grid already holds irradiance over pi.
        std::array::from_fn(|c| {
            material.emissive[c] + material.albedo[c] * (direct[c] / PI + indirect[c])
        })
    }

    /// One probe's irradiance from all directions, and whether it sits outside every solid.
    fn trace(&self, index: usize) -> (ProbeSh, bool) {
        let origin = self.spec.position(index);
        let mesh = Mesh::new(&self.scene.positions, &self.scene.indices);
        let mut samples = Vec::with_capacity(self.dirs.len());
        let mut inside = 0;
        for dir in &self.dirs {
            let hit = self
                .bvh
                .as_ref()
                .and_then(|bvh| bvh.raycast(&mesh, origin, *dir));
            let radiance = if let Some(hit) = hit {
                let raw = norm(hit.normal);
                let toward_ray = crate::dot(raw, *dir) < 0.0;
                // A back face close by means the probe is inside a solid.
                if !toward_ray && hit.distance < INSIDE_RANGE * self.spec.spacing {
                    inside += 1;
                }
                let facing = if toward_ray { raw } else { neg(raw) };
                self.leaving(&mesh, hit, facing)
            } else {
                sky_radiance(self.scene.sky, *dir)
            };
            samples.push((*dir, radiance));
        }
        (project(&samples), inside * 4 <= self.dirs.len())
    }
}

/// Bakes every probe in one call.
///
/// # Errors
/// As [`Baker::new`].
pub fn bake(scene: Scene, spec: GridSpec, settings: Settings) -> Result<ProbeGrid, GiError> {
    let mut baker = Baker::new(scene, spec, settings)?;
    while !baker.step(usize::MAX) {}
    baker.finish()
}
