//! The uniform evaluates the CPU field, including per-material seed, direction, ripple and flow.
use super::*;
use crate::{ProceduralRecipe, VeMaterial};

#[test]
fn uploaded_coefficients_evaluate_the_reference_height_and_normal() {
    let look = SurfaceWater {
        seed: 73,
        direction: -135.0,
        ripple: 1.7,
        flow: 2.4,
        ..SurfaceWater::default()
    };
    let field = WaveField::new(look.seed, look.ripple, look.flow, look.direction);
    let uniform = WaterUniform::new(look);
    for (x, z, t) in [(1.2, 4.0, 0.0), (-12.0, 3.0, 5.0), (70.0, -80.0, 19.0)] {
        let (mut height, mut dx, mut dz) = (0.0, 0.0, 0.0);
        for wave in uniform.waves {
            let p = wave.spatial;
            let phase = p.x * x + p.y * z - wave.temporal.x * t + p.w;
            height += p.z * phase.sin();
            dx += p.x * p.z * phase.cos();
            dz += p.y * p.z * phase.cos();
        }
        assert!((height - field.height(x, z, t)).abs() < 1e-6);
        let normal = bevy_math::Vec3::new(-dx, 1.0, -dz).normalize();
        assert!(normal.abs_diff_eq(field.normal(x, z, t).into(), 1e-6));
    }
}

#[test]
fn detail_has_a_separate_seed_and_obeys_ripple_and_flow() {
    let full = WaterUniform::new(SurfaceWater::default());
    let flat = WaterUniform::new(SurfaceWater {
        ripple: 0.0,
        flow: 0.0,
        ..SurfaceWater::default()
    });
    for wave in flat.waves.into_iter().chain(flat.detail) {
        assert_eq!(wave.spatial.z, 0.0);
        assert_eq!(wave.temporal.x, 0.0);
    }
    assert_ne!(full.detail[0].spatial.w, full.waves[0].spatial.w);
    assert!(full.detail[0].spatial.z < full.waves[0].spatial.z);
    assert!(full.detail[0].spatial.truncate().length() > full.waves[0].spatial.truncate().length());
}

#[test]
fn distinct_water_materials_on_one_mesh_get_distinct_batches() {
    use crate::{extract::ExtractedMesh, prepare::group_into_batches};
    use bevy_asset::{AssetId, uuid::Uuid};
    let make = |seed| ExtractedMesh {
        mesh: AssetId::Uuid {
            uuid: Uuid::from_u128(1),
        },
        world_from_local: bevy_math::Mat4::IDENTITY,
        material: VeMaterial {
            finish: crate::SurfaceFinish {
                water: Some(SurfaceWater {
                    seed,
                    ..SurfaceWater::default()
                }),
                ..crate::SurfaceFinish::default()
            },
            ..VeMaterial::default()
        }
        .pack_for_instance(0),
        transparent: false,
        sort_key: 0,
        center: bevy_math::Vec3::ZERO,
    };
    let (_, batches) = group_into_batches(&[make(1), make(1), make(2)]);
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].instances, 0..2);
    assert_eq!(batches[1].instances, 2..3);
    assert_ne!(
        WaterKey::from(batches[0].water.unwrap_or_default()),
        WaterKey::from(batches[1].water.unwrap_or_default())
    );
}

#[test]
fn water_finishes_and_legacy_water_recipes_reach_recipe_ten() {
    let material = VeMaterial {
        recipe: ProceduralRecipe::Water,
        texture_scale: 0.0,
        ..VeMaterial::default()
    };
    let packed = material.pack_for_instance(0);
    assert_eq!(packed.params[2], 10.0);
    assert_eq!(packed.water.unwrap_or_default().flow, 0.0);
    let finished = VeMaterial {
        finish: crate::SurfaceFinish {
            water: Some(SurfaceWater::default()),
            ..crate::SurfaceFinish::default()
        },
        ..VeMaterial::default()
    };
    assert_eq!(finished.pack_for_instance(0).params[2], 10.0);
}

#[test]
fn terrain_colors_split_batches_and_uniform_keys_on_the_same_mesh() {
    use crate::{
        SurfaceFinish, SurfaceTerrain, extract::ExtractedMesh, prepare::group_into_batches,
    };
    use bevy_asset::{AssetId, uuid::Uuid};
    let terrain = SurfaceTerrain {
        colors: [[0.4; 3]; 4],
        ..SurfaceTerrain::default()
    };
    let different = SurfaceTerrain {
        colors: [[0.8; 3]; 4],
        ..terrain
    };
    let make = |terrain| ExtractedMesh {
        mesh: AssetId::Uuid {
            uuid: Uuid::from_u128(1),
        },
        world_from_local: bevy_math::Mat4::IDENTITY,
        material: VeMaterial {
            finish: SurfaceFinish {
                terrain: Some(terrain),
                ..SurfaceFinish::default()
            },
            ..VeMaterial::default()
        }
        .pack_for_instance(0),
        transparent: false,
        sort_key: 0,
        center: bevy_math::Vec3::ZERO,
    };
    let (_, batches) = group_into_batches(&[make(terrain), make(terrain), make(different)]);
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].instances, 0..2);
    assert_eq!(batches[1].instances, 2..3);
    let first = crate::surface_terrain::TerrainUniform::new(batches[0].terrain);
    let second = crate::surface_terrain::TerrainUniform::new(batches[1].terrain);
    assert_ne!(first.key(), second.key());
    assert_eq!(first.c.y, 1.0);
    assert_eq!(crate::surface_terrain::TerrainUniform::new(None).c.y, 0.0);
}

#[test]
fn simulation_bounds_and_enable_flag_match_the_shader_uniform() {
    let still = WaterUniform::new(SurfaceWater::default());
    assert!(still.sim_enabled.x.abs() < 1e-6);
    let sim = WaterUniform::new(SurfaceWater {
        sim: true,
        sim_id: 7,
        sim_bounds: [-4.0, 10.0, 8.0, 6.0],
        ..SurfaceWater::default()
    });
    assert_eq!(sim.sim_bounds.to_array(), [-4.0, 10.0, 8.0, 6.0]);
    assert!((sim.sim_enabled.x - 1.0).abs() < 1e-6);
}
