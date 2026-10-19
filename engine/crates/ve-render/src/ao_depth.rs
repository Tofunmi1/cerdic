//! Depth for screen-space effects (ambient occlusion).
//!
//! The browser backend is WebGL2 through wgpu's GL path, which cannot read a depth texture except by comparison
//! (`textureLoad` has no GLSL form and a plain sample comes out as a shadow sampler). So the opaque batches are drawn once
//! more, single-sampled, into a float colour texture holding the fragment depth (see `shaders/ao_depth.wgsl`), which the
//! post pass reads like any other colour texture. The pass only runs while occlusion is on.

use std::collections::HashMap;

use bevy_asset::Handle;
use bevy_camera::Camera3d;
use bevy_ecs::prelude::*;
use bevy_log::warn_once;
use bevy_mesh::{Mesh, MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_render::camera::ExtractedCamera;
use bevy_render::mesh::RenderMesh;
use bevy_render::mesh::allocator::MeshAllocator;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{
    BindGroupLayoutDescriptor, CachedRenderPipelineId, ColorTargetState, ColorWrites,
    CompareFunction, DepthBiasState, DepthStencilState, Extent3d, FragmentState, LoadOp,
    MultisampleState, Operations, PipelineCache, PrimitiveState, PrimitiveTopology,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipelineDescriptor, StencilState, StoreOp, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, VertexState,
};
use bevy_render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy_render::texture::{CachedTexture, TextureCache};
use bevy_shader::Shader;

use crate::counters::Pass;
use crate::cutout::CutoutKind;
use crate::geometry::bind_geometry;
use crate::pass::bind_slot;
use crate::pipeline::instance_layout;
use crate::prepare::{InstanceBatches, ViewSlots, ViewUniforms};
use crate::quality::RenderQuality;
use crate::targets::DEPTH_FORMAT;

/// Source of the depth shader.
pub(crate) const SHADER: &str = include_str!("shaders/ao_depth.wgsl");

/// The shader's asset name.
pub(crate) const SHADER_NAME: &str = "ve_ao_depth.wgsl";

/// Format of the depth colour texture: one 32-bit float, which is renderable (not blendable) and read with `textureLoad`.
pub(crate) const FORMAT: TextureFormat = TextureFormat::R32Float;

/// A view's depth for the post pass: the float colour texture the pass writes and the depth buffer it tests with.
#[derive(Component, Clone)]
pub(crate) struct ViewAoDepth {
    pub(crate) color: CachedTexture,
    depth: CachedTexture,
    /// The `ao` pass's output, one byte per pixel, which the present pass averages and applies.
    pub(crate) occlusion: CachedTexture,
}

/// Whether a view needs the depth texture: the post pass binds it whenever occlusion or bloom is on.
fn wanted(quality: &RenderQuality) -> bool {
    quality.ssao_enabled || quality.bloom_enabled
}

/// Creates (or reuses, through the texture cache) the depth targets of every 3D camera while a post effect is on, and
/// removes them otherwise.
pub(crate) fn prepare_ao_depth_targets(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    quality: Res<RenderQuality>,
    views: Query<(Entity, &ExtractedCamera), With<Camera3d>>,
) {
    for (entity, camera) in &views {
        let Some(size) = camera.physical_target_size.filter(|_| wanted(&quality)) else {
            commands.entity(entity).remove::<ViewAoDepth>();
            continue;
        };
        let size = Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        };
        let mut texture = |label, format, usage| {
            texture_cache.get(
                &device,
                TextureDescriptor {
                    label: Some(label),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                },
            )
        };
        let color = texture(
            "ve_ao_depth",
            FORMAT,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        let depth = texture(
            "ve_ao_depth_buffer",
            DEPTH_FORMAT,
            TextureUsages::RENDER_ATTACHMENT,
        );
        let occlusion = texture(
            "ve_ao_occlusion",
            crate::ao::FORMAT,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        commands.entity(entity).insert(ViewAoDepth {
            color,
            depth,
            occlusion,
        });
    }
}

/// The pipelines of the depth pass, one per mesh vertex layout, sharing the forward pass's view uniform layout.
#[derive(Resource)]
pub(crate) struct AoDepthPipelines {
    view_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    cache: HashMap<MeshVertexBufferLayoutRef, CachedRenderPipelineId>,
}

impl AoDepthPipelines {
    pub(crate) fn new(shader: Handle<Shader>, view_layout: BindGroupLayoutDescriptor) -> Self {
        Self {
            view_layout,
            shader,
            cache: HashMap::new(),
        }
    }

    fn descriptor(&self, mesh_layout: VertexBufferLayout) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("ve_ao_depth".into()),
            layout: vec![self.view_layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![mesh_layout, instance_layout()],
                ..Default::default()
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                cull_mode: Some(bevy_render::render_resource::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                // Reverse-z, as the forward pass: nearer fragments have larger depth.
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

/// Queues the depth pipeline for every mesh layout about to be drawn, while occlusion is on.
pub(crate) fn queue_ao_depth_pipelines(
    mut pipelines: ResMut<AoDepthPipelines>,
    quality: Res<RenderQuality>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    batches: Res<InstanceBatches>,
) {
    if !quality.ssao_enabled {
        return;
    }
    for batch in batches.queue_batches().filter(|batch| !batch.transparent) {
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
            warn_once!("depth pipeline: a mesh lacks POSITION attributes");
            continue;
        };
        let id = pipeline_cache.queue_render_pipeline(pipelines.descriptor(mesh_layout));
        pipelines.cache.insert(layout.clone(), id);
    }
}

/// Draws the opaque batches from every slot of the view into its depth texture (cleared to 0: the sky).
pub(crate) fn ao_depth_pass(
    view: ViewQuery<(&ViewAoDepth, &ViewSlots)>,
    quality: Res<RenderQuality>,
    batches: Res<InstanceBatches>,
    pipelines: Res<AoDepthPipelines>,
    cutouts: Res<crate::cutout::CutoutPipelines>,
    textures: Res<crate::textures::TexturesGpu>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    mut ctx: RenderContext,
) {
    if !quality.ssao_enabled {
        return;
    }
    let (targets, slots) = view.into_inner();
    let Some(view_bind_group) = view_uniforms.bind_group.as_ref() else {
        return;
    };
    let color_attachments = [Some(RenderPassColorAttachment {
        view: &targets.color.default_view,
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Clear(bevy_color::LinearRgba::BLACK.into()),
            store: StoreOp::Store,
        },
    })];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_ao_depth_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            view: &targets.depth.default_view,
            depth_ops: Some(Operations {
                load: LoadOp::Clear(0.0),
                store: StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    for batch in batches.main_batches().filter(|batch| !batch.transparent) {
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            continue;
        };
        // A batch with a cut-out instance draws with the variant that tests the albedo alpha, when its mesh can.
        let cutout = batch
            .cutout
            .then(|| cutouts.get(CutoutKind::Depth, &render_mesh.layout))
            .flatten()
            .zip(textures.bind_group.as_ref());
        // Pipelines are queued one frame after a mesh first appears.
        let id = match cutout {
            Some((id, maps)) => {
                pass.set_bind_group(1, maps, &[]);
                Some(id)
            }
            None => pipelines.cache.get(&render_mesh.layout).copied(),
        };
        let Some(pipeline) = id.and_then(|id| pipeline_cache.get_render_pipeline(id)) else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        let Some(instance_buffer) = batches.instance_buffer(batch) else {
            continue;
        };
        let Some(call) = bind_geometry(&mut pass, &allocator, render_mesh, batch, instance_buffer)
        else {
            continue;
        };
        for slot in &slots.0 {
            bind_slot(&mut pass, view_bind_group, slot);
            call.issue(&mut pass, batch.instances.clone(), Pass::Depth);
        }
    }
}
