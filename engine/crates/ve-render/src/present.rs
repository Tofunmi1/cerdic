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
use bevy_render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
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

use crate::ao_depth::ViewAoDepth;
use crate::lens::{LensGpu, LensUniform, LensWarp};
use crate::prepare::{ViewSlots, ViewUniform, ViewUniforms};
use crate::quality::RenderQuality;
use crate::targets::{ViewOffscreen, blits};

/// Source of the present shader.
pub(crate) const PRESENT_SHADER: &str = include_str!("shaders/present.wgsl");

/// The present shader's asset name.
pub(crate) const PRESENT_SHADER_NAME: &str = "ve_present.wgsl";

/// The present pipelines, one per output format and whether the lens warp is on.
#[derive(Resource)]
pub(crate) struct PresentPipelines {
    layout: BindGroupLayoutDescriptor,
    lens_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    sampler: Option<Sampler>,
    lens_sampler: Option<Sampler>,
    post_layout: BindGroupLayoutDescriptor,
    cache: HashMap<(TextureFormat, bool, bool), CachedRenderPipelineId>,
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
        let lens_layout = BindGroupLayoutDescriptor::new(
            "ve_present_lens_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<LensUniform>(false),
                ),
            ),
        );
        let post_layout = BindGroupLayoutDescriptor::new(
            "ve_present_post_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<LensUniform>(false),
                    // The depth pass's float colour texture: the GL backend cannot read a real depth texture (see `ao_depth`).
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    uniform_buffer::<ViewUniform>(true),
                    // The `ao` pass's occlusion texture.
                    texture_2d(TextureSampleType::Float { filterable: false }),
                ),
            ),
        );
        Self {
            layout,
            lens_layout,
            post_layout,
            shader,
            sampler: None,
            lens_sampler: None,
            cache: HashMap::new(),
        }
    }

    fn descriptor(
        &self,
        output_format: TextureFormat,
        lens: bool,
        post: bool,
    ) -> RenderPipelineDescriptor {
        // Non-sRGB outputs get the transfer function applied by hand.
        let entry = match (post, output_format.is_srgb()) {
            (true, true) => "fragment_post",
            (true, false) => "fragment_post_srgb",
            (false, true) if lens => "fragment_lens",
            (false, false) if lens => "fragment_lens_srgb",
            (false, true) => "fragment_copy",
            (false, false) => "fragment_srgb",
        };
        let layout = if post {
            &self.post_layout
        } else if lens {
            &self.lens_layout
        } else {
            &self.layout
        };
        RenderPipelineDescriptor {
            label: Some("ve_present".into()),
            layout: vec![layout.clone()],
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
    // The lens warp resamples between pixels, so it filters.
    if pipelines.lens_sampler.is_none() {
        pipelines.lens_sampler = Some(device.create_sampler(&SamplerDescriptor {
            label: Some("ve_present_lens_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        }));
    }
}

/// Queues the present pipeline for every output format that needs one.
pub(crate) fn queue_present_pipelines(
    mut pipelines: ResMut<PresentPipelines>,
    pipeline_cache: Res<PipelineCache>,
    lens: Res<LensWarp>,
    quality: Res<RenderQuality>,
    views: Query<&ViewTarget>,
) {
    let warped = lens.active();
    let post = quality.ssao_enabled || quality.bloom_enabled;
    for format in views.iter().filter_map(ViewTarget::out_texture_view_format) {
        let key = (format, warped, post);
        if blits(format, *lens, *quality) && !pipelines.cache.contains_key(&key) {
            let id =
                pipeline_cache.queue_render_pipeline(pipelines.descriptor(format, warped, post));
            pipelines.cache.insert(key, id);
        }
    }
}

/// Copies the resolved main texture to the output for views that cannot render to it directly.
pub(crate) fn present_pass(
    view: ViewQuery<(
        &ViewTarget,
        &ViewOffscreen,
        Option<&ViewAoDepth>,
        &ViewSlots,
    )>,
    pipelines: Res<PresentPipelines>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    lens: Res<LensWarp>,
    lens_gpu: Res<LensGpu>,
    quality: Res<RenderQuality>,
    view_uniforms: Res<ViewUniforms>,
    mut ctx: RenderContext,
) {
    let (target, offscreen, depth, slots) = view.into_inner();
    let post = quality.ssao_enabled || quality.bloom_enabled;
    let Some(format) = target
        .out_texture_view_format()
        .filter(|f| blits(*f, *lens, *quality))
    else {
        return;
    };
    let warped = lens.active();
    let sampler = if warped || post {
        pipelines.lens_sampler.as_ref()
    } else {
        pipelines.sampler.as_ref()
    };
    let (Some(pipeline), Some(sampler)) = (
        pipelines
            .cache
            .get(&(format, warped, post))
            .and_then(|id| pipeline_cache.get_render_pipeline(*id)),
        sampler,
    ) else {
        return;
    };
    let Some(bind_group) = build_present_bind_group(
        &pipelines,
        &pipeline_cache,
        &device,
        offscreen,
        sampler,
        &lens_gpu,
        depth,
        &view_uniforms,
        warped,
        post,
    ) else {
        return;
    };
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
    if post {
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
    } else {
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn build_present_bind_group(
    pipelines: &PresentPipelines,
    pipeline_cache: &PipelineCache,
    device: &RenderDevice,
    offscreen: &ViewOffscreen,
    sampler: &Sampler,
    lens_gpu: &LensGpu,
    depth: Option<&ViewAoDepth>,
    view_uniforms: &ViewUniforms,
    warped: bool,
    post: bool,
) -> Option<bevy_render::render_resource::BindGroup> {
    let mut entries = vec![
        BindGroupEntry {
            binding: 0,
            resource: BindingResource::TextureView(&offscreen.0.default_view),
        },
        BindGroupEntry {
            binding: 1,
            resource: BindingResource::Sampler(sampler),
        },
    ];
    let layout_descriptor = if post {
        let uniform = lens_gpu.buffer.binding()?;
        entries.push(BindGroupEntry {
            binding: 2,
            resource: uniform,
        });
        let depth = depth?;
        entries.push(BindGroupEntry {
            binding: 3,
            resource: BindingResource::TextureView(&depth.color.default_view),
        });
        let view_uniform = view_uniforms.buffer.binding()?;
        entries.push(BindGroupEntry {
            binding: 4,
            resource: view_uniform,
        });
        entries.push(BindGroupEntry {
            binding: 5,
            resource: BindingResource::TextureView(&depth.occlusion.default_view),
        });
        &pipelines.post_layout
    } else if warped {
        let uniform = lens_gpu.buffer.binding()?;
        entries.push(BindGroupEntry {
            binding: 2,
            resource: uniform,
        });
        &pipelines.lens_layout
    } else {
        &pipelines.layout
    };
    let layout = pipeline_cache.get_bind_group_layout(layout_descriptor);
    Some(device.create_bind_group("ve_present_bind_group", &layout, &entries))
}
