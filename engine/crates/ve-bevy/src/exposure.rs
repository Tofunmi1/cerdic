//! Automatic exposure on the WebGL2 build. Bevy's own auto exposure is a compute shader, so this is a pair of fragment passes before tone
//! mapping: one measures the lit HDR picture into a 1 by 1 float texture (the exposure scale, smoothed over time), the other multiplies
//! the picture by it. Two such textures swap every frame, so no readback to the CPU is needed. The manual exposure (the viewer's slider)
//! is part of the measurement, so with auto exposure on the picture settles at the target wherever the slider is.

use bevy_app::{App, Plugin};
use bevy_asset::{Assets, Handle};
use bevy_core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy_ecs::prelude::*;
use bevy_math::{UVec4, Vec4};
use bevy_render::camera::ExtractedCamera;
use bevy_render::render_resource::binding_types::{texture_2d, uniform_buffer};
use bevy_render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId,
    ColorTargetState, ColorWrites, DynamicUniformBuffer, Extent3d, LoadOp, Operations,
    PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, ShaderStages, ShaderType,
    StoreOp, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, TextureView, TextureViewDescriptor,
};
use bevy_render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy_render::view::{ExtractedView, ViewTarget};
use bevy_render::{Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};
use bevy_shader::Shader;
use ve_render::{FrameDelta, Grading, RenderQuality};

use crate::ao::{descriptor, multiply_blend};

const MEASURE_SHADER: &str = include_str!("shaders/exposure_measure.wgsl");
const APPLY_SHADER: &str = include_str!("shaders/exposure_apply.wgsl");

/// Format of the exposure textures: one 32-bit float, renderable and read with `textureLoad`.
const FORMAT: TextureFormat = TextureFormat::R32Float;
/// The brightness (linear) the controller steers the picture's geometric mean towards: 18% grey.
const TARGET: f32 = 0.18;
/// Per second, how fast the scale closes on its target: about a third of a second to cover half the error.
const SPEED: f32 = 2.0;

/// Adds the auto exposure passes to the render world.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyExposurePlugin;

impl Plugin for VeBevyExposurePlugin {
    fn build(&self, app: &mut App) {
        let (measure, apply) = {
            let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
            (
                shaders.add(Shader::from_wgsl(
                    MEASURE_SHADER,
                    "ve_bevy_exposure_measure.wgsl",
                )),
                shaders.add(Shader::from_wgsl(
                    APPLY_SHADER,
                    "ve_bevy_exposure_apply.wgsl",
                )),
            )
        };
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(ExposureShaders { measure, apply })
            .init_resource::<ExposureControl>()
            .init_resource::<ExposureState>()
            .init_resource::<ExposureUniforms>()
            .add_systems(ExtractSchedule, extract_control)
            .add_systems(RenderStartup, init)
            .add_systems(
                Render,
                prepare_views.in_set(RenderSystems::PrepareResources),
            )
            .add_systems(
                Core3d,
                auto_exposure
                    .after(crate::ao::AoSet)
                    .in_set(Core3dSystems::EarlyPostProcess),
            );
    }
}

#[derive(Resource)]
struct ExposureShaders {
    measure: Handle<Shader>,
    apply: Handle<Shader>,
}

/// What the render world needs of the main world each frame.
#[derive(Resource, Default)]
struct ExposureControl {
    enabled: bool,
    manual: f32,
    /// The share of the correction this frame applies (0 to 1).
    adapt: f32,
}

fn extract_control(
    mut control: ResMut<ExposureControl>,
    quality: Extract<Res<RenderQuality>>,
    grading: Extract<Res<Grading>>,
    delta: Extract<Res<FrameDelta>>,
) {
    control.enabled = quality.auto_exposure_enabled;
    control.manual = grading.exposure;
    control.adapt = 1.0 - (-delta.0.clamp(0.0, 0.5) * SPEED).exp();
}

struct Slot {
    // Held so the texture outlives its view and can be written again.
    _texture: Texture,
    view: TextureView,
}

/// The two 1 by 1 exposure textures that swap each frame, and the pipelines that fill and apply them.
#[derive(Resource, Default)]
struct ExposureState {
    slots: Option<[Slot; 2]>,
    /// The texture read this frame; the other is written.
    current: usize,
    /// The passes ran last frame, so the scale in `current` is live.
    was_enabled: bool,
    pipelines: Option<Pipelines>,
}

struct Pipelines {
    measure_layout: BindGroupLayoutDescriptor,
    apply_layout: BindGroupLayoutDescriptor,
    measure: CachedRenderPipelineId,
    apply: CachedRenderPipelineId,
}

