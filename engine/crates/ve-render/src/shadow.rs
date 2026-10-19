//! The sun's shadow map: its texture and the pass that fills it (the pipelines, the sun cameras and this frame's parameters are
//! in `shadow_pipelines`).
//!
//! Up to three cascaded orthographic maps share one atlas and follow the viewer (see `shadow_math`). They are sampled in the forward shader
//! with a comparison sampler. When shadows are off the lights bind group still needs a texture, so a
//! 1 by 1 stand-in is bound and the shader skips the lookup.

use bevy_ecs::prelude::*;
use bevy_render::mesh::RenderMesh;
use bevy_render::mesh::allocator::MeshAllocator;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{
    CompareFunction, Extent3d, FilterMode, LoadOp, Operations, PipelineCache,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, Sampler, SamplerDescriptor, StoreOp,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor,
};
use bevy_render::renderer::{RenderContext, RenderDevice};

use crate::counters::{self, Pass};
use crate::cutout::{CutoutKind, CutoutPipelines};
use crate::geometry::bind_geometry;
use crate::prepare::InstanceBatches;
use crate::quality::RenderQuality;
use crate::shadow_math::MAX_CASCADES;
pub(crate) use crate::shadow_pipelines::{
    ShadowPipelines, ShadowState, prepare_shadow_state, queue_shadow_pipelines,
};

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
    /// Side of one cascade in texels.
    resolution: u32,
    /// Cascades laid side by side: the atlas is `resolution * cascades` wide.
    cascades: u32,
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

    /// Side of one cascade of the real map in texels and how many cascades it has, or `None` when shadows are off.
    pub(crate) fn layout(&self) -> Option<(u32, u32)> {
        self.target
            .as_ref()
            .map(|target| (target.resolution, target.cascades))
    }
}

fn depth_texture(
    device: &RenderDevice,
    width: u32,
    height: u32,
    label: &'static str,
) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width,
            height,
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
        map.stand_in = Some(depth_texture(&device, 1, 1, "ve_shadow_stand_in"));
    }
    let most = u32::try_from(MAX_CASCADES).unwrap_or(3);
    let wanted = quality.shadows_enabled().then_some((
        quality.shadow_resolution,
        quality.shadow_cascades.clamp(1, most),
    ));
    if map.layout() != wanted {
        map.target = wanted.map(|(resolution, cascades)| {
            let (texture, view) = depth_texture(
                &device,
                resolution * cascades,
                resolution,
                "ve_shadow_atlas",
            );
            ShadowTarget {
                _texture: texture,
                view,
                resolution,
                cascades,
            }
        });
    }
}

/// Renders every batch into the shadow map from the sun's point of view.
pub(crate) fn shadow_pass(
    state: Res<ShadowState>,
    map: Res<ShadowMap>,
    pipelines: Res<ShadowPipelines>,
    cutouts: Res<CutoutPipelines>,
    textures: Res<crate::textures::TexturesGpu>,
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
    let (Some(target), Some(view_bind_group)) =
        (map.target.as_ref(), pipelines.view_bind_group.as_ref())
    else {
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
    // Every cascade draws the entity batches and the instance-set chunks inside its own box, from its own sun camera, into its own
    // tile of the atlas.
    for (index, offset) in pipelines.offsets.iter().enumerate() {
        let tile = u32::try_from(index).unwrap_or(0) * target.resolution;
        #[expect(
            clippy::cast_precision_loss,
            reason = "shadow tiles are far below 2^24 texels"
        )]
        pass.set_viewport(
            tile as f32,
            0.0,
            target.resolution as f32,
            target.resolution as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(tile, 0, target.resolution, target.resolution);
        pass.set_bind_group(0, view_bind_group, &[*offset]);
        for batch in batches.cascade_batches(index) {
            let Some(render_mesh) = render_meshes.get(batch.mesh) else {
                continue;
            };
            // A batch with a cut-out instance draws with the variant that tests the albedo alpha, when its mesh can.
            let cutout = batch
                .cutout
                .then(|| cutouts.get(CutoutKind::Shadow, &render_mesh.layout))
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
            if let Some(call) =
                bind_geometry(&mut pass, &allocator, render_mesh, batch, instance_buffer)
            {
                call.issue(&mut pass, batch.instances.clone(), Pass::Shadow);
            }
        }
    }
}
