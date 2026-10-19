//! The depth-only pipelines of the sun's shadow pass, the sun cameras' uniform, and this frame's shadow parameters.

use std::collections::HashMap;

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_log::warn_once;
use bevy_math::{Vec3, Vec4};
use bevy_mesh::{Mesh, MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_render::mesh::RenderMesh;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, CachedRenderPipelineId, CompareFunction,
    DepthBiasState, DepthStencilState, DynamicUniformBuffer, MultisampleState, PipelineCache,
    PrimitiveState, PrimitiveTopology, RenderPipelineDescriptor, StencilState, VertexState,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::view::ExtractedView;
use bevy_shader::Shader;

use crate::light::ExtractedLights;
use crate::pipeline::instance_layout;
use crate::prepare::{InstanceBatches, ViewUniform};
use crate::quality::RenderQuality;
use crate::shadow::{SHADOW_FORMAT, ShadowMap};
use crate::shadow_math::{ShadowParams, sun_cascades, view_frustum};
use crate::stereo::{StereoViews, viewer_position};

/// This frame's shadow parameters (disabled when there is no map or no sun).
#[derive(Resource, Default)]
pub(crate) struct ShadowState {
    pub(crate) params: ShadowParams,
}

/// The depth-only pipelines and the sun camera's uniform.
#[derive(Resource)]
pub(crate) struct ShadowPipelines {
    view_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    pub(crate) cache: HashMap<MeshVertexBufferLayoutRef, CachedRenderPipelineId>,
    /// One camera per cascade, addressed by dynamic offset.
    view_buffer: DynamicUniformBuffer<ViewUniform>,
    pub(crate) offsets: Vec<u32>,
    pub(crate) view_bind_group: Option<BindGroup>,
}

impl ShadowPipelines {
    pub(crate) fn new(shader: Handle<Shader>) -> Self {
        Self {
            // The sun cameras have a bind group of their own: just the uniform, without the exposure texture.
            view_layout: crate::pipeline::uniform_only_view_layout(),
            shader,
            cache: HashMap::new(),
            view_buffer: DynamicUniformBuffer::default(),
            offsets: Vec::new(),
            view_bind_group: None,
        }
    }

    fn descriptor(&self, mesh_layout: VertexBufferLayout) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("ve_shadow".into()),
            layout: vec![self.view_layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![mesh_layout, instance_layout()],
                ..Default::default()
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                // Both faces cast: thin shells would otherwise let light through.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: SHADOW_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::Less),
                stencil: StencilState::default(),
                bias: DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState::default(),
            fragment: None,
            ..Default::default()
        }
    }
}

/// Fits the sun camera to the viewer, uploads its uniform and records this frame's shadow parameters.
pub(crate) fn prepare_shadow_state(
    lights: Res<ExtractedLights>,
    quality: Res<RenderQuality>,
    map: Res<ShadowMap>,
    views: Query<(&ExtractedView, Option<&StereoViews>)>,
    mut state: ResMut<ShadowState>,
    mut pipelines: ResMut<ShadowPipelines>,
    wind: Res<crate::WindField>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let sun = lights
        .sun
        .as_ref()
        .filter(|(_, light)| light.intensity > 0.0);
    let (Some((resolution, cascades)), Some((forward, _))) = (map.layout(), sun) else {
        state.params = ShadowParams::default();
        return;
    };
    let viewer = viewer_position(
        views
            .iter()
            .map(|(view, stereo)| (view.world_from_view.translation(), stereo)),
    );
    // The cascades follow the first view's direction and field of view (the head, for a headset).
    let (view_forward, tan) = views
        .iter()
        .next()
        .map_or((Vec3::NEG_Z, [1.0, 0.6]), |(view, _)| {
            view_frustum(view.world_from_view.rotation(), view.clip_from_view)
        });
    let params = sun_cascades(
        -*forward,
        viewer,
        view_forward,
        tan,
        cascades as usize,
        resolution,
        quality.shadow_pcf_radius,
    );
    pipelines.view_buffer.clear();
    pipelines.offsets.clear();
    for cascade in 0..params.cascades {
        let uniform = ViewUniform {
            clip_from_world: params.clip_from_world[cascade],
            camera_position: Vec4::ZERO,
            exposure: Vec4::new(1.0, 0.0, 0.0, 0.0),
            // The depth-only shadow pass does not grade; keep the fields neutral but present.
            grading: Vec4::new(1.0, 1.0, 0.0, 0.0),
            tint: Vec4::new(1.0, 1.0, 1.0, 0.0),
            // Foliage sways in its shadow as it does on screen.
            wind: wind.uniform(),
            ..ViewUniform::default()
        };
        let offset = pipelines.view_buffer.push(&uniform);
        pipelines.offsets.push(offset);
    }
    pipelines.view_buffer.write_buffer(&device, &queue);
    let layout = pipeline_cache.get_bind_group_layout(&pipelines.view_layout);
    pipelines.view_bind_group = pipelines.view_buffer.binding().map(|binding| {
        device.create_bind_group(
            "ve_shadow_view_bind_group",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: binding,
            }],
        )
    });
    state.params = params;
}

/// Queues the depth-only pipeline for every mesh layout about to be drawn, while shadows are on.
pub(crate) fn queue_shadow_pipelines(
    mut pipelines: ResMut<ShadowPipelines>,
    state: Res<ShadowState>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    batches: Res<InstanceBatches>,
) {
    if !state.params.enabled {
        return;
    }
    for batch in batches.queue_batches() {
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            continue;
        };
        let layout = &render_mesh.layout;
        if pipelines.cache.contains_key(layout) {
            continue;
        }
        let Ok(mesh_layout) = layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])
        else {
            warn_once!("shadow pipeline: a mesh lacks POSITION attributes");
            continue;
        };
        let id = pipeline_cache.queue_render_pipeline(pipelines.descriptor(mesh_layout));
        pipelines.cache.insert(layout.clone(), id);
    }
}
