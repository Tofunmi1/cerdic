//! Cached per-material water and terrain uniforms. Ordinary draws bind the shared default.

use std::collections::{HashMap, HashSet};

use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntries, PipelineCache, ShaderType, UniformBuffer,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};

use crate::water_waves::{GRAVITY, WAVES, Wave, WaveField, finite_or};
use crate::{SurfaceWater, pipeline::ForwardPipelines, prepare::InstanceBatches};

const DETAIL_WAVES: usize = 4;

/// GPU-ready spatial frequency, amplitude, phase and angular speed of one CPU wave.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub(crate) struct GpuWave {
    pub(crate) spatial: Vec4,
    pub(crate) temporal: Vec4,
}

impl GpuWave {
    fn new(wave: &Wave, flow: f32) -> Self {
        let k = std::f32::consts::TAU / wave.wavelength;
        let (z, x) = wave.direction_deg.to_radians().sin_cos();
        Self {
            spatial: Vec4::new(k * x, k * z, wave.amplitude, wave.phase),
            temporal: Vec4::new((GRAVITY * k).sqrt() * flow, 0.0, 0.0, 0.0),
        }
    }
}

/// One uniform per distinct water/terrain look, shared by all its instances and views.
#[derive(Clone, Debug, ShaderType)]
pub(crate) struct WaterUniform {
    pub(crate) sim_bounds: Vec4,
    pub(crate) sim_enabled: Vec4,
    pub(crate) waves: [GpuWave; WAVES],
    pub(crate) detail: [GpuWave; DETAIL_WAVES],
    pub(crate) shallow: Vec4,
    pub(crate) deep: Vec4,
    /// Flow, ripple and foam; the fourth lane is reserved.
    pub(crate) params: Vec4,
    pub(crate) terrain: crate::surface_terrain::TerrainUniform,
    pub(crate) grass: crate::grass_gpu::GrassUniform,
}

