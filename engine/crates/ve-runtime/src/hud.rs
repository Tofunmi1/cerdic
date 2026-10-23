//! World-space pixel-text panels for the live game.
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::visibility::Visibility;
use bevy_color::Color;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::With,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
};
use bevy_light::{NotShadowCaster, NotShadowReceiver};
use bevy_math::{Quat, Vec3};
use bevy_mesh::{Indices, Mesh, Mesh3d, PrimitiveTopology};
use bevy_time::{Real, Time};
use bevy_transform::components::Transform;
use ve_render::{StereoViews, VeMaterial};
use ve_xr_web::XrCamera;

const DISTANCE: f32 = 1.3;
const YAW_RATE: f32 = std::f32::consts::FRAC_PI_2;

#[derive(Component)]
pub(crate) struct HudDrawing;

/// Meshes and text snapshots for the current game's HUD panels.
#[derive(Resource, Default)]
pub(crate) struct HudRuntime {
    panels: Vec<Option<Entity>>,
    lines: Vec<String>,
    yaw: Option<f32>,
}

/// Builds changed text and positions the panels in front of the viewer.
pub(crate) fn update(
    mut commands: Commands,
    game: Res<crate::game::GameRuntime>,
    rig: Res<crate::viewer::ViewerRig>,
    mut state: ResMut<HudRuntime>,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time<Real>>,
    mut transforms: Query<&mut Transform, With<HudDrawing>>,
    xr_cameras: Query<&StereoViews, With<XrCamera>>,
) {
    let Some(world) = game.world.as_ref() else {
        clear(&mut commands, &mut state);
        return;
    };
    let definitions = crate::game_view::display_huds(world);
    if definitions.is_empty() {
        clear(&mut commands, &mut state);
        return;
    }

    let lines: Vec<String> = definitions.iter().map(|hud| hud.text.clone()).collect();
    let reset = state.panels.len() != definitions.len();
    if reset {
        clear(&mut commands, &mut state);
        state.panels.resize(definitions.len(), None);
        state.lines.resize(definitions.len(), String::new());
    }
    let (eye, target_yaw) = crate::game_view::eye_and_yaw(&rig, &xr_cameras);
    let yaw = state.yaw.get_or_insert(target_yaw);
    let delta = (target_yaw - *yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    *yaw += delta.clamp(-YAW_RATE * time.delta_secs(), YAW_RATE * time.delta_secs());
    let rotation = Quat::from_rotation_y(*yaw);
    let right = rotation * Vec3::X;
    for (index, (hud, line)) in definitions.iter().zip(&lines).enumerate() {
        // `state.lines` holds what was drawn last; a changed line is rebuilt, an unchanged one only moves below.
        if state.lines.get(index) == Some(line) {
            continue;
        }
        if let Some(entity) = state.panels[index].take() {
            commands.entity(entity).despawn();
        }
        state.lines[index].clone_from(line);
        let layout = ve_world::pixel_text::layout(line, pixel_size(hud.size));
        if layout.rects.is_empty() {
            continue;
        }
        let mesh = meshes.add(panel_mesh(&layout.rects, layout.width, layout.height));
        let (horizontal, vertical) = anchor_offset(&hud.anchor);
        let center =
            eye + rotation * (Vec3::NEG_Z * DISTANCE) + right * horizontal + Vec3::Y * vertical;
        let entity = commands
            .spawn((
                Mesh3d(mesh),
                hud_material(),
                Transform::from_translation(center).with_rotation(rotation),
                Visibility::Visible,
                NotShadowCaster,
                NotShadowReceiver,
                HudDrawing,
            ))
            .id();
        state.panels[index] = Some(entity);
    }

    let mut centers = Vec::new();
    for (index, hud) in definitions.iter().enumerate() {
        let (horizontal, vertical) = anchor_offset(&hud.anchor);
        let center =
            eye + rotation * (Vec3::NEG_Z * DISTANCE) + right * horizontal + Vec3::Y * vertical;
        centers.push(center.to_array());
        let Some(entity) = state.panels[index] else {
            continue;
        };
        if let Ok(mut transform) = transforms.get_mut(entity) {
            *transform = Transform::from_translation(center).with_rotation(rotation);
        }
    }
    CENTERS.with(|list| *list.borrow_mut() = centers);
    state.lines = lines;
}

thread_local! {
    static CENTERS: std::cell::RefCell<Vec<[f32; 3]>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Where each panel is centred this frame (world metres), for the status report.
pub(crate) fn centers() -> Vec<[f32; 3]> {
    CENTERS.with(|list| list.borrow().clone())
}

fn anchor_offset(anchor: &str) -> (f32, f32) {
    match anchor {
        "left" => (-0.45, -0.12),
        "right" => (0.45, -0.12),
        "top" => (0.0, 0.42),
        "bottom" => (0.0, -0.48),
        _ => (0.0, -0.16),
    }
}

fn clear(commands: &mut Commands, state: &mut HudRuntime) {
    for entity in state.panels.drain(..).flatten() {
        commands.entity(entity).despawn();
    }
    state.lines.clear();
    state.yaw = None;
}

fn hud_material() -> VeMaterial {
    VeMaterial {
        base_color: Color::srgb(1.0, 0.95, 0.8),
        emissive: Color::linear_rgb(1.0, 0.95, 0.8),
        roughness: 1.0,
        ..VeMaterial::default()
    }
}

fn panel_mesh(rects: &[[f32; 4]], width: f32, height: f32) -> Mesh {
    let mut positions = Vec::with_capacity(rects.len() * 4);
    let mut indices = Vec::with_capacity(rects.len() * 6);
    for (index, [x, y, w, h]) in rects.iter().copied().enumerate() {
        let left = x - width * 0.5;
        let top = height * 0.5 - y;
        let right = left + w;
        let bottom = top - h;
        positions.extend([
            [left, bottom, 0.0],
            [right, bottom, 0.0],
            [right, top, 0.0],
            [left, top, 0.0],
        ]);
        let base = u32::try_from(index * 4).unwrap_or(0);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let count = positions.len();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count]);
    mesh.insert_attribute(ve_render::ATTRIBUTE_OCCLUSION, vec![1.0_f32; count]);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "HUD pixel sizes are validated metre values and layout uses f32"
)]
fn pixel_size(size: f64) -> f32 {
    size as f32
}
