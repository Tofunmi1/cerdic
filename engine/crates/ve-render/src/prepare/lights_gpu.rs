//! The lights uniform for each frame and the bind group over it and the sun's shadow map.

use bevy_ecs::prelude::*;
use bevy_render::render_resource::{
    BindGroup, BindGroupEntry, BindingResource, PipelineCache, UniformBuffer,
};
use bevy_render::renderer::{RenderDevice, RenderQueue};
use bevy_render::view::ExtractedView;

use crate::light::{ExtractedLights, LightsUniform, pack_lights};
use crate::pipeline::ForwardPipelines;
use crate::probes::ProbeGpu;
use crate::quality::RenderQuality;
use crate::shadow::{ShadowMap, ShadowState};
use crate::stereo::{StereoViews, viewer_position};

/// The lights uniform for this frame plus its bind group.
#[derive(Resource, Default)]
pub(crate) struct LightsGpu {
    pub(crate) buffer: UniformBuffer<LightsUniform>,
    pub(crate) bind_group: Option<BindGroup>,
}

/// Packs the extracted lights (capped by the quality tier) and rebuilds the lights bind group.
pub(crate) fn prepare_lights(
    lights: Res<ExtractedLights>,
    quality: Res<RenderQuality>,
    views: Query<(&ExtractedView, Option<&StereoViews>)>,
    shadow: Res<ShadowState>,
    shadow_map: Res<ShadowMap>,
    probes: Res<ProbeGpu>,
    mut gpu: ResMut<LightsGpu>,
    pipelines: Res<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let viewer = viewer_position(
        views
            .iter()
            .map(|(view, stereo)| (view.world_from_view.translation(), stereo)),
    );
    let mut uniform = pack_lights(
        &lights,
        viewer,
        quality.max_point_lights,
        quality.max_spot_lights,
        &shadow.params,
    );
    uniform.gi_origin_spacing = probes.origin_spacing;
    uniform.gi_dims = probes.dims_enabled;
    gpu.buffer.set(uniform);
    gpu.buffer.write_buffer(&device, &queue);
    let (Some(binding), Some(map_view), Some(map_sampler), Some(probe_views), Some(probe_sampler)) = (
        gpu.buffer.binding(),
        shadow_map.sampled_view(),
        shadow_map.sampler(),
        probes.views(),
        probes.sampler(),
    ) else {
        gpu.bind_group = None;
        return;
    };
    let layout = pipeline_cache.get_bind_group_layout(&pipelines.lights_layout);
    gpu.bind_group = Some(device.create_bind_group(
        "ve_lights_bind_group",
        &layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: binding,
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(map_view),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Sampler(map_sampler),
            },
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::TextureView(&probe_views[0]),
            },
            BindGroupEntry {
                binding: 4,
                resource: BindingResource::TextureView(&probe_views[1]),
            },
            BindGroupEntry {
                binding: 5,
                resource: BindingResource::TextureView(&probe_views[2]),
            },
            BindGroupEntry {
                binding: 6,
                resource: BindingResource::TextureView(&probe_views[3]),
            },
            BindGroupEntry {
                binding: 7,
                resource: BindingResource::Sampler(probe_sampler),
            },
        ],
    ));
}