fn init(
    mut state: ResMut<ExposureState>,
    shaders: Res<ExposureShaders>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
) {
    let slot = || {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("ve_bevy_exposure"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        Slot {
            _texture: texture,
            view,
        }
    };
    state.slots = Some([slot(), slot()]);
    let float = || texture_2d(TextureSampleType::Float { filterable: false });
    let measure_layout = BindGroupLayoutDescriptor::new(
        "ve_bevy_exposure_measure_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (float(), float(), uniform_buffer::<Params>(true)),
        ),
    );
    let apply_layout = BindGroupLayoutDescriptor::new(
        "ve_bevy_exposure_apply_layout",
        &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, float()),
    );
    let measure = pipeline_cache.queue_render_pipeline(descriptor(
        "ve_bevy_exposure_measure",
        &measure_layout,
        &shaders.measure,
        ColorTargetState {
            format: FORMAT,
            blend: None,
            write_mask: ColorWrites::ALL,
        },
    ));
    let apply = pipeline_cache.queue_render_pipeline(descriptor(
        "ve_bevy_exposure_apply",
        &apply_layout,
        &shaders.apply,
        ColorTargetState {
            format: TextureFormat::Rgba16Float,
            blend: Some(multiply_blend()),
            write_mask: ColorWrites::ALL,
        },
    ));
    state.pipelines = Some(Pipelines {
        measure_layout,
        apply_layout,
        measure,
        apply,
    });
}

/// The measuring pass's inputs: the region of the target the view draws and the controller's settings.
#[derive(ShaderType, Clone)]
struct Params {
    viewport: Vec4,
    control: Vec4,
}

#[derive(Resource, Default)]
struct ExposureUniforms {
    params: DynamicUniformBuffer<Params>,
}

/// Where a view's parameters sit in the uniform buffer.
#[derive(Component)]
struct ExposureTarget {
    offset: u32,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "viewports are below 2^24 pixels"
)]
fn viewport_of(rect: UVec4) -> Vec4 {
    Vec4::new(rect.x as f32, rect.y as f32, rect.z as f32, rect.w as f32)
}

fn prepare_views(
    mut commands: Commands,
    control: Res<ExposureControl>,
    mut state: ResMut<ExposureState>,
    mut uniforms: ResMut<ExposureUniforms>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    views: Query<(Entity, &ExtractedView), With<ExtractedCamera>>,
) {
    uniforms.params.clear();
    // The scale written last frame is the one to read now; after a pause the old scale is stale and is forgotten.
    let forget = control.enabled && !state.was_enabled;
    if control.enabled && state.was_enabled {
        state.current ^= 1;
    }
    state.was_enabled = control.enabled;
    for (entity, view) in &views {
        if !control.enabled {
            commands.entity(entity).remove::<ExposureTarget>();
            continue;
        }
        let offset = uniforms.params.push(&Params {
            viewport: viewport_of(view.viewport),
            control: Vec4::new(
                control.manual,
                control.adapt,
                TARGET,
                if forget { 1.0 } else { 0.0 },
            ),
        });
        commands.entity(entity).insert(ExposureTarget { offset });
    }
    uniforms.params.write_buffer(&device, &queue);
}

fn auto_exposure(
    view: ViewQuery<(&ViewTarget, &ExposureTarget)>,
    state: Res<ExposureState>,
    pipeline_cache: Res<PipelineCache>,
    uniforms: Res<ExposureUniforms>,
    mut ctx: RenderContext,
) {
    let (target, exposure) = view.into_inner();
    let (Some(slots), Some(pipelines)) = (state.slots.as_ref(), state.pipelines.as_ref()) else {
        return;
    };
    let (Some(measure), Some(apply), Some(params)) = (
        pipeline_cache.get_render_pipeline(pipelines.measure),
        pipeline_cache.get_render_pipeline(pipelines.apply),
        uniforms.params.binding(),
    ) else {
        return;
    };
    let (read, write) = (&slots[state.current].view, &slots[state.current ^ 1].view);
    let device = ctx.render_device().clone();
    let measure_group = device.create_bind_group(
        "ve_bevy_exposure_measure_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipelines.measure_layout),
        &BindGroupEntries::sequential((target.main_texture_view(), read, params)),
    );
    let apply_group = device.create_bind_group(
        "ve_bevy_exposure_apply_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipelines.apply_layout),
        &BindGroupEntries::single(write),
    );
    {
        let attachments = [Some(RenderPassColorAttachment {
            view: write,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(bevy_color::LinearRgba::WHITE.into()),
                store: StoreOp::Store,
            },
        })];
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("ve_bevy_exposure_measure_pass"),
            color_attachments: &attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_render_pipeline(measure);
        pass.set_bind_group(0, &measure_group, &[exposure.offset]);
        pass.draw(0..3, 0..1);
    }
    let attachments = [Some(RenderPassColorAttachment {
        view: target.main_texture_view(),
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Load,
            store: StoreOp::Store,
        },
    })];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_bevy_exposure_apply_pass"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(apply);
    pass.set_bind_group(0, &apply_group, &[]);
    pass.draw(0..3, 0..1);
}