impl WaterUniform {
    pub(crate) fn new(water: SurfaceWater) -> Self {
        let flow = finite_or(water.flow, 1.0).clamp(0.0, 4.0);
        let field = WaveField::new(water.seed, water.ripple, flow, water.direction);
        let detail = WaveField::new(
            water.seed ^ 0x68bc_21eb,
            water.ripple,
            flow,
            water.direction + 37.0,
        );
        let mut waves = [GpuWave::default(); WAVES];
        for (slot, wave) in waves.iter_mut().zip(field.waves()) {
            *slot = GpuWave::new(wave, flow);
        }
        let mut small = [GpuWave::default(); DETAIL_WAVES];
        for (slot, wave) in small.iter_mut().zip(detail.waves()) {
            let mut wave = *wave;
            wave.wavelength *= 0.4;
            wave.amplitude *= 0.18;
            *slot = GpuWave::new(&wave, flow);
        }
        let linear = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]).to_linear();
        let (shallow, deep) = (linear(water.shallow), linear(water.deep));
        Self {
            terrain: crate::surface_terrain::TerrainUniform::default(),
            grass: crate::grass_gpu::GrassUniform::default(),
            sim_bounds: Vec4::from_array(water.sim_bounds),
            sim_enabled: Vec4::new(if water.sim_id != 0 { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0),
            waves,
            detail: small,
            shallow: Vec4::new(
                shallow.red,
                shallow.green,
                shallow.blue,
                water.clarity.max(0.05),
            ),
            deep: Vec4::new(deep.red, deep.green, deep.blue, 0.0),
            params: Vec4::new(
                flow,
                finite_or(water.ripple, 1.0).clamp(0.0, 2.0),
                water.foam.clamp(0.0, 1.0),
                0.0,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct WaterKey {
    numbers: [u32; 11],
    seed: u32,
    sim_id: u64,
    sim_bounds: [u32; 4],
}

impl From<SurfaceWater> for WaterKey {
    fn from(w: SurfaceWater) -> Self {
        Self {
            numbers: [
                w.shallow[0],
                w.shallow[1],
                w.shallow[2],
                w.deep[0],
                w.deep[1],
                w.deep[2],
                w.clarity,
                w.flow,
                w.ripple,
                w.foam,
                w.direction,
            ]
            .map(f32::to_bits),
            seed: w.seed,
            sim_id: w.sim_id,
            sim_bounds: w.sim_bounds.map(f32::to_bits),
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct WaterGpu {
    materials: HashMap<
        (WaterKey, [u32; 28], [u32; 12]),
        (UniformBuffer<WaterUniform>, BindGroup, Option<BindGroup>),
    >,
}

impl WaterGpu {
    pub(crate) fn forward_binding(
        &self,
        water: Option<SurfaceWater>,
        terrain: Option<crate::SurfaceTerrain>,
        grass: crate::grass_gpu::GrassUniform,
    ) -> Option<&BindGroup> {
        self.materials
            .get(&(
                water.unwrap_or_default().into(),
                crate::surface_terrain::TerrainUniform::new(terrain).key(),
                grass.key(),
            ))
            .map(|(_, group, sim)| sim.as_ref().unwrap_or(group))
    }

    pub(crate) fn binding(
        &self,
        water: Option<SurfaceWater>,
        terrain: Option<crate::SurfaceTerrain>,
        grass: crate::grass_gpu::GrassUniform,
    ) -> Option<&BindGroup> {
        self.materials
            .get(&(
                water.unwrap_or_default().into(),
                crate::surface_terrain::TerrainUniform::new(terrain).key(),
                grass.key(),
            ))
            .map(|(_, group, _)| group)
    }
}

/// Retains only live material tables, uploading each new look once rather than once per vertex or frame.
pub(crate) fn prepare_water(
    mut gpu: ResMut<WaterGpu>,
    simulations: Res<crate::water_sim::SimGpu>,
    batches: Res<InstanceBatches>,
    grass_bounds: Res<crate::grass_gpu::GrassBounds>,
    pipelines: Res<ForwardPipelines>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let looks = std::iter::once((
        SurfaceWater::default(),
        None,
        crate::grass_gpu::GrassUniform::default(),
    ))
    .chain(batches.queue_batches().map(|batch| {
        (
            batches.water_surface(batch).unwrap_or_default(),
            batches.terrain_surface(batch),
            grass_bounds.uniform(batches.grass_surface(batch), batch.mesh),
        )
    }));
    let mut alive = HashSet::new();
    for (look, terrain, grass) in looks {
        let terrain = crate::surface_terrain::TerrainUniform::new(terrain);
        let key = (WaterKey::from(look), terrain.key(), grass.key());
        alive.insert(key);
        let Some(texture) = simulations
            .0
            .get(&look.sim_id)
            .or_else(|| simulations.0.get(&0))
        else {
            continue;
        };
        gpu.materials.entry(key).or_insert_with(|| {
            let mut uniform = WaterUniform::new(look);
            uniform.terrain = terrain;
            uniform.grass = grass;
            let mut buffer = UniformBuffer::from(uniform);
            buffer.write_buffer(&device, &queue);
            let layout = cache.get_bind_group_layout(&pipelines.water_layout);
            let group =
                device.create_bind_group("ve_water", &layout, &BindGroupEntries::single(&buffer));
            let sim = (look.sim_id != 0).then(|| {
                let layout = cache.get_bind_group_layout(&sim_layout());
                device.create_bind_group(
                    "ve_water_sim",
                    &layout,
                    &BindGroupEntries::sequential((&buffer, &texture.view)),
                )
            });
            (buffer, group, sim)
        });
    }
    gpu.materials.retain(|key, _| alive.contains(key));
}

#[cfg(test)]
#[path = "water_gpu_tests.rs"]
mod tests;

/// Shared surface uniform layout used by every geometry pass.
pub(crate) fn sim_layout() -> bevy_render::render_resource::BindGroupLayoutDescriptor {
    use bevy_render::render_resource::{
        BindGroupLayoutDescriptor, BindGroupLayoutEntries, ShaderStages, TextureSampleType,
        binding_types::{texture_2d, uniform_buffer},
    };
    BindGroupLayoutDescriptor::new(
        "ve_surface_sim_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                uniform_buffer::<WaterUniform>(false),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    )
}

/// Ordinary water retains its original uniform-only binding.
pub(crate) fn layout() -> bevy_render::render_resource::BindGroupLayoutDescriptor {
    use bevy_render::render_resource::{
        BindGroupLayoutDescriptor, BindGroupLayoutEntries, ShaderStages,
        binding_types::uniform_buffer,
    };
    BindGroupLayoutDescriptor::new(
        "ve_surface_layout",
        &BindGroupLayoutEntries::single(
            ShaderStages::VERTEX_FRAGMENT,
            uniform_buffer::<WaterUniform>(false),
        ),
    )
}
