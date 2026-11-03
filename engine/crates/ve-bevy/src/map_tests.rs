use super::*;
use bevy_camera::visibility::NoFrustumCulling;
use bevy_pbr::MeshMaterial3d;
use ve_render::MaterialTextures;

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<RenderQuality>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<ToonMaterial>>()
        .init_resource::<Assets<DetailMaterial>>()
        .init_resource::<Assets<TerrainMaterial>>()
        .init_resource::<Assets<WaterMaterial>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<MapImages>()
        .init_resource::<MaterialTextures>()
        .add_systems(Update, map_materials);
    app
}

fn standard_handles(app: &mut App) -> Vec<Handle<StandardMaterial>> {
    let world = app.world_mut();
    let mut query = world.query::<&MeshMaterial3d<StandardMaterial>>();
    query.iter(world).map(|m| m.0.clone()).collect()
}

#[test]
fn identical_surfaces_share_one_material_so_copies_can_batch() {
    let mut app = app();
    let same = VeMaterial::default();
    for _ in 0..50 {
        app.world_mut().spawn(same);
    }
    app.update();
    let handles = standard_handles(&mut app);
    assert_eq!(handles.len(), 50);
    assert!(
        handles.iter().all(|h| h.id() == handles[0].id()),
        "fifty copies, one material"
    );
    assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 1);
}

#[test]
fn different_surfaces_get_different_materials() {
    let mut app = app();
    let rough = VeMaterial {
        roughness: 0.9,
        ..VeMaterial::default()
    };
    let shiny = VeMaterial {
        roughness: 0.1,
        ..VeMaterial::default()
    };
    app.world_mut().spawn(rough);
    app.world_mut().spawn(shiny);
    app.world_mut().spawn(rough);
    app.update();
    let handles = standard_handles(&mut app);
    assert_eq!(handles.len(), 3);
    assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 2);
}

#[test]
fn a_particle_surface_is_flat_double_sided_and_blends_or_adds() {
    let particle = |mode| {
        standard_material(
            &VeMaterial {
                finish: ve_render::SurfaceFinish {
                    particle: mode,
                    ..Default::default()
                },
                ..VeMaterial::default()
            },
            &mut |_, _| None,
        )
    };
    let alpha = particle(1);
    assert!(alpha.unlit && alpha.double_sided && alpha.cull_mode.is_none());
    assert!(matches!(alpha.alpha_mode, AlphaMode::Blend));
    let additive = particle(2);
    assert!(additive.unlit && matches!(additive.alpha_mode, AlphaMode::Add));
    // An ordinary surface is lit and opaque.
    let plain = particle(0);
    assert!(!plain.unlit && matches!(plain.alpha_mode, AlphaMode::Opaque));
}

#[test]
fn a_static_part_is_not_exempt_from_frustum_culling() {
    let mut app = app();
    let entity = app.world_mut().spawn(VeMaterial::default()).id();
    app.update();
    assert!(
        app.world()
            .entity(entity)
            .get::<NoFrustumCulling>()
            .is_none(),
        "only skinned parts opt out, and the runtime marks those"
    );
}
