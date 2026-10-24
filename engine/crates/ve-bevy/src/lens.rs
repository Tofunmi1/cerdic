//! The lens warp (fisheye, or barrel distortion for a split-screen headset view). Bevy has no such step, so while a warp is on the views
//! render into an image of the window's size and a camera of its own, drawn last, resamples that image onto the window with the warp
//! shader (`shaders/lens.wgsl`, ported from the forward renderer's present pass). With the warp off none of this exists.

use std::collections::HashMap;

use bevy_app::{App, Plugin, Update};
use bevy_asset::{AssetId, Assets, Handle};
use bevy_camera::{Camera, Camera3d, ClearColorConfig, Projection, RenderTarget};
use bevy_color::LinearRgba;
use bevy_core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_math::{UVec2, Vec4};
use bevy_render::camera::{CameraRenderGraph, ExtractedCamera};
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy_render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId,
    ColorTargetState, ColorWrites, FilterMode, LoadOp, Operations, PipelineCache,
    RenderPassColorAttachment, RenderPassDescriptor, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderStages, ShaderType, StoreOp, TextureFormat, TextureSampleType,
    UniformBuffer,
};
use bevy_render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy_render::texture::GpuImage;
use bevy_render::view::{ExtractedView, Msaa, ViewTarget};
use bevy_render::{Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};
use bevy_shader::Shader;
use bevy_window::{PrimaryWindow, Window};
use ve_render::{LensMode, LensWarp};

use crate::ao::descriptor;
use crate::stereo::EyeCamera;

const SHADER: &str = include_str!("shaders/lens.wgsl");

/// The lens camera draws after every other camera.
const LENS_ORDER: isize = 1000;

/// The views render here, in the window's format and size, while a warp is on.
const SOURCE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

/// Adds the lens warp.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyLensPlugin;

impl Plugin for VeBevyLensPlugin {
    fn build(&self, app: &mut App) {
        let shader = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(SHADER, "ve_bevy_lens.wgsl"));
        app.init_resource::<LensSource>()
            .add_systems(Update, map_lens);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(LensShader(shader))
            .init_resource::<LensSettings>()
            .init_resource::<LensPipelines>()
            .init_resource::<LensUniform>()
            .add_systems(ExtractSchedule, extract_settings)
            .add_systems(RenderStartup, init)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
            .add_systems(Core3d, lens_present.in_set(Core3dSystems::PostProcess));
    }
}

/// The image the views render into while a warp is on, the camera that draws it, and whether the views were pointed at it.
#[derive(Resource, Default)]
struct LensSource {
    image: Option<Handle<Image>>,
    size: UVec2,
    camera: Option<Entity>,
}

/// The camera that resamples the image onto the window. It is not a 3D camera, so none of the scene systems see it.
#[derive(Component)]
struct LensCamera;

fn source_image(size: UVec2) -> Image {
    Image::new_target_texture(size.x.max(1), size.y.max(1), SOURCE_FORMAT, None)
}

fn map_lens(
    mut commands: Commands,
    lens: Res<LensWarp>,
    mut source: ResMut<LensSource>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut mains: Query<&mut RenderTarget, (With<Camera3d>, Without<EyeCamera>)>,
) {
    if lens.mode == LensMode::Off {
        if source.image.take().is_some() {
            // Point the views back at the window; the eye cameras copy their main camera's target.
            for mut target in &mut mains {
                *target = RenderTarget::default();
            }
        }
        if let Some(entity) = source.camera.take() {
            commands.entity(entity).despawn();
        }
        return;
    }
    let size = windows.single().map_or(UVec2::new(2, 1), |window| {
        UVec2::new(
            window.resolution.physical_width(),
            window.resolution.physical_height(),
        )
    });
    if source.image.is_none() || source.size != size {
        source.image = Some(images.add(source_image(size)));
        source.size = size;
    }
    let Some(handle) = source.image.clone() else {
        return;
    };
    for mut target in &mut mains {
        *target = RenderTarget::from(handle.clone());
    }
    if source.camera.is_none() {
        source.camera = Some(
            commands
                .spawn((
                    LensCamera,
                    Camera {
                        order: LENS_ORDER,
                        clear_color: ClearColorConfig::Custom(LinearRgba::BLACK.into()),
                        ..Camera::default()
                    },
                    CameraRenderGraph::new(Core3d),
                    Projection::default(),
                    Msaa::Off,
                ))
                .id(),
        );
    }
}

