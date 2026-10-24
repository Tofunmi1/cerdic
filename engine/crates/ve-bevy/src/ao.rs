//! Ambient occlusion on the WebGL2 build. Bevy's own SSAO is a compute shader, which WebGL2 does not have, so this is the engine's
//! earlier fragment-shader occlusion (`ve-render`'s `ao` and present passes) moved onto Bevy's prepass: the prepass's normal texture holds
//! the world normal and, with Bevy patch 4, the fragment depth in its alpha. Two passes follow the main passes: one writes the occlusion of
//! each pixel into a one-byte texture, the other multiplies the lit picture by its 4 by 4 depth-aware average.

use bevy_app::{App, Plugin};
use bevy_asset::{Assets, Handle};
use bevy_core_pipeline::prepass::ViewPrepassTextures;
use bevy_core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, UVec4, Vec4};
use bevy_render::camera::ExtractedCamera;
use bevy_render::render_resource::binding_types::{texture_2d, uniform_buffer};
use bevy_render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendComponent,
    BlendFactor, BlendOperation, BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites,
    DynamicUniformBuffer, Extent3d, FragmentState, LoadOp, Operations, PipelineCache,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages,
    ShaderType, StoreOp, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, VertexState,
};
use bevy_render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy_render::texture::{CachedTexture, TextureCache};
use bevy_render::view::{ExtractedView, ViewTarget};
use bevy_render::{Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};
use bevy_shader::Shader;
use ve_render::RenderQuality;

const AO_SHADER: &str = include_str!("shaders/ao.wgsl");
const APPLY_SHADER: &str = include_str!("shaders/ao_apply.wgsl");

/// Format of the occlusion texture: one normalised byte, renderable on every WebGL2 device.
const OCCLUSION_FORMAT: TextureFormat = TextureFormat::R8Unorm;

/// Adds the occlusion passes to the render world.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyAoPlugin;

impl Plugin for VeBevyAoPlugin {
    fn build(&self, app: &mut App) {
        let (ao, apply) = {
            let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
            (
                shaders.add(Shader::from_wgsl(AO_SHADER, "ve_bevy_ao.wgsl")),
                shaders.add(Shader::from_wgsl(APPLY_SHADER, "ve_bevy_ao_apply.wgsl")),
            )
        };
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .insert_resource(AoShaders { ao, apply })
            .init_resource::<AoUniforms>()
            .init_resource::<AoEnabled>()
            .add_systems(ExtractSchedule, extract_enabled)
            .add_systems(RenderStartup, init_pipelines)
            .add_systems(
                Render,
                prepare_views.in_set(RenderSystems::PrepareResources),
            )
            .add_systems(
                Core3d,
                ambient_occlusion
                    .in_set(AoSet)
                    .in_set(Core3dSystems::EarlyPostProcess),
            );
    }
}

/// The occlusion passes, so later passes can be ordered after them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct AoSet;

/// Whether the render world draws occlusion this frame: the quality setting, copied over each frame.
#[derive(Resource, Default)]
struct AoEnabled(bool);

fn extract_enabled(mut enabled: ResMut<AoEnabled>, quality: Extract<Res<RenderQuality>>) {
    enabled.0 = quality.ssao_enabled;
}

#[derive(Resource)]
struct AoShaders {
    ao: Handle<Shader>,
    apply: Handle<Shader>,
}

#[derive(Resource)]
struct AoPipelines {
    ao_layout: BindGroupLayoutDescriptor,
    apply_layout: BindGroupLayoutDescriptor,
    ao: CachedRenderPipelineId,
    apply: CachedRenderPipelineId,
}

fn init_pipelines(
    mut commands: Commands,
    shaders: Res<AoShaders>,
    pipeline_cache: Res<PipelineCache>,
) {
    let ao_layout = BindGroupLayoutDescriptor::new(
        "ve_bevy_ao_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<AoView>(true),
            ),
        ),
    );
    let apply_layout = BindGroupLayoutDescriptor::new(
        "ve_bevy_ao_apply_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<Region>(true),
            ),
        ),
    );
    let ao = pipeline_cache.queue_render_pipeline(descriptor(
        "ve_bevy_ao",
        &ao_layout,
        &shaders.ao,
        ColorTargetState {
            format: OCCLUSION_FORMAT,
            blend: None,
            write_mask: ColorWrites::ALL,
        },
    ));
    let apply = pipeline_cache.queue_render_pipeline(descriptor(
        "ve_bevy_ao_apply",
        &apply_layout,
        &shaders.apply,
        ColorTargetState {
            format: TextureFormat::Rgba16Float,
            blend: Some(multiply_blend()),
            write_mask: ColorWrites::ALL,
        },
    ));
    commands.insert_resource(AoPipelines {
        ao_layout,
        apply_layout,
        ao,
        apply,
    });
}

