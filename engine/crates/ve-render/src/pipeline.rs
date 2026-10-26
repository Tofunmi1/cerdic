//! The forward render pipeline and its per-mesh-layout, per-format, per-sample-count specialisation.

use std::collections::HashMap;

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_log::warn_once;
use bevy_mesh::{Mesh, MeshVertexBufferLayoutRef, VertexBufferLayout};
use bevy_render::mesh::RenderMesh;
use bevy_render::render_asset::RenderAssets;
use bevy_render::render_resource::binding_types::{
    sampler, texture_2d, texture_3d, texture_depth_2d, uniform_buffer,
};
use bevy_render::render_resource::{
    BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState, CachedRenderPipelineId,
    ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face,
    FragmentState, FrontFace, MultisampleState, PipelineCache, PrimitiveState, PrimitiveTopology,
    RenderPipelineDescriptor, SamplerBindingType, ShaderStages, StencilState, TextureFormat,
    TextureSampleType, VertexAttribute, VertexFormat, VertexState, VertexStepMode,
};
use bevy_render::view::{Msaa, ViewTarget};
use bevy_shader::Shader;

use crate::lens::LensWarp;
use crate::light::LightsUniform;
use crate::prepare::{InstanceBatches, InstanceData, ViewUniform};
use crate::quality::RenderQuality;
use crate::targets::{DEPTH_FORMAT, render_format};

pub(crate) use sim::FORWARD_SHADER;

/// The forward shader for meshes without texture coordinates: the same source with the vertex input and its
/// copy removed (plain WGSL has no preprocessor here). Meshes without baked maps use it, which keeps their
/// vertices 8 bytes smaller.
pub(crate) fn forward_shader_without_uv() -> String {
    FORWARD_SHADER
        .replace("    @location(11) uv: vec2<f32>,\n", "")
        .replace("    out.uv = input.uv;\n", "    out.uv = vec2<f32>(0.0);\n")
}

/// Coloured meshes use the occlusion attribute's location for the four water channels,
/// keeping the total at WebGL2's sixteen vertex attributes.
pub(crate) fn forward_shader_with_color(has_uv: bool) -> String {
    let source = if has_uv {
        FORWARD_SHADER.to_owned()
    } else {
        forward_shader_without_uv()
    };
    source
        .replace(
            "@location(2) occlusion: f32,",
            "@location(2) color: vec4<f32>,",
        )
        .replace("unpack_occlusion(input.occlusion)", "1.0")
        .replace(
            "outline_normal(input.occlusion, input.normal)",
            "input.normal",
        )
        .replace(
            "out.water_data = vec4<f32>(0.5, 0.5, 0.4, 0.0);",
            "out.water_data = input.color;",
        )
}

/// Which draw of a batch a pipeline serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PassKind {
    /// The ordinary opaque draw.
    Opaque,
    /// The alpha-blended draw.
    Transparent,
    /// The toon outline: a second draw with front faces culled and the vertices pushed outward.
    Outline,
    /// The opaque draw of a batch with foliage (leaves, grass, fences): thin cards seen from both sides, so nothing is culled.
    TwoSided,
}

type PipelineKey = (
    MeshVertexBufferLayoutRef,
    TextureFormat,
    u32,
    PassKind,
    bool,
    bool,
);

/// The forward pipeline family: shared layouts and shader, specialised lazily.
#[derive(Resource)]
pub(crate) struct ForwardPipelines {
    pub(crate) view_layout: BindGroupLayoutDescriptor,
    pub(crate) lights_layout: BindGroupLayoutDescriptor,
    pub(crate) textures_layout: BindGroupLayoutDescriptor,
    pub(crate) water_layout: BindGroupLayoutDescriptor,
    pub(crate) sim_shaders: Option<[Handle<Shader>; 4]>,
    shader: Handle<Shader>,
    shader_without_uv: Handle<Shader>,
    colored: Handle<Shader>,
    colored_without_uv: Handle<Shader>,
    cache: HashMap<PipelineKey, CachedRenderPipelineId>,
}

