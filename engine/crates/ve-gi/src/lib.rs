//! Deterministic CPU diffuse irradiance probe baking: a grid of light probes, each holding the irradiance arriving from every
//! direction as first-order spherical harmonics, traced against a triangle scene with a sun, a gradient sky and emissive
//! surfaces, with a few bounces between surfaces.

use std::f32::consts::PI;
use thiserror::Error;

mod ambient_cube;
mod baker;
mod builder;
mod sh;

pub use ambient_cube::{AmbientCubeVolume, SIDES, ambient_cube, volume_placement, volume_texels};
pub use baker::{Baker, bake};
pub use builder::{MAX_TRIANGLES, SceneBuilder, thinning};
use sh::{eval, fix_invalid, project, sky_irradiance, sky_radiance};

/// A surface's linear-colour response.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// Diffuse reflectance.
    pub albedo: [f32; 3],
    /// Emitted radiance.
    pub emissive: [f32; 3],
}

/// Directional sunlight, pointing from a surface toward the sun.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    /// Unit direction toward the sun.
    pub direction: [f32; 3],
    /// Surface irradiance facing the sun.
    pub irradiance: [f32; 3],
}

/// Environment radiance at zenith, horizon and nadir.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// Upward radiance.
    pub zenith: [f32; 3],
    /// Horizon radiance.
    pub horizon: [f32; 3],
    /// Downward radiance.
    pub ground: [f32; 3],
}

/// Triangle soup and its lighting.
#[derive(Clone, Debug)]
pub struct Scene {
    /// World-space vertices.
    pub positions: Vec<[f32; 3]>,
    /// Triangle vertex indices.
    pub indices: Vec<u32>,
    /// Material index per triangle.
    pub triangle_material: Vec<u32>,
    /// Surface materials.
    pub materials: Vec<Material>,
    /// Optional sun.
    pub sun: Option<Sun>,
    /// Environment.
    pub sky: Sky,
}

/// Uniform axis-aligned probe grid: probe (i, j, k) is at `origin + spacing * (i, j, k)` and has index
/// `i + dims[0] * (j + dims[1] * k)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSpec {
    /// Position of probe zero.
    pub origin: [f32; 3],
    /// Cell width.
    pub spacing: f32,
    /// Probe counts.
    pub dims: [u32; 3],
}

impl GridSpec {
    /// Total number of probes.
    #[must_use]
    pub fn probe_count(&self) -> usize {
        self.dims.iter().map(|&v| v as usize).product()
    }

    /// Position of the indexed probe.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "a probe's cell coordinate is at most 65 535"
    )]
    pub fn position(&self, index: usize) -> [f32; 3] {
        let x = index % self.dims[0] as usize;
        let y = index / self.dims[0] as usize % self.dims[1] as usize;
        let z = index / (self.dims[0] as usize * self.dims[1] as usize);
        [
            self.origin[0] + x as f32 * self.spacing,
            self.origin[1] + y as f32 * self.spacing,
            self.origin[2] + z as f32 * self.spacing,
        ]
    }

    /// A grid covering the box `min..max` with about `target_spacing` between probes, padded by one cell on every side, with at
    /// most `max_per_axis` (4 to 64) probes along an axis and 65 536 in all: the spacing grows until it fits.
    ///
    /// # Errors
    /// [`GiError::BadGrid`] when a coordinate is not finite, `min` is above `max`, the spacing is not positive, or
    /// `max_per_axis` is outside 4 to 64.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the cell counts are finite and not negative, and are limited right after"
    )]
    pub fn fit(
        min: [f32; 3],
        max: [f32; 3],
        target_spacing: f32,
        max_per_axis: u32,
    ) -> Result<Self, GiError> {
        if !min.iter().chain(max.iter()).all(|v| v.is_finite())
            || min.iter().zip(max).any(|(a, b)| *a > b)
            || !target_spacing.is_finite()
            || target_spacing <= 0.0
            || !(4..=64).contains(&max_per_axis)
        {
            return Err(GiError::BadGrid);
        }
        let mut spacing = target_spacing;
        let dims_for = |spacing: f32| -> [u32; 3] {
            std::array::from_fn(|i| (((max[i] - min[i]) / spacing).ceil() as u32).saturating_add(3))
        };
        let mut dims = dims_for(spacing);
        while dims.iter().any(|&d| d > max_per_axis)
            || dims.iter().map(|&d| d as usize).product::<usize>() > 65536
        {
            spacing *= 1.05;
            dims = dims_for(spacing);
            if !spacing.is_finite() {
                return Err(GiError::BadGrid);
            }
        }
        Ok(Self {
            origin: std::array::from_fn(|i| min[i] - spacing),
            spacing,
            dims,
        })
    }
}