/// Blending that multiplies the colour already in the target by the shader's output and leaves the target's alpha alone.
pub(crate) fn multiply_blend() -> BlendState {
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::Src,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    }
}

/// A full-screen pipeline: the shader's `vertex` and `fragment` entry points drawing into one colour target.
pub(crate) fn descriptor(
    label: &'static str,
    layout: &BindGroupLayoutDescriptor,
    shader: &Handle<Shader>,
    target: ColorTargetState,
) -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
        label: Some(label.into()),
        layout: vec![layout.clone()],
        vertex: VertexState {
            shader: shader.clone(),
            entry_point: Some("vertex".into()),
            ..Default::default()
        },
        fragment: Some(FragmentState {
            shader: shader.clone(),
            entry_point: Some("fragment".into()),
            targets: vec![Some(target)],
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// What the occlusion pass needs of a view: the matrices to rebuild positions from depth, the eye, and the viewport in target pixels.
#[derive(ShaderType, Clone)]
struct AoView {
    clip_from_world: Mat4,
    world_from_clip: Mat4,
    camera_position: Vec4,
    viewport: Vec4,
}

/// The region of the target a view draws, for the apply pass.
#[derive(ShaderType, Clone)]
struct Region {
    viewport: Vec4,
}

#[derive(Resource, Default)]
struct AoUniforms {
    views: DynamicUniformBuffer<AoView>,
    regions: DynamicUniformBuffer<Region>,
}

/// A view's occlusion texture and where its uniforms sit in the buffers.
#[derive(Component)]
struct AoTarget {
    occlusion: CachedTexture,
    view_offset: u32,
    region_offset: u32,
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
    enabled: Res<AoEnabled>,
    mut texture_cache: ResMut<TextureCache>,
    mut uniforms: ResMut<AoUniforms>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    views: Query<(
        Entity,
        &ExtractedCamera,
        &ExtractedView,
        &ViewPrepassTextures,
    )>,
) {
    uniforms.views.clear();
    uniforms.regions.clear();
    for (entity, camera, view, prepass) in &views {
        let Some(size) = camera
            .physical_target_size
            .filter(|_| enabled.0 && prepass.normal.is_some())
        else {
            commands.entity(entity).remove::<AoTarget>();
            continue;
        };
        let occlusion = texture_cache.get(
            &device,
            TextureDescriptor {
                label: Some("ve_bevy_ao_occlusion"),
                size: Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: OCCLUSION_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let clip_from_world = view
            .clip_from_world
            .unwrap_or_else(|| view.clip_from_view * view.world_from_view.to_matrix().inverse());
        let viewport = viewport_of(view.viewport);
        let view_offset = uniforms.views.push(&AoView {
            clip_from_world,
            world_from_clip: clip_from_world.inverse(),
            camera_position: view.world_from_view.translation().extend(1.0),
            viewport,
        });
        let region_offset = uniforms.regions.push(&Region { viewport });
        commands.entity(entity).insert(AoTarget {
            occlusion,
            view_offset,
            region_offset,
        });
    }
    uniforms.views.write_buffer(&device, &queue);
    uniforms.regions.write_buffer(&device, &queue);
}

fn ambient_occlusion(
    view: ViewQuery<(&ViewTarget, &ViewPrepassTextures, &AoTarget)>,
    pipelines: Res<AoPipelines>,
    pipeline_cache: Res<PipelineCache>,
    uniforms: Res<AoUniforms>,
    mut ctx: RenderContext,
) {
    let (target, prepass, ao) = view.into_inner();
    let (
        Some(normals),
        Some(ao_pipeline),
        Some(apply_pipeline),
        Some(view_binding),
        Some(region_binding),
    ) = (
        prepass.normal_view(),
        pipeline_cache.get_render_pipeline(pipelines.ao),
        pipeline_cache.get_render_pipeline(pipelines.apply),
        uniforms.views.binding(),
        uniforms.regions.binding(),
    )
    else {
        return;
    };
    let device = ctx.render_device().clone();
    let ao_group = device.create_bind_group(
        "ve_bevy_ao_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipelines.ao_layout),
        &BindGroupEntries::sequential((normals, view_binding)),
    );
    let apply_group = device.create_bind_group(
        "ve_bevy_ao_apply_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipelines.apply_layout),
        &BindGroupEntries::sequential((normals, &ao.occlusion.default_view, region_binding)),
    );
    {
        let attachments = [Some(RenderPassColorAttachment {
            view: &ao.occlusion.default_view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(bevy_color::LinearRgba::WHITE.into()),
                store: StoreOp::Store,
            },
        })];
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("ve_bevy_ao_pass"),
            color_attachments: &attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_render_pipeline(ao_pipeline);
        pass.set_bind_group(0, &ao_group, &[ao.view_offset]);
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
        label: Some("ve_bevy_ao_apply_pass"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(apply_pipeline);
    pass.set_bind_group(0, &apply_group, &[ao.region_offset]);
    pass.draw(0..3, 0..1);
}
