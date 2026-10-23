//! Global illumination in the viewer: the committed scene's triangles are traced by `ve-gi` a few probes per frame, and the
//! finished grid of light probes is handed to the renderer (see `ve-render`'s `probes.rs`).
use super::Uploaded;
use bevy_asset::Assets;
use bevy_ecs::system::ResMut;
use bevy_math::Mat4;
use bevy_mesh::{Mesh, VertexAttributeValues};
use std::cell::RefCell;
use ve_gi::{Baker, GridSpec, Material, SceneBuilder, Settings, Sky, Sun};
use ve_modeling::{LightProbes, SceneEnvironment};
use ve_render::{ProbeVolume, VeMaterial};

/// About this many probes along the scene's longest side when the author gives no spacing.
const PROBES_ACROSS: f32 = 24.0;
/// Time spent baking in one frame, in milliseconds.
const FRAME_MS: f64 = 4.0;

thread_local! { static BAKER: RefCell<Option<Baker>> = const { RefCell::new(None) }; }

/// Collects the geometry of a committed scene for baking.
pub(super) struct Collector {
    builder: SceneBuilder,
    settings: LightProbes,
    environment: SceneEnvironment,
    sun: Option<Sun>,
}

impl Collector {
    /// A collector for a scene whose environment asks for light probes, or `None` when it does not.
    pub(super) fn new(environment: Option<&SceneEnvironment>, sun: Option<Sun>) -> Option<Self> {
        let environment = environment?;
        Some(Self {
            builder: SceneBuilder::new(ve_gi::MAX_TRIANGLES),
            settings: environment.gi?,
            environment: environment.clone(),
            sun,
        })
    }

    /// Adds one uploaded part, once per placement when it is an instanced set. Clear glass and the like is left out so light
    /// passes through it.
    pub(super) fn add(
        &mut self,
        item: &Uploaded,
        surface: &VeMaterial,
        placements: Option<&[Mat4]>,
        meshes: &Assets<Mesh>,
    ) {
        if surface.opacity < 0.5 {
            return;
        }
        let Some(mesh) = meshes.get(&item.handle) else {
            return;
        };
        let Some(VertexAttributeValues::Float32x3(corners)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return;
        };
        let linear = surface.base_color.to_linear();
        let emitted = surface.emissive.to_linear();
        // A metal has no diffuse colour; keep a little so it still bounces something.
        let diffuse = 1.0 - 0.9 * surface.metallic;
        let material = self.builder.material(Material {
            albedo: [linear.red, linear.green, linear.blue].map(|v| v * diffuse),
            emissive: [emitted.red, emitted.green, emitted.blue],
        });
        let Some(placements) = placements else {
            self.builder.add(corners, None, material);
            return;
        };
        let remaining = ve_gi::MAX_TRIANGLES.saturating_sub(self.builder.triangle_count());
        let step = ve_gi::thinning(corners.len() / 3 * placements.len(), remaining);
        for matrix in placements.iter().step_by(step) {
            if !self
                .builder
                .add(corners, Some(&matrix.to_cols_array()), material)
            {
                break;
            }
        }
    }

    /// Starts baking what was collected, replacing any bake in progress. Nothing is baked for an empty scene.
    pub(super) fn start(self) {
        let Some((low, high)) = self.builder.bounds() else {
            return;
        };
        let longest = (0..3).map(|a| high[a] - low[a]).fold(0.0_f32, f32::max);
        let spacing = if self.settings.spacing > 0.0 {
            self.settings.spacing
        } else {
            (longest / PROBES_ACROSS).max(0.25)
        };
        let Ok(spec) = GridSpec::fit(low, high, spacing, 64) else {
            return;
        };
        let environment = &self.environment;
        let scale = environment.intensity;
        let colour = |c: [f32; 3]| {
            let linear = bevy_color::Color::srgb(c[0], c[1], c[2]).to_linear();
            [
                linear.red * scale,
                linear.green * scale,
                linear.blue * scale,
            ]
        };
        let zenith = colour(environment.sky_color);
        let ground = colour(environment.ground_color);
        let sky = Sky {
            zenith,
            horizon: std::array::from_fn(|i| 0.5 * (zenith[i] + ground[i])),
            ground,
        };
        let settings = Settings {
            rays_per_probe: self.settings.rays,
            bounces: self.settings.bounces,
        };
        let scene = self.builder.build(self.sun, sky);
        if let Ok(baker) = Baker::new(scene, spec, settings) {
            BAKER.with(|cell| *cell.borrow_mut() = Some(baker));
        }
    }
}

/// The sun a scene's lights give, as the bake wants it: the brightest sun light, colour times intensity.
pub(super) fn sun(lights: &[ve_modeling::SceneLight]) -> Option<Sun> {
    lights
        .iter()
        .filter(|light| light.kind == ve_modeling::SceneLightKind::Sun)
        .max_by(|a, b| a.intensity.total_cmp(&b.intensity))
        .map(|light| {
            let colour =
                bevy_color::Color::srgb(light.color[0], light.color[1], light.color[2]).to_linear();
            let toward = bevy_math::Vec3::from_array(light.direction)
                .normalize_or_zero()
                .to_array()
                .map(|v| -v);
            Sun {
                direction: toward,
                irradiance: [colour.red, colour.green, colour.blue].map(|v| v * light.intensity),
            }
        })
}

/// Drops the bake in progress and the published probes (a new scene was committed).
pub(super) fn clear(volume: &mut ProbeVolume) {
    BAKER.with(|cell| *cell.borrow_mut() = None);
    if *volume != ProbeVolume::default() {
        *volume = ProbeVolume::default();
    }
}

/// Bakes for a few milliseconds a frame and publishes the grid when it is done.
pub(crate) fn step(mut volume: ResMut<ProbeVolume>) {
    BAKER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(baker) = slot.as_mut() else {
            return;
        };
        let began = super::now();
        let mut done = false;
        while !done && super::now() - began < FRAME_MS {
            done = baker.step(8);
        }
        if !done {
            return;
        }
        let Some(baker) = slot.take() else {
            return;
        };
        let Ok(grid) = baker.finish() else {
            return;
        };
        *volume = ProbeVolume {
            origin: grid.spec.origin,
            spacing: grid.spec.spacing,
            dims: grid.spec.dims,
            coefficients: grid
                .probes
                .iter()
                .map(|p| {
                    let mut out = [0.0; 12];
                    out[0..3].copy_from_slice(&p.dc);
                    out[3..6].copy_from_slice(&p.x);
                    out[6..9].copy_from_slice(&p.y);
                    out[9..12].copy_from_slice(&p.z);
                    out
                })
                .collect(),
        };
    });
}

/// How far the running bake has got, 0 to 1; `None` when none is running.
pub(crate) fn progress() -> Option<f32> {
    BAKER.with(|cell| cell.borrow().as_ref().map(Baker::progress))
}
