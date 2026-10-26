//! Grass parameters share the water/terrain uniform, keeping all 16 WebGL2 attributes intact.
use crate::water_waves::finite_or;
use crate::{FoliageStyle, extract::ExtractedMeshes, instance_sets::ExtractedSets};
use bevy_asset::{AssetId, Assets};
use bevy_camera::primitives::MeshAabb;
use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_mesh::Mesh;
use bevy_render::{Extract, render_resource::ShaderType};
use std::collections::HashMap;

/// Exact local-space minimum y and inverse height for each grass mesh, including every LOD.
#[derive(Resource, Default)]
pub(crate) struct GrassBounds(HashMap<AssetId<Mesh>, [f32; 2]>);

/// Extract once per distinct mesh, rather than once per blade instance.
pub(crate) fn extract_grass_bounds(
    mut bounds: ResMut<GrassBounds>,
    meshes: Extract<Res<Assets<Mesh>>>,
    entities: Res<ExtractedMeshes>,
    sets: Res<ExtractedSets>,
) {
    bounds.0.clear();
    let ids = entities
        .0
        .iter()
        .filter(|m| m.material.grass.is_some())
        .map(|m| m.mesh)
        .chain(
            sets.0
                .iter()
                .filter(|(_, _, m)| m.foliage.grass)
                .flat_map(|(_, set, _)| set.meshes.iter().map(|m| m.mesh.id())),
        );
    for id in ids {
        bounds.0.entry(id).or_insert_with(|| {
            meshes
                .get(id)
                .and_then(Mesh::get_aabb)
                .map_or([0.0, 0.0], |aabb| {
                    let height = aabb.half_extents.y * 2.0;
                    [
                        aabb.center.y - aabb.half_extents.y,
                        if height > 1e-6 { height.recip() } else { 0.0 },
                    ]
                })
        });
    }
}

/// Shader-compatible look and height range; bound once per surface and mesh.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub(crate) struct GrassUniform {
    pub(crate) ground: Vec4,
    pub(crate) look: Vec4,
    pub(crate) shape: Vec4,
}
impl GrassUniform {
    pub(crate) fn new(style: Option<FoliageStyle>, bounds: [f32; 2]) -> Self {
        let Some(s) = style.filter(|s| s.grass) else {
            return Self::default();
        };
        let c = s.ground_color.map(|v| finite_or(v, 0.0).clamp(0.0, 1.0));
        let c = bevy_color::Color::srgb(c[0], c[1], c[2]).to_linear();
        Self {
            ground: Vec4::new(
                c.red,
                c.green,
                c.blue,
                finite_or(s.ground_blend, 0.0).clamp(0.0, 1.0),
            ),
            look: Vec4::new(
                1.0,
                finite_or(s.translucency, 0.0).clamp(0.0, 1.0),
                finite_or(s.exposure_boost, 1.0).max(0.1),
                0.0,
            ),
            shape: Vec4::new(
                finite_or(s.fade_near, 25.0),
                finite_or(s.fade_far, 70.0),
                bounds[0],
                bounds[1],
            ),
        }
    }
    pub(crate) fn key(self) -> [u32; 12] {
        let values = [self.ground, self.look, self.shape];
        std::array::from_fn(|i| values[i / 4][i % 4].to_bits())
    }
}
impl GrassBounds {
    pub(crate) fn uniform(&self, style: Option<FoliageStyle>, mesh: AssetId<Mesh>) -> GrassUniform {
        GrassUniform::new(style, self.0.get(&mesh).copied().unwrap_or([0.0; 2]))
    }
}

#[cfg(test)]
#[path = "grass_gpu_tests.rs"]
mod tests;
