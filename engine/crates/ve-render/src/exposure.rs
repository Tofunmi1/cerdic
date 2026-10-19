//! Automatic exposure (eye adaptation), entirely on the GPU.
//!
//! Each frame, after the scene is drawn, a one-pixel pass averages the picture's brightness (the geometric mean over a grid of
//! samples), compares it with a target, and writes a new exposure into a 1 by 1 float texture. Two such textures swap every
//! frame: the view bind group exposes the current one to every shader (the forward pass multiplies the scene by it, the sky
//! too), and the pass reads it and writes the other. No readback to the CPU is needed, which the browser makes slow.
//!
//! Tonemapping sits inside the forward shader, so the measurement is of the displayed picture, not of scene radiance. That
//! makes this a feedback controller: it corrects part of the remaining error in stops each frame, scaled by the frame time so
//! adaptation takes the same time at any frame rate. The maths in [`next_exposure`] is mirrored in `shaders/exposure.wgsl`.

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_log::info_once;
use bevy_render::Extract;
use bevy_render::render_resource::binding_types::texture_2d;
use bevy_render::render_resource::{
    BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BindingResource,
    CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d, FragmentState, LoadOp,
    MultisampleState, Operations, Origin3d, PipelineCache, PrimitiveState,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages,
    StoreOp, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, VertexState,
};
use bevy_render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy_shader::Shader;

use crate::prepare::{ViewSlots, ViewUniforms};
use crate::quality::RenderQuality;
use crate::targets::ViewOffscreen;

/// Source of the exposure shader.
pub(crate) const SHADER: &str = include_str!("shaders/exposure.wgsl");

/// The shader's asset name.
pub(crate) const SHADER_NAME: &str = "ve_exposure.wgsl";

/// Format of the exposure textures: one 32-bit float, renderable and read with `textureLoad`.
pub(crate) const FORMAT: TextureFormat = TextureFormat::R32Float;

/// The display brightness (linear, after tonemapping) the controller steers the picture's geometric mean towards: 18% grey.
pub(crate) const TARGET: f32 = 0.18;
/// Per second, how fast the exposure closes on its target: about a third of a second to cover half the error.
pub(crate) const SPEED: f32 = 2.0;
// The controller itself runs in `exposure.wgsl`; its Rust mirror below exists so the maths can be tested (a test also keeps the
// constants of the two in step).

/// The exposure may change by this many stops either way (a factor of 64 each way: from deep shade to a sunlit scene).
#[cfg(test)]
pub(crate) const EV_LIMIT: f32 = 6.0;
/// The largest error in stops the controller reacts to in one step, so a flash or a black frame does not swing it.
#[cfg(test)]
const ERROR_LIMIT: f32 = 3.0;
/// The share of the remaining error corrected per unit of adaptation. Tonemapping compresses the response (a stop of
/// exposure moves the picture by less than a stop), so correcting less than all of it settles without overshooting.
#[cfg(test)]
const GAIN: f32 = 0.6;

/// The frame time in seconds, set by the host each frame, so the adaptation speed does not depend on the frame rate.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FrameDelta(pub f32);

impl Default for FrameDelta {
    fn default() -> Self {
        Self(1.0 / 60.0)
    }
}

/// Copies the main-world [`FrameDelta`] into the render world each frame.
pub(crate) fn extract_frame_delta(mut commands: Commands, delta: Extract<Res<FrameDelta>>) {
    commands.insert_resource(**delta);
}

/// How much of the correction a frame of `delta` seconds applies (0 to 1).
#[must_use]
pub(crate) fn adaptation(delta: f32) -> f32 {
    1.0 - (-delta.clamp(0.0, 0.5) * SPEED).exp()
}

/// The exposure for the next frame, from this frame's exposure, the measured brightness of the picture and the adaptation
/// of the frame. Mirrors `next_exposure` in `exposure.wgsl`.
#[cfg(test)]
#[must_use]
pub(crate) fn next_exposure(previous: f32, measured: f32, target: f32, adapt: f32) -> f32 {
    let error = (target / measured.max(1.0e-4))
        .log2()
        .clamp(-ERROR_LIMIT, ERROR_LIMIT);
    (previous.max(1.0e-4).log2() + error * GAIN * adapt)
        .clamp(-EV_LIMIT, EV_LIMIT)
        .exp2()
}

struct Target {
    // Held so the texture outlives its view and can be written again.
    texture: Texture,
    view: TextureView,
}

/// The two 1 by 1 exposure textures that swap each frame.
#[derive(Resource, Default)]
pub(crate) struct ExposureState {
    targets: Option<[Target; 2]>,
    current: usize,
    /// The exposure pass ran this frame, so the other texture holds the newer value.
    advance: bool,
    /// A texture holds a value other than 1, to be reset when auto exposure is switched off.
    adapted: bool,
}