/// A view layout with only the view uniform: for passes that build their own bind group and have no use for the exposure texture
/// the shared view layout carries.
pub(crate) fn uniform_only_view_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "ve_uniform_only_view_layout",
        &BindGroupLayoutEntries::single(
            ShaderStages::VERTEX_FRAGMENT,
            uniform_buffer::<ViewUniform>(true),
        ),
    )
}

impl ForwardPipelines {
    pub(crate) fn new(
        shader: Handle<Shader>,
        shader_without_uv: Handle<Shader>,
        colored: Handle<Shader>,
        colored_without_uv: Handle<Shader>,
    ) -> Self {
        let view_layout = BindGroupLayoutDescriptor::new(
            "ve_view_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX_FRAGMENT,
                (
                    uniform_buffer::<ViewUniform>(true),
                    // The exposure now in use (a 1 by 1 float texture, see `exposure.rs`), read by the fragment stages.
                    texture_2d(TextureSampleType::Float { filterable: false })
                        .visibility(ShaderStages::FRAGMENT),
                ),
            ),
        );
        // Lights, then the sun's shadow map and the comparison sampler that reads it.
        let lights_layout = BindGroupLayoutDescriptor::new(
            "ve_lights_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    uniform_buffer::<LightsUniform>(false),
                    texture_depth_2d(),
                    sampler(SamplerBindingType::Comparison),
                    // The light probes (see `probes.rs`): four coefficient textures and the trilinear sampler that reads them.
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        );
        Self {
            view_layout,
            lights_layout,
            textures_layout: crate::textures::textures_layout(),
            water_layout: crate::water_gpu::layout(),
            sim_shaders: None,
            shader,
            shader_without_uv,
            colored,
            colored_without_uv,
            cache: HashMap::new(),
        }
    }

    /// The pipeline queued for this mesh layout, output format and sample count, if any.
    pub(crate) fn get(
        &self,
        layout: &MeshVertexBufferLayoutRef,
        format: TextureFormat,
        samples: u32,
        kind: PassKind,
        water: bool,
        sim: bool,
    ) -> Option<CachedRenderPipelineId> {
        self.cache
            .get(&(
                layout.clone(),
                format,
                samples,
                kind,
                water && layout.0.contains(Mesh::ATTRIBUTE_COLOR),
                sim,
            ))
            .copied()
    }

    fn descriptor(
        &self,
        mesh_layout: VertexBufferLayout,
        format: TextureFormat,
        samples: u32,
        has_uv: bool,
        kind: PassKind,
        has_color: bool,
        sim: bool,
    ) -> RenderPipelineDescriptor {
        let transparent = kind == PassKind::Transparent;
        let outline = kind == PassKind::Outline;
        // Meshes without baked maps carry no texture coordinates; their variant reads none.
        let shader = if has_color {
            if has_uv {
                self.colored.clone()
            } else {
                self.colored_without_uv.clone()
            }
        } else if has_uv {
            self.shader.clone()
        } else {
            self.shader_without_uv.clone()
        };
        let shader = if sim {
            self.sim_shaders.as_ref().map_or(shader.clone(), |shaders| {
                shaders[match (has_color, has_uv) {
                    (false, true) => 0,
                    (false, false) => 1,
                    (true, true) => 2,
                    (true, false) => 3,
                }]
                .clone()
            })
        } else {
            shader
        };
        RenderPipelineDescriptor {
            label: Some("ve_forward".into()),
            layout: vec![
                self.view_layout.clone(),
                self.lights_layout.clone(),
                self.textures_layout.clone(),
                if sim {
                    crate::water_gpu::sim_layout()
                } else {
                    self.water_layout.clone()
                },
            ],
            vertex: VertexState {
                shader: shader.clone(),
                entry_point: Some(if outline { "vertex_outline" } else { "vertex" }.into()),
                buffers: vec![mesh_layout, instance_layout()],
                ..Default::default()
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                // The outline hull shows only its back faces, which peek out around the silhouette.
                cull_mode: match kind {
                    PassKind::Outline => Some(Face::Front),
                    PassKind::TwoSided => None,
                    PassKind::Opaque | PassKind::Transparent => Some(Face::Back),
                },
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(!transparent),
                // Bevy projections are reverse-z: nearer fragments have larger depth.
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                shader,
                entry_point: Some(
                    if outline {
                        "fragment_outline"
                    } else {
                        "fragment"
                    }
                    .into(),
                ),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: transparent.then_some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}

/// Instance buffer layout: the model matrix as four `vec4` columns (locations 3 to 6), then base
/// colour, material parameters, emissive and texture-array layers (locations 7 to 10), then the three toon
/// vectors (locations 12 to 14; 11 is the mesh's texture coordinates), then the foliage vector (location 15).
pub(crate) fn instance_layout() -> VertexBufferLayout {
    let vec4 = u64::try_from(size_of::<[f32; 4]>()).unwrap_or(16);
    VertexBufferLayout {
        array_stride: u64::try_from(size_of::<InstanceData>()).unwrap_or(192),
        step_mode: VertexStepMode::Instance,
        attributes: (0..12_u32)
            .map(|i| VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: vec4 * u64::from(i),
                shader_location: if i < 8 { 3 + i } else { 4 + i },
            })
            .collect(),
    }
}

/// Queues a pipeline for every (mesh layout, output format, sample count) about to be drawn.
pub(crate) fn queue_forward_pipelines(
    mut pipelines: ResMut<ForwardPipelines>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<RenderMesh>>,
    batches: Res<InstanceBatches>,
    lens: Res<LensWarp>,
    quality: Res<RenderQuality>,
    views: Query<(&ViewTarget, &Msaa)>,
) {
    let variants: Vec<(TextureFormat, u32)> = views
        .iter()
        .filter_map(|(target, msaa)| {
            Some((
                render_format(target.out_texture_view_format()?, *lens, *quality),
                msaa.samples(),
            ))
        })
        .collect();
    for batch in batches.queue_batches() {
        // A mesh that is not uploaded yet is retried next frame.
        let Some(render_mesh) = render_meshes.get(batch.mesh) else {
            continue;
        };
        let layout = &render_mesh.layout;
        let has_uv = layout.0.contains(Mesh::ATTRIBUTE_UV_0);
        let has_color =
            batches.water_surface(batch).is_some() && layout.0.contains(Mesh::ATTRIBUTE_COLOR);
        let mut wanted = vec![
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            if has_color {
                Mesh::ATTRIBUTE_COLOR.at_shader_location(2)
            } else {
                crate::ATTRIBUTE_OCCLUSION.at_shader_location(2)
            },
        ];
        if has_uv {
            wanted.push(Mesh::ATTRIBUTE_UV_0.at_shader_location(11));
        }
        let Ok(mesh_layout) = layout.0.get_layout(&wanted) else {
            // A mesh without positions and normals cannot be drawn by this pipeline.
            warn_once!("pipeline queue: a mesh lacks POSITION or NORMAL attributes");
            continue;
        };
        for &(format, samples) in &variants {
            let kinds = [
                (PassKind::Opaque, true),
                (PassKind::Transparent, true),
                (PassKind::Outline, true),
                (PassKind::TwoSided, batch.foliage),
            ];
            for kind in kinds
                .into_iter()
                .filter_map(|(kind, wanted)| wanted.then_some(kind))
            {
                let sim = batches.water_surface(batch).is_some_and(|w| w.sim_id != 0);
                let key = (layout.clone(), format, samples, kind, has_color, sim);
                if pipelines.cache.contains_key(&key) {
                    continue;
                }
                let descriptor = pipelines.descriptor(
                    mesh_layout.clone(),
                    format,
                    samples,
                    has_uv,
                    kind,
                    has_color,
                    sim,
                );
                let id = pipeline_cache.queue_render_pipeline(descriptor);
                pipelines.cache.insert(key, id);
            }
        }
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;

#[path = "pipeline_sim.rs"]
mod sim;
pub(crate) use sim::without_sim;
