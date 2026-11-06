//! Per-scene procedural sky data and the fullscreen background pipeline.
use crate::targets::{DEPTH_FORMAT, render_format};
use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_render::RenderApp;
use bevy_render::extract_resource::ExtractResource;
use bevy_render::render_resource::{
    CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
    DepthStencilState, FragmentState, FrontFace, MultisampleState, PipelineCache, PrimitiveState,
    PrimitiveTopology, RenderPipelineDescriptor, StencilState, TextureFormat, VertexState,
};
use bevy_render::view::{Msaa, ViewTarget};
use bevy_shader::Shader;
use std::collections::HashMap;

/// Extracted analytic sky parameters; disabled by default to preserve the traditional clear color.
#[derive(Resource, ExtractResource, Clone, Copy, Debug, PartialEq)]
#[extract_app(RenderApp)]
pub struct SkyBackground {
    /// Enables the sky pass.
    pub enabled: bool,
    /// RGB horizon, zenith and ground colours in sRGB.
    pub horizon: [f32; 3],
    /// Zenith colour.
    pub zenith: [f32; 3],
    /// Ground colour.
    pub ground: [f32; 3],
    /// Direction travelled by sunlight.
    pub sun_direction: [f32; 3],
    /// Sun and haze controls.
    pub sun_intensity: f32,
    /// Turbidity, 1..10.
    pub turbidity: f32,
    /// Clouds enabled and coverage threshold.
    pub clouds: bool,
    /// Cloud cover, 0..1.
    pub cloud_cover: f32,
    /// Stable cloud hash seed.
    pub seed: u32,
    /// Distance haze from the document (`haze`, 0 clear to 1 thick fog); added to the haze the turbidity already gives.
    pub haze: f32,
    /// sRGB colour of the distance haze (`haze_color`).
    pub haze_color: [f32; 3],
    /// Height in metres over which ground haze thins (`haze_height`); above zero a fog volume hugs the ground.
    pub haze_height: f32,
    /// Strength of the sunlight shafts through haze (`light_shafts`, 0 to 1), drawn by Bevy's volumetric fog.
    pub shafts: f32,
    /// Depth of field: the focus distance in metres (`focus_distance`); 0 is off.
    pub focus_distance: f32,
    /// Depth of field aperture in f-stops (`aperture`).
    pub aperture: f32,
    /// Motion blur strength (`motion_blur`, 0 off to 1).
    pub motion_blur: f32,
    /// Screen-space contact shadows (`contact_shadows`).
    pub contact_shadows: bool,
}

impl Default for SkyBackground {
    fn default() -> Self {
        Self {
            enabled: false,
            horizon: [0.47, 0.66, 0.89],
            zenith: [0.15, 0.36, 0.72],
            ground: [0.37, 0.29, 0.20],
            sun_direction: [0.0, -0.7, -0.7],
            sun_intensity: 3.0,
            turbidity: 3.0,
            clouds: false,
            cloud_cover: 0.0,
            seed: 0,
            haze: 0.0,
            haze_color: [0.72, 0.78, 0.86],
            haze_height: 0.0,
            shafts: 0.0,
            focus_distance: 0.0,
            aperture: 2.8,
            motion_blur: 0.0,
            contact_shadows: false,
        }
    }
}

pub(crate) const SHADER: &str = include_str!("shaders/sky.wgsl");

#[derive(Resource)]
pub(crate) struct SkyPipelines {
    pub(crate) view_layout: bevy_render::render_resource::BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    cache: HashMap<(TextureFormat, u32), CachedRenderPipelineId>,
}

impl SkyPipelines {
    pub(crate) fn new(
        shader: Handle<Shader>,
        view_layout: bevy_render::render_resource::BindGroupLayoutDescriptor,
    ) -> Self {
        Self {
            view_layout,
            shader,
            cache: HashMap::new(),
        }
    }

    pub(crate) fn get(
        &self,
        format: TextureFormat,
        samples: u32,
    ) -> Option<CachedRenderPipelineId> {
        self.cache.get(&(format, samples)).copied()
    }
}

pub(crate) fn queue(
    mut sky: ResMut<SkyPipelines>,
    cache: Res<PipelineCache>,
    views: Query<(&ViewTarget, &Msaa)>,
    lens: Res<crate::lens::LensWarp>,
    quality: Res<crate::quality::RenderQuality>,
) {
    for (target, msaa) in &views {
        let Some(format) = target.out_texture_view_format() else {
            continue;
        };
        let key = (render_format(format, *lens, *quality), msaa.samples());
        if sky.cache.contains_key(&key) {
            continue;
        }
        let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("ve_procedural_sky".into()),
            layout: vec![sky.view_layout.clone()],
            vertex: VertexState {
                shader: sky.shader.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![],
                ..Default::default()
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: msaa.samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader: sky.shader.clone(),
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.0,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..Default::default()
            }),
            ..Default::default()
        });
        sky.cache.insert(key, id);
    }
}

/// Rust half of the exact u32 lattice hash used by sky.wgsl, for CPU/GPU contract tests.
#[cfg(test)]
fn lattice_hash(cell: [i64; 3], seed: u32) -> u64 {
    let mut hash = u64::from(seed).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for value in cell {
        hash ^= value.cast_unsigned();
        hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash ^= hash >> 31;
    }
    hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^ (hash >> 29)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wgsl_and_cpu_use_the_same_seeded_integer_hash_contract() {
        assert!(
            SHADER.contains("0x9e3779b9u")
                && SHADER.contains("0x7f4a7c15u")
                && SHADER.contains("0xbf58476du")
                && SHADER.contains("0x1ce4e5b9u")
                && SHADER.contains("0x94d049bbu")
                && SHADER.contains("0x133111ebu")
        );
        assert_eq!(lattice_hash([0, 0, 0], 0), 0);
        assert_eq!(lattice_hash([2, -7, 19], 41), 0xcfba_5ae7_64f4_97d3);
        assert_eq!(lattice_hash([1, 2, 3], 9), 0x2320_939f_0a70_210a);
    }
}