/// What the render world needs of the main world each frame.
#[derive(Resource, Default)]
struct LensSettings {
    mode: f32,
    half_fov_tan: f32,
    image: Option<AssetId<Image>>,
}

fn extract_settings(
    mut settings: ResMut<LensSettings>,
    lens: Extract<Res<LensWarp>>,
    source: Extract<Res<LensSource>>,
) {
    settings.mode = match lens.mode {
        LensMode::Off => 0.0,
        LensMode::Fisheye => 1.0,
        LensMode::Headset => 2.0,
    };
    settings.half_fov_tan = if lens.source_half_fov_tan.is_finite() {
        lens.source_half_fov_tan.clamp(0.05, 50.0)
    } else {
        1.0
    };
    settings.image = source.image.as_ref().map(Handle::id);
}

#[derive(Resource)]
struct LensShader(Handle<Shader>);

#[derive(Resource)]
struct LensPipelines {
    layout: Option<BindGroupLayoutDescriptor>,
    sampler: Option<Sampler>,
    by_format: HashMap<TextureFormat, CachedRenderPipelineId>,
}

impl Default for LensPipelines {
    fn default() -> Self {
        Self {
            layout: None,
            sampler: None,
            by_format: HashMap::new(),
        }
    }
}

/// What the shader reads: x the mode, y the tangent of half the source's vertical field of view.
#[derive(ShaderType, Clone, Default)]
struct LensParams {
    params: Vec4,
}

#[derive(Resource, Default)]
struct LensUniform(UniformBuffer<LensParams>);

fn init(mut pipelines: ResMut<LensPipelines>, device: Res<RenderDevice>) {
    pipelines.layout = Some(BindGroupLayoutDescriptor::new(
        "ve_bevy_lens_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<LensParams>(false),
            ),
        ),
    ));
    pipelines.sampler = Some(device.create_sampler(&SamplerDescriptor {
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        ..SamplerDescriptor::default()
    }));
}

/// Writes the warp's numbers and queues the pipeline for the window's format the first time it is needed.
fn prepare(
    settings: Res<LensSettings>,
    shader: Res<LensShader>,
    mut pipelines: ResMut<LensPipelines>,
    mut uniform: ResMut<LensUniform>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    views: Query<(&ExtractedCamera, &ExtractedView)>,
) {
    uniform.0.set(LensParams {
        params: Vec4::new(settings.mode, settings.half_fov_tan, 0.0, 0.0),
    });
    uniform.0.write_buffer(&device, &queue);
    let Some(layout) = pipelines.layout.clone() else {
        return;
    };
    for (camera, view) in &views {
        if camera.order == LENS_ORDER && !pipelines.by_format.contains_key(&view.target_format) {
            let target = ColorTargetState {
                format: view.target_format,
                blend: None,
                write_mask: ColorWrites::ALL,
            };
            let id = pipeline_cache.queue_render_pipeline(descriptor(
                "ve_bevy_lens",
                &layout,
                &shader.0,
                target,
            ));
            pipelines.by_format.insert(view.target_format, id);
        }
    }
}

fn lens_present(
    view: ViewQuery<(&ViewTarget, &ExtractedCamera, &ExtractedView)>,
    settings: Res<LensSettings>,
    pipelines: Res<LensPipelines>,
    pipeline_cache: Res<PipelineCache>,
    images: Res<RenderAssets<GpuImage>>,
    uniform: Res<LensUniform>,
    mut ctx: RenderContext,
) {
    let (target, camera, extracted) = view.into_inner();
    if camera.order != LENS_ORDER || settings.mode == 0.0 {
        return;
    }
    let (Some(layout), Some(sampler), Some(source), Some(pipeline), Some(params)) = (
        pipelines.layout.as_ref(),
        pipelines.sampler.as_ref(),
        settings.image.and_then(|id| images.get(id)),
        pipelines
            .by_format
            .get(&extracted.target_format)
            .and_then(|id| pipeline_cache.get_render_pipeline(*id)),
        uniform.0.binding(),
    ) else {
        return;
    };
    let device = ctx.render_device().clone();
    let bind_group = device.create_bind_group(
        "ve_bevy_lens_bind_group",
        &pipeline_cache.get_bind_group_layout(layout),
        &BindGroupEntries::sequential((&source.texture_view, sampler, params)),
    );
    let attachments = [Some(RenderPassColorAttachment {
        view: target.main_texture_view(),
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Clear(LinearRgba::BLACK.into()),
            store: StoreOp::Store,
        },
    })];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("ve_bevy_lens_pass"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}
