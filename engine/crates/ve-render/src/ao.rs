//! The ambient occlusion pass: a full-screen pass that reads the `ao_depth` pass's depth texture and writes one occlusion
//! value per pixel into the view's `R8` occlusion texture (see `shaders/ao.wgsl`). The present pass averages and applies it.

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_render::render_resource::binding_types::{texture_2d, uniform_buffer};
use bevy_render::render_resource::{
    BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BindingResource,
    CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, LoadOp,
    MultisampleState, Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages, StoreOp, TextureFormat,
    TextureSampleType, VertexState,
};
use bevy_render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy_shader::Shader;

use crate::ao_depth::ViewAoDepth;
use crate::prepare::{ViewSlots, ViewUniform, ViewUniforms};
use crate::quality::RenderQuality;

/// Source of the occlusion shader.
pub(crate) const SHADER: &str = include_str!("shaders/ao.wgsl");

/// The shader's asset name.
pub(crate) const SHADER_NAME: &str = "ve_ao.wgsl";

/// Format of the occlusion texture: one normalised byte, which is renderable on every WebGL2 device.
pub(crate) const FORMAT: TextureFormat = TextureFormat::R8Unorm;

/// The occlusion pipeline and its bind group layout (the depth texture and the view uniform).
#[derive(Resource)]
pub(crate) struct AoPipeline {
    layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    id: Option<CachedRenderPipelineId>,
}

impl AoPipeline {
    pub(crate) fn new(shader: Handle<Shader>) -> Self {
        let layout = BindGroupLayoutDescriptor::new(
            "ve_ao_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    uniform_buffer::<ViewUniform>(true),
                ),
            ),
        );
        Self {
            layout,
            shader,
            id: None,
        }
    }

    fn descriptor(&self) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("ve_ao".into()),
            layout: vec![self.layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("vertex".into()),
                ..Default::default()
            },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
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

/// Queues the occlusion pipeline the first time occlusion is on.
pub(crate) fn queue_ao_pipeline(
    mut pipeline: ResMut<AoPipeline>,
    quality: Res<RenderQuality>,
    pipeline_cache: Res<PipelineCache>,
) {
    if quality.ssao_enabled && pipeline.id.is_none() {
        pipeline.id = Some(pipeline_cache.queue_render_pipeline(pipeline.descriptor()));
    }
}

/// Fills the view's occlusion texture from its depth texture, once per view slot (both eyes in stereo).
pub(crate) fn ao_pass(
    view: ViewQuery<(&ViewAoDepth, &ViewSlots)>,
    quality: Res<RenderQuality>,
    pipeline: Res<AoPipeline>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    if !quality.ssao_enabled {
        return;
    }
    let (targets, slots) = view.into_inner();
    let (Some(render_pipeline), Some(uniform)) = (
        pipeline
            .id
            .and_then(|id| pipeline_cache.get_render_pipeline(id)),
        view_uniforms.buffer.binding(),
    ) else {
        return;
    };
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.layout);
    let bind_group = device.create_bind_group(
        "ve_ao_bind_group",
        &layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&targets.color.default_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: uniform,
            },
        ],
    );
    let color_attachments = [Some(RenderPassColorAttachment {
        view: &targets.occlusion.default_view,
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            // Open (1) wherever no slot covers the pixel.
            load: LoadOp::Clear(bevy_color::LinearRgba::WHITE.into()),
            store: StoreOp::Store,
        },
    })];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_ao_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(render_pipeline);
    for slot in &slots.0 {
        if let Some([x, y, width, height]) = slot.viewport {
            #[expect(
                clippy::cast_precision_loss,
                reason = "viewports are below 2^24 pixels"
            )]
            pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
        }
        pass.set_bind_group(0, &bind_group, &[slot.offset]);
        pass.draw(0..3, 0..1);
    }
}
