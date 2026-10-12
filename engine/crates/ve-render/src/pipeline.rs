//! The forward render pipeline and its per-mesh-layout, per-format, per-sample-count specialisation.

use std::collections::HashMap;

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_log::warn_once;
use bevy_mesh::{Mesh, MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_render::mesh::RenderMesh;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::binding_types::{sampler, texture_depth_2d, uniform_buffer};
use bevy_render::render_resource::{
    BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId, ColorTargetState,
    ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face, FragmentState,
    FrontFace, MultisampleState, PipelineCache, PrimitiveState, PrimitiveTopology,
    RenderPipelineDescriptor, SamplerBindingType, ShaderStages, StencilState, TextureFormat,
    VertexAttribute, VertexFormat, VertexState, VertexStepMode,
};
use bevy_render::view::{Msaa, ViewTarget};
use bevy_shader::Shader;

use crate::lens::LensWarp;
use crate::light::LightsUniform;
use crate::prepare::{InstanceBatches, InstanceData, ViewUniform};
use crate::targets::{DEPTH_FORMAT, render_format};

/// Source of the forward shader, compiled at runtime by the shader pipeline.
pub(crate) const FORWARD_SHADER: &str = include_str!("shaders/forward.wgsl");

type PipelineKey = (MeshVertexBufferLayoutRef, TextureFormat, u32);

/// The forward pipeline family: shared layouts and shader, specialised lazily.
#[derive(Resource)]
pub(crate) struct ForwardPipelines {
    pub(crate) view_layout: BindGroupLayoutDescriptor,
    pub(crate) lights_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    cache: HashMap<PipelineKey, CachedRenderPipelineId>,
}

impl ForwardPipelines {
    pub(crate) fn new(shader: Handle<Shader>) -> Self {
        let view_layout = BindGroupLayoutDescriptor::new(
            "ve_view_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::VERTEX_FRAGMENT,
                uniform_buffer::<ViewUniform>(true),
            ),
        );
        // Lights, then the sun's shadow map and the comparison sampler that reads it.
        let lights_layout = BindGroupLayoutDescriptor::new(
            "ve_lights_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    uniform_buffer::<LightsUniform>(false),
                    texture_depth_2d(),
                    sampler(SamplerBindingType::Comparison),
                ),
            ),
        );
        Self {
            view_layout,
            lights_layout,
            shader,
            cache: HashMap::new(),
        }
    }

    /// The pipeline queued for this mesh layout, output format and sample count, if any.
    pub(crate) fn get(
        &self,
        layout: &MeshVertexBufferLayoutRef,
        format: TextureFormat,
        samples: u32,
    ) -> Option<CachedRenderPipelineId> {
        self.cache.get(&(layout.clone(), format, samples)).copied()
    }

    fn descriptor(
        &self,
        mesh_layout: VertexBufferLayout,
        format: TextureFormat,
        samples: u32,
    ) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("ve_forward".into()),
            layout: vec![self.view_layout.clone(), self.lights_layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![mesh_layout, instance_layout()],
                ..Default::default()
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                // Bevy projections are reverse-z: nearer fragments have larger depth.
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

/// Instance buffer layout: the model matrix as four `vec4` columns (locations 3 to 6), then base
/// colour, material parameters and emissive (locations 7 to 9).
pub(crate) fn instance_layout() -> VertexBufferLayout {
    let vec4 = u64::try_from(size_of::<[f32; 4]>()).unwrap_or(16);
    VertexBufferLayout {
        array_stride: u64::try_from(size_of::<InstanceData>()).unwrap_or(112),
        step_mode: VertexStepMode::Instance,
        attributes: (0..7_u32)
            .map(|i| VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: vec4 * u64::from(i),
                shader_location: 3 + i,
            })
            .collect(),
    }
}

/// Queues a pipeline for every (mesh layout, output format, sample count) about to be drawn.
pub(crate) fn queue_forward_pipelines(
    mut pipelines: ResMut<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    batches: Res<InstanceBatches>,
    lens: Res<LensWarp>,
    views: Query<(&ViewTarget, &Msaa)>,
) {
    let variants: Vec<(TextureFormat, u32)> = views
        .iter()
        .filter_map(|(target, msaa)| {
            Some((
                render_format(target.out_texture_view_format()?, *lens),
                msaa.samples(),
            ))
        })
        .collect();
    for batch in &batches.batches {
        // A mesh that is not uploaded yet is retried next frame.
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            continue;
        };
        let layout = &render_mesh.layout;
        let Ok(mesh_layout) = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
        ]) else {
            // A mesh without positions and normals cannot be drawn by this pipeline.
            warn_once!("pipeline queue: a mesh lacks POSITION or NORMAL attributes");
            continue;
        };
        for &(format, samples) in &variants {
            let key = (layout.clone(), format, samples);
            if pipelines.cache.contains_key(&key) {
                continue;
            }
            let descriptor = pipelines.descriptor(mesh_layout.clone(), format, samples);
            let id = pipeline_cache.queue_render_pipeline(descriptor);
            pipelines.cache.insert(key, id);
        }
    }
}
