//! The present pass: copies a view's resolved main texture to its output when the output cannot be
//! rendered to directly (it is not an sRGB view, as with a `WebXR` layer framebuffer).
//!
//! Rendering straight into an sRGB output lets the hardware apply the transfer function for free, so
//! windows skip this pass. Other outputs get the main texture drawn into them here, with the sRGB
//! encode done in the shader. A copy also sidesteps a wgpu GL limit: an external framebuffer supports
//! one single-sampled colour attachment, so it cannot be an MSAA resolve target.

use std::collections::HashMap;

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_render::render_resource::binding_types::{sampler, texture_2d};
use bevy_render::render_resource::{
    BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BindingResource,
    CachedRenderPipelineId, ColorTargetState, ColorWrites, FilterMode, FragmentState,
    MultisampleState, PipelineCache, PrimitiveState, RenderPassDescriptor,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
    TextureFormat, TextureSampleType, VertexState,
};
use bevy_render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy_render::view::ViewTarget;
use bevy_shader::Shader;

use crate::targets::{ViewOffscreen, needs_blit};

/// Source of the present shader.
pub(crate) const PRESENT_SHADER: &str = include_str!("shaders/present.wgsl");

/// The present shader's asset name.
pub(crate) const PRESENT_SHADER_NAME: &str = "ve_present.wgsl";

/// The present pipelines, one per output format.
#[derive(Resource)]
pub(crate) struct PresentPipelines {
    layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    sampler: Option<Sampler>,
    cache: HashMap<TextureFormat, CachedRenderPipelineId>,
}

impl PresentPipelines {
    pub(crate) fn new(shader: Handle<Shader>) -> Self {
        let layout = BindGroupLayoutDescriptor::new(
            "ve_present_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    sampler(SamplerBindingType::NonFiltering),
                ),
            ),
        );
        Self {
            layout,
            shader,
            sampler: None,
            cache: HashMap::new(),
        }
    }

    fn descriptor(&self, output_format: TextureFormat) -> RenderPipelineDescriptor {
        // Non-sRGB outputs get the transfer function applied by hand.
        let entry = if output_format.is_srgb() {
            "fragment_copy"
        } else {
            "fragment_srgb"
        };
        RenderPipelineDescriptor {
            label: Some("ve_present".into()),
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
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState {
                    format: output_format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

/// Creates the nearest-neighbour sampler once the device exists. The copy is one to one, so no
/// filtering is wanted.
pub(crate) fn prepare_present_sampler(
    mut pipelines: ResMut<PresentPipelines>,
    device: Res<RenderDevice>,
) {
    if pipelines.sampler.is_none() {
        pipelines.sampler = Some(device.create_sampler(&SamplerDescriptor {
            label: Some("ve_present_sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        }));
    }
}

/// Queues the present pipeline for every output format that needs one.
pub(crate) fn queue_present_pipelines(
    mut pipelines: ResMut<PresentPipelines>,
    pipeline_cache: Res<PipelineCache>,
    views: Query<&ViewTarget>,
) {
    for format in views.iter().filter_map(ViewTarget::out_texture_view_format) {
        if needs_blit(format) && !pipelines.cache.contains_key(&format) {
            let id = pipeline_cache.queue_render_pipeline(pipelines.descriptor(format));
            pipelines.cache.insert(format, id);
        }
    }
}

/// Copies the resolved main texture to the output for views that cannot render to it directly.
pub(crate) fn present_pass(
    view: ViewQuery<(&ViewTarget, &ViewOffscreen)>,
    pipelines: Res<PresentPipelines>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (target, offscreen) = view.into_inner();
    let Some(format) = target.out_texture_view_format().filter(|f| needs_blit(*f)) else {
        return;
    };
    let (Some(pipeline), Some(sampler)) = (
        pipelines
            .cache
            .get(&format)
            .and_then(|id| pipeline_cache.get_render_pipeline(*id)),
        pipelines.sampler.as_ref(),
    ) else {
        return;
    };
    let layout = pipeline_cache.get_bind_group_layout(&pipelines.layout);
    let bind_group = device.create_bind_group(
        "ve_present_bind_group",
        &layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&offscreen.0.default_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    );
    // The present pass is the one that writes the output, so it must take the output attachment from
    // the view (which records that it was written; see the note in `pass.rs`).
    let Some(output) = target.out_texture_color_attachment(None) else {
        return;
    };
    let color_attachments = [Some(output)];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_present_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}