/// First-order irradiance-over-pi coefficients: for a unit normal `n`, the light a probe gives a surface facing it is
/// `max(dc + x * n.x + y * n.y + z * n.z, 0)` per colour channel. A uniform sky of radiance L gives `dc = L` and no linear
/// terms, and a diffuse surface's outgoing radiance is its albedo times this.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProbeSh {
    /// Constant term.
    pub dc: [f32; 3],
    /// X directional term.
    pub x: [f32; 3],
    /// Y directional term.
    pub y: [f32; 3],
    /// Z directional term.
    pub z: [f32; 3],
}

/// Baked probes and validity mask.
#[derive(Clone, Debug)]
pub struct ProbeGrid {
    /// Grid layout.
    pub spec: GridSpec,
    /// Probe coefficients.
    pub probes: Vec<ProbeSh>,
    /// Whether each probe was outside solids.
    pub valid: Vec<bool>,
    sky: [f32; 3],
}

impl ProbeGrid {
    /// Construct a grid from coefficients, validity flags and the fallback sky irradiance.
    #[must_use]
    pub fn from_parts(
        spec: GridSpec,
        probes: Vec<ProbeSh>,
        valid: Vec<bool>,
        sky: [f32; 3],
    ) -> Self {
        Self {
            spec,
            probes,
            valid,
            sky,
        }
    }

    /// Trilinear irradiance lookup, clamped to the grid. Invalid probes carry no weight.
    #[must_use]
    pub fn sample(&self, point: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
        sample_grid(
            &self.spec,
            &self.probes,
            Some(&self.valid),
            self.sky,
            point,
            normal,
        )
    }
}

/// The trilinear lookup over borrowed probes: the eight probes around `point`, each weighted by its trilinear weight (none
/// for a probe `valid` rules out), evaluated for `normal`. With no usable probe it returns `fallback`. The point is clamped
/// to the grid.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "grid coordinates are clamped to the grid, whose axes hold at most 65 536 probes"
)]
pub(crate) fn sample_grid(
    spec: &GridSpec,
    probes: &[ProbeSh],
    valid: Option<&[bool]>,
    fallback: [f32; 3],
    point: [f32; 3],
    normal: [f32; 3],
) -> [f32; 3] {
    let cell = std::array::from_fn::<_, 3, _>(|axis| {
        ((point[axis] - spec.origin[axis]) / spec.spacing).clamp(0.0, (spec.dims[axis] - 1) as f32)
    });
    let low = cell.map(|v| v.floor() as u32);
    let fraction = std::array::from_fn::<_, 3, _>(|axis| cell[axis] - low[axis] as f32);
    let mut sum = [0.0; 3];
    let mut total = 0.0;
    for corner in 0..8_u32 {
        let step = std::array::from_fn::<_, 3, _>(|axis| (corner >> axis) & 1);
        let at = std::array::from_fn::<_, 3, _>(|axis| {
            (low[axis] + step[axis]).min(spec.dims[axis] - 1)
        });
        let index = at[0] as usize
            + spec.dims[0] as usize * (at[1] as usize + spec.dims[1] as usize * at[2] as usize);
        let weight = (0..3)
            .map(|axis| {
                if step[axis] == 0 {
                    1.0 - fraction[axis]
                } else {
                    fraction[axis]
                }
            })
            .product::<f32>();
        if valid.is_none_or(|valid| valid[index]) {
            let value = eval(probes[index], normal);
            for (total_channel, channel) in sum.iter_mut().zip(value) {
                *total_channel += channel * weight;
            }
            total += weight;
        }
    }
    if total == 0.0 {
        return fallback;
    }
    sum.map(|v| (v / total).max(0.0))
}

/// Bake settings.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Rays per probe (default 128).
    pub rays_per_probe: u32,
    /// Number of transport passes (default 2).
    pub bounces: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rays_per_probe: 128,
            bounces: 2,
        }
    }
}

/// Invalid bake input.
#[derive(Debug, Error)]
pub enum GiError {
    /// Invalid grid dimensions or spacing.
    #[error("invalid grid")]
    BadGrid,
    /// Invalid scene data.
    #[error("bad scene: {0}")]
    BadScene(String),
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

fn norm(a: [f32; 3]) -> [f32; 3] {
    mul(a, 1.0 / length(a))
}

fn mul(a: [f32; 3], s: f32) -> [f32; 3] {
    a.map(|v| v * s)
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}

fn neg(a: [f32; 3]) -> [f32; 3] {
    mul(a, -1.0)
}