impl ExposureState {
    /// The texture the shaders read this frame.
    pub(crate) fn current_view(&self) -> Option<&TextureView> {
        self.targets
            .as_ref()
            .map(|targets| &targets[self.current].view)
    }

    fn next_view(&self) -> Option<&TextureView> {
        self.targets
            .as_ref()
            .map(|targets| &targets[1 - self.current].view)
    }
}

fn write_value(queue: &RenderQueue, texture: &Texture, value: f32) {
    queue.write_texture(
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &value.to_le_bytes(),
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
}

/// Creates the textures at exposure 1, swaps them after a frame in which the pass ran, and resets them when auto exposure is off.
pub(crate) fn prepare_exposure_state(
    mut state: ResMut<ExposureState>,
    quality: Res<RenderQuality>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    if state.targets.is_none() {
        let make = || {
            let texture = device.create_texture(&TextureDescriptor {
                label: Some("ve_exposure"),
                size: Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            write_value(&queue, &texture, 1.0);
            let view = texture.create_view(&TextureViewDescriptor::default());
            Target { texture, view }
        };
        state.targets = Some([make(), make()]);
    }
    if state.advance {
        state.current = 1 - state.current;
        state.advance = false;
        state.adapted = true;
    }
    if !quality.auto_exposure_enabled && state.adapted {
        if let Some(targets) = &state.targets {
            for target in targets {
                write_value(&queue, &target.texture, 1.0);
            }
        }
        state.adapted = false;
    }
}

/// The exposure pipeline: the view bind group (the uniform and the current exposure) and the picture to measure.
#[derive(Resource)]
pub(crate) struct ExposurePipeline {
    view_layout: BindGroupLayoutDescriptor,
    color_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    id: Option<CachedRenderPipelineId>,
}

impl ExposurePipeline {
    pub(crate) fn new(shader: Handle<Shader>, view_layout: BindGroupLayoutDescriptor) -> Self {
        let color_layout = BindGroupLayoutDescriptor::new(
            "ve_exposure_color_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::FRAGMENT,
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        );
        Self {
            view_layout,
            color_layout,
            shader,
            id: None,
        }
    }

    fn descriptor(&self) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("ve_exposure".into()),
            layout: vec![self.view_layout.clone(), self.color_layout.clone()],
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

/// Queues the exposure pipeline the first time auto exposure is on.
pub(crate) fn queue_exposure_pipeline(
    mut pipeline: ResMut<ExposurePipeline>,
    quality: Res<RenderQuality>,
    pipeline_cache: Res<PipelineCache>,
) {
    if quality.auto_exposure_enabled && pipeline.id.is_none() {
        pipeline.id = Some(pipeline_cache.queue_render_pipeline(pipeline.descriptor()));
    }
}

/// Measures the finished picture of the view and writes the next exposure. With several cameras the last one measured wins.
pub(crate) fn exposure_pass(
    view: ViewQuery<(&ViewOffscreen, &ViewSlots)>,
    quality: Res<RenderQuality>,
    mut state: ResMut<ExposureState>,
    pipeline: Res<ExposurePipeline>,
    pipeline_cache: Res<PipelineCache>,
    view_uniforms: Res<ViewUniforms>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    if !quality.auto_exposure_enabled {
        return;
    }
    let (offscreen, slots) = view.into_inner();
    let (Some(render_pipeline), Some(view_group), Some(slot), Some(target)) = (
        pipeline
            .id
            .and_then(|id| pipeline_cache.get_render_pipeline(id)),
        view_uniforms.bind_group.as_ref(),
        slots.0.first(),
        state.next_view(),
    ) else {
        info_once!(
            "exposure pass skipped: its pipeline, the view bind group or the exposure textures are not ready"
        );
        return;
    };
    let color_layout = pipeline_cache.get_bind_group_layout(&pipeline.color_layout);
    let color_group = device.create_bind_group(
        "ve_exposure_color_group",
        &color_layout,
        &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::TextureView(&offscreen.0.default_view),
        }],
    );
    let color_attachments = [Some(RenderPassColorAttachment {
        view: target,
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Clear(bevy_color::LinearRgba::WHITE.into()),
            store: StoreOp::Store,
        },
    })];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_exposure_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(render_pipeline);
    pass.set_bind_group(0, view_group, &[slot.offset]);
    pass.set_bind_group(1, &color_group, &[]);
    pass.draw(0..3, 0..1);
    drop(pass);
    info_once!("exposure pass ran");
    state.advance = true;
}

#[cfg(test)]
#[path = "exposure_tests.rs"]
mod tests;
