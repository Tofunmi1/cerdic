//! The forward render pipeline and its per-mesh-layout, per-target-format specialisation.

use std::collections::HashMap;

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_mesh::{Mesh, MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_render::mesh::RenderMesh;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::binding_types::uniform_buffer;
use bevy_render::render_resource::{
    BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId, ColorTargetState,
    ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face, FragmentState,
    FrontFace, MultisampleState, PipelineCache, PrimitiveState, PrimitiveTopology,
    RenderPipelineDescriptor, ShaderStages, StencilState, TextureFormat, VertexAttribute,
    VertexFormat, VertexState, VertexStepMode,
};
use bevy_render::view::ViewTarget;
use bevy_shader::Shader;

use crate::prepare::{DEPTH_FORMAT, InstanceBatches, InstanceData, ViewUniform};

/// Source of the forward shader, compiled at runtime by the shader pipeline.
pub(crate) const FORWARD_SHADER: &str = include_str!("shaders/forward.wgsl");

type PipelineKey = (MeshVertexBufferLayoutRef, TextureFormat);

/// The forward pipeline family: shared layout and shader, specialised lazily per mesh layout.
#[derive(Resource)]
pub(crate) struct ForwardPipelines {
    pub(crate) view_layout: BindGroupLayoutDescriptor,
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
        Self {
            view_layout,
            shader,
            cache: HashMap::new(),
        }
    }

    /// The pipeline queued for this mesh layout and output format, if any.
    pub(crate) fn get(
        &self,
        layout: &MeshVertexBufferLayoutRef,
        format: TextureFormat,
    ) -> Option<CachedRenderPipelineId> {
        self.cache.get(&(layout.clone(), format)).copied()
    }

    fn descriptor(
        &self,
        mesh_layout: VertexBufferLayout,
        format: TextureFormat,
    ) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("ve_forward".into()),
            layout: vec![self.view_layout.clone()],
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
            multisample: MultisampleState::default(),
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

/// Instance buffer layout: a model matrix as four `vec4` columns at shader locations 3 to 6.
fn instance_layout() -> VertexBufferLayout {
    let column = u64::try_from(size_of::<[f32; 4]>()).unwrap_or(16);
    VertexBufferLayout {
        array_stride: u64::try_from(size_of::<InstanceData>()).unwrap_or(64),
        step_mode: VertexStepMode::Instance,
        attributes: (0..4_u32)
            .map(|i| VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: column * u64::from(i),
                shader_location: 3 + i,
            })
            .collect(),
    }
}

/// Queues a pipeline for every (mesh layout, output format) combination about to be drawn.
pub(crate) fn queue_forward_pipelines(
    mut pipelines: ResMut<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    batches: Res<InstanceBatches>,
    views: Query<&ViewTarget>,
) {
    let formats: Vec<TextureFormat> = views
        .iter()
        .filter_map(ViewTarget::out_texture_view_format)
        .collect();
    for batch in &batches.batches {
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            continue;
        };
        let layout = &render_mesh.layout;
        let Ok(mesh_layout) = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
        ]) else {
            // A mesh without positions and normals cannot be drawn by this pipeline.
            continue;
        };
        for &format in &formats {
            let key = (layout.clone(), format);
            if pipelines.cache.contains_key(&key) {
                continue;
            }
            let descriptor = pipelines.descriptor(mesh_layout.clone(), format);
            let id = pipeline_cache.queue_render_pipeline(descriptor);
            pipelines.cache.insert(key, id);
        }
    }
}
