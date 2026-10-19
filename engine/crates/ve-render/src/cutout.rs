//! Cut-out variants of the depth-only pipelines.
//!
//! A leaf is a flat card whose shape lives in the alpha of its albedo map. The forward pass discards the transparent pixels,
//! and the passes that only write depth (the sun's shadow map, the depth the occlusion pass reads) must do the same, or a
//! leaf casts the shadow of its whole card. Their ordinary pipelines have no fragment stage and no texture coordinates, so a
//! batch with a cut-out instance draws with the variant here instead (see `Batch::cutout`): it also reads the mesh's
//! coordinates and the shared material maps, and an instance with no cutoff drops nothing.

use std::collections::HashMap;

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_mesh::{Mesh, MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_render::mesh::RenderMesh;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::{
    BindGroupLayoutDescriptor, CachedRenderPipelineId, ColorTargetState, ColorWrites,
    CompareFunction, DepthBiasState, DepthStencilState, FragmentState, MultisampleState,
    PipelineCache, PrimitiveState, PrimitiveTopology, RenderPipelineDescriptor, StencilState,
    VertexState,
};
use bevy_shader::Shader;

use crate::pipeline::{instance_layout, uniform_only_view_layout};
use crate::prepare::InstanceBatches;
use crate::quality::RenderQuality;
use crate::shadow::SHADOW_FORMAT;
use crate::targets::DEPTH_FORMAT;

/// Which depth-only pass a cut-out pipeline serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CutoutKind {
    /// The sun's shadow map.
    Shadow,
    /// The depth texture of the occlusion pass.
    Depth,
}

/// The cut-out pipelines, one per pass and mesh vertex layout.
#[derive(Resource)]
pub(crate) struct CutoutPipelines {
    shadow_shader: Handle<Shader>,
    depth_shader: Handle<Shader>,
    depth_view_layout: BindGroupLayoutDescriptor,
    textures_layout: BindGroupLayoutDescriptor,
    cache: HashMap<(CutoutKind, MeshVertexBufferLayoutRef), CachedRenderPipelineId>,
}

impl CutoutPipelines {
    /// `depth_view_layout` is the shared view layout the depth pass binds; the shadow pass uses its own uniform-only one.
    pub(crate) fn new(
        shadow_shader: Handle<Shader>,
        depth_shader: Handle<Shader>,
        depth_view_layout: BindGroupLayoutDescriptor,
    ) -> Self {
        Self {
            shadow_shader,
            depth_shader,
            depth_view_layout,
            textures_layout: crate::textures::textures_layout(),
            cache: HashMap::new(),
        }
    }

    /// The queued pipeline for a pass and mesh layout, if the mesh has the coordinates a cut-out needs.
    pub(crate) fn get(
        &self,
        kind: CutoutKind,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Option<CachedRenderPipelineId> {
        self.cache.get(&(kind, layout.clone())).copied()
    }

    fn descriptor(
        &self,
        kind: CutoutKind,
        mesh_layout: VertexBufferLayout,
    ) -> RenderPipelineDescriptor {
        let (shader, view_layout) = match kind {
            CutoutKind::Shadow => (&self.shadow_shader, uniform_only_view_layout()),
            CutoutKind::Depth => (&self.depth_shader, self.depth_view_layout.clone()),
        };
        let (depth_stencil, targets, cull_mode) = match kind {
            // Both faces cast, with the same bias as the plain shadow pipeline.
            CutoutKind::Shadow => (
                DepthStencilState {
                    format: SHADOW_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(CompareFunction::Less),
                    stencil: StencilState::default(),
                    bias: DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                },
                vec![],
                None,
            ),
            CutoutKind::Depth => (
                DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    // Reverse-z, as the forward pass: nearer fragments have larger depth.
                    depth_compare: Some(CompareFunction::GreaterEqual),
                    stencil: StencilState::default(),
                    bias: DepthBiasState::default(),
                },
                vec![Some(ColorTargetState {
                    format: crate::ao_depth::FORMAT,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                // Cut-out foliage is two-sided: its back faces count in the depth too.
                None,
            ),
        };
        RenderPipelineDescriptor {
            label: Some(
                match kind {
                    CutoutKind::Shadow => "ve_shadow_cutout",
                    CutoutKind::Depth => "ve_ao_depth_cutout",
                }
                .into(),
            ),
            layout: vec![view_layout, self.textures_layout.clone()],
            vertex: VertexState {
                shader: shader.clone(),
                entry_point: Some("vertex_cutout".into()),
                buffers: vec![mesh_layout, instance_layout()],
                ..Default::default()
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                cull_mode,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil),
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                shader: shader.clone(),
                entry_point: Some("fragment_cutout".into()),
                targets,
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

/// Queues the cut-out pipelines for every mesh layout with a cut-out batch, for the passes that are running.
pub(crate) fn queue_cutout_pipelines(
    mut pipelines: ResMut<CutoutPipelines>,
    quality: Res<RenderQuality>,
    shadows: Res<crate::shadow::ShadowState>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    batches: Res<InstanceBatches>,
) {
    let wanted = [
        (CutoutKind::Shadow, shadows.params.enabled),
        (CutoutKind::Depth, quality.ssao_enabled),
    ];
    for batch in batches.batches.iter().filter(|batch| batch.cutout) {
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            continue;
        };
        for (kind, running) in wanted {
            let key = (kind, render_mesh.layout.clone());
            if !running || pipelines.cache.contains_key(&key) {
                continue;
            }
            // A mesh with no texture coordinates has no alpha to read: it draws with the plain pipeline.
            let Ok(mesh_layout) = render_mesh.layout.0.get_layout(&[
                Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                Mesh::ATTRIBUTE_UV_0.at_shader_location(11),
            ]) else {
                continue;
            };
            let id = pipeline_cache.queue_render_pipeline(pipelines.descriptor(kind, mesh_layout));
            pipelines.cache.insert(key, id);
        }
    }
}
