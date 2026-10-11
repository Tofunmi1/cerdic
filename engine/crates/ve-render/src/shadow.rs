//! The sun's shadow map: its texture, per-frame matrix, depth-only pipelines and pass.
//!
//! One orthographic map follows the viewer (see `shadow_math`). It is sampled in the forward shader
//! with a comparison sampler. When shadows are off the lights bind group still needs a texture, so a
//! 1 by 1 stand-in is bound and the shader skips the lookup.

use std::collections::HashMap;

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_log::warn_once;
use bevy_math::Vec4;
use bevy_mesh::{Mesh, MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_render::mesh::RenderMesh;
use bevy_render::mesh::allocator::MeshAllocator;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, CachedRenderPipelineId, CompareFunction,
    DepthBiasState, DepthStencilState, Extent3d, FilterMode, LoadOp, MultisampleState, Operations,
    PipelineCache, PrimitiveState, PrimitiveTopology, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerDescriptor, StencilState,
    StoreOp, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView, TextureViewDescriptor, UniformBuffer, VertexState,
};
use bevy_render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy_render::view::ExtractedView;
use bevy_shader::Shader;

use crate::counters::{self, Pass};
use crate::geometry::bind_geometry;
use crate::light::ExtractedLights;
use crate::pipeline::instance_layout;
use crate::prepare::{InstanceBatches, ViewUniform};
use crate::quality::RenderQuality;
use crate::shadow_math::{SHADOW_DISTANCE, ShadowParams, sun_shadow_params};
use crate::stereo::{StereoViews, viewer_position};

/// Source of the shadow shader.
pub(crate) const SHADOW_SHADER: &str = include_str!("shaders/shadow.wgsl");

/// The shadow shader's asset name.
pub(crate) const SHADOW_SHADER_NAME: &str = "ve_shadow.wgsl";

/// Depth format of the shadow map.
pub(crate) const SHADOW_FORMAT: TextureFormat = TextureFormat::Depth32Float;

struct ShadowTarget {
    // Held so the texture outlives its view.
    _texture: Texture,
    view: TextureView,
    resolution: u32,
}

/// The shadow map texture (when shadows are on), a stand-in for when they are off, and the
/// comparison sampler.
#[derive(Resource, Default)]
pub(crate) struct ShadowMap {
    target: Option<ShadowTarget>,
    stand_in: Option<(Texture, TextureView)>,
    sampler: Option<Sampler>,
}

impl ShadowMap {
    /// The view the forward shader samples: the real map, or the stand-in.
    pub(crate) fn sampled_view(&self) -> Option<&TextureView> {
        self.target
            .as_ref()
            .map(|target| &target.view)
            .or_else(|| self.stand_in.as_ref().map(|(_, view)| view))
    }

    /// The comparison sampler.
    pub(crate) fn sampler(&self) -> Option<&Sampler> {
        self.sampler.as_ref()
    }

    /// Side of the real map in texels, or `None` when shadows are off.
    pub(crate) fn resolution(&self) -> Option<u32> {
        self.target.as_ref().map(|target| target.resolution)
    }
}

fn depth_texture(device: &RenderDevice, size: u32, label: &'static str) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: SHADOW_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

/// Keeps the shadow map's size in step with the quality settings. Replacing the map drops the old
/// texture, so switching tiers does not accumulate GPU memory.
pub(crate) fn prepare_shadow_map(
    mut map: ResMut<ShadowMap>,
    quality: Res<RenderQuality>,
    device: Res<RenderDevice>,
) {
    if map.sampler.is_none() {
        map.sampler = Some(device.create_sampler(&SamplerDescriptor {
            label: Some("ve_shadow_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            compare: Some(CompareFunction::LessEqual),
            ..Default::default()
        }));
    }
    if map.stand_in.is_none() {
        map.stand_in = Some(depth_texture(&device, 1, "ve_shadow_stand_in"));
    }
    let wanted = quality
        .shadows_enabled()
        .then_some(quality.shadow_resolution);
    if map.resolution() != wanted {
        map.target = wanted.map(|resolution| {
            let (texture, view) = depth_texture(&device, resolution, "ve_shadow_map");
            ShadowTarget {
                _texture: texture,
                view,
                resolution,
            }
        });
    }
}

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
    cache: HashMap<MeshVertexBufferLayoutRef, CachedRenderPipelineId>,
    view_buffer: UniformBuffer<ViewUniform>,
    view_bind_group: Option<BindGroup>,
}

impl ShadowPipelines {
    pub(crate) fn new(shader: Handle<Shader>, view_layout: BindGroupLayoutDescriptor) -> Self {
        Self {
            view_layout,
            shader,
            cache: HashMap::new(),
            view_buffer: UniformBuffer::default(),
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
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let sun = lights
        .sun
        .as_ref()
        .filter(|(_, light)| light.intensity > 0.0);
    let (Some(resolution), Some((forward, _))) = (map.resolution(), sun) else {
        state.params = ShadowParams::default();
        return;
    };
    let viewer = viewer_position(
        views
            .iter()
            .map(|(view, stereo)| (view.world_from_view.translation(), stereo)),
    );
    let params = sun_shadow_params(
        -*forward,
        viewer,
        SHADOW_DISTANCE,
        resolution,
        quality.shadow_pcf_radius,
    );
    pipelines.view_buffer.set(ViewUniform {
        clip_from_world: params.clip_from_world,
        camera_position: Vec4::ZERO,
        exposure: Vec4::new(1.0, 0.0, 0.0, 0.0),
    });
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
    for batch in &batches.batches {
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

/// Renders every batch into the shadow map from the sun's point of view.
pub(crate) fn shadow_pass(
    state: Res<ShadowState>,
    map: Res<ShadowMap>,
    pipelines: Res<ShadowPipelines>,
    pipeline_cache: Res<PipelineCache>,
    batches: Res<InstanceBatches>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    mut ctx: RenderContext,
) {
    // The shadow pass opens each frame, so it is where the draw counters roll over.
    counters::begin_frame();
    if !state.params.enabled {
        return;
    }
    let (Some(target), Some(view_bind_group), Some(instance_buffer)) = (
        map.target.as_ref(),
        pipelines.view_bind_group.as_ref(),
        batches.buffer.buffer(),
    ) else {
        return;
    };
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_shadow_pass"),
        color_attachments: &[],
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: &target.view,
            depth_ops: Some(Operations {
                load: LoadOp::Clear(1.0),
                store: StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_bind_group(0, view_bind_group, &[0]);
    for batch in &batches.batches {
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            continue;
        };
        // Pipelines are queued one frame after a mesh first appears.
        let Some(pipeline) = pipelines
            .cache
            .get(&render_mesh.layout)
            .and_then(|id| pipeline_cache.get_render_pipeline(*id))
        else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        if let Some(call) =
            bind_geometry(&mut pass, &allocator, render_mesh, batch, instance_buffer)
        {
            call.issue(&mut pass, batch.instances.clone(), Pass::Shadow);
        }
    }
}
