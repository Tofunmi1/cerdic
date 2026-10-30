//! The walkable grid the game's creatures path on (ADR 0067): built from the triangles of the authored parts a `nav` line names, given to
//! the game world, and rebuilt when those parts change.
use std::sync::Arc;

use bevy_asset::Assets;
use bevy_ecs::{
    entity::Entity,
    resource::Resource,
    system::{Query, Res, ResMut},
};
use bevy_mesh::{Mesh, Mesh3d, VertexAttributeValues};
use bevy_transform::components::Transform;
use ve_nav::{NavConfig, NavMesh};

use crate::{game::GameRuntime, projection::Authored};

/// Most triangles taken from the named parts (a level's floor is far below this; a bigger input is refused, not truncated).
const MAX_TRIANGLES: usize = 400_000;

thread_local! {
    /// The last build's outcome for the status report: (walkable cells, error).
    static REPORT: std::cell::RefCell<(usize, Option<String>)> = const { std::cell::RefCell::new((0, None)) };
}

/// `(walkable cells, the error of the last build if it failed)`.
pub(crate) fn report() -> (usize, Option<String>) {
    REPORT.with(|report| report.borrow().clone())
}

fn remember(cells: usize, error: Option<String>) {
    REPORT.with(|report| *report.borrow_mut() = (cells, error));
}

/// What the grid was last built from, so a frame that changes nothing builds nothing.
#[derive(Resource, Default)]
pub(crate) struct NavState {
    key: Option<(Vec<String>, Vec<Entity>)>,
    /// The grid built for `key`, given again to a game world that was made after it (loading a game makes a new world).
    built: Option<Arc<NavMesh>>,
    /// Why the last build gave no grid, for the status report.
    pub(crate) error: Option<String>,
    pub(crate) walkable_cells: usize,
}

/// The triangles of one mesh in world space, appended to `vertices` and `indices`.
pub(crate) fn append(
    mesh: &Mesh,
    transform: &Transform,
    vertices: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
) {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    let base = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
    vertices.extend(positions.iter().map(|p| {
        transform
            .transform_point(bevy_math::Vec3::from(*p))
            .to_array()
    }));
    match mesh.indices() {
        Some(list) => indices.extend(list.iter().map(|i| base + u32::try_from(i).unwrap_or(0))),
        None => indices.extend((0..u32::try_from(positions.len()).unwrap_or(0)).map(|i| base + i)),
    }
}

/// Builds the grid for the loaded game when it declares one and the named parts exist, and hands it to the game world.
pub(crate) fn sync(
    mut runtime: ResMut<GameRuntime>,
    mut state: ResMut<NavState>,
    meshes: Res<Assets<Mesh>>,
    sources: Query<(Entity, &Authored, &Mesh3d, &Transform)>,
) {
    let declaration = runtime
        .world
        .as_ref()
        .and_then(|world| world.nav_declaration().cloned());
    let Some(declaration) = declaration else {
        if state.key.take().is_some() {
            state.built = None;
            state.walkable_cells = 0;
            state.error = None;
            if let Some(world) = runtime.world.as_mut() {
                world.set_nav(None);
            }
        }
        return;
    };
    let mut parts: Vec<(Entity, &Mesh3d, &Transform)> = sources
        .iter()
        .filter(|(_, authored, ..)| declaration.parts.iter().any(|name| name == authored.name()))
        .map(|(entity, _, mesh, transform)| (entity, mesh, transform))
        .collect();
    parts.sort_by_key(|(entity, ..)| *entity);
    let key = (
        declaration.parts.clone(),
        parts.iter().map(|(entity, ..)| *entity).collect::<Vec<_>>(),
    );
    if parts.is_empty() {
        return;
    }
    if state.key.as_ref() == Some(&key) {
        // Nothing changed in the level; a world created since the last build still needs the grid.
        if let (Some(mesh), Some(world)) = (state.built.clone(), runtime.world.as_mut())
            && world.nav().is_none()
        {
            world.set_nav(Some(mesh));
        }
        return;
    }
    state.key = Some(key);
    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    for (_, handle, transform) in &parts {
        if let Some(mesh) = meshes.get(&handle.0) {
            append(mesh, transform, &mut vertices, &mut indices);
        }
    }
    if indices.len() / 3 > MAX_TRIANGLES {
        state.error = Some(format!(
            "the nav parts have {} triangles; the limit is {MAX_TRIANGLES}",
            indices.len() / 3
        ));
        remember(0, state.error.clone());
        return;
    }
    // The grid covers the parts' own box with a margin, so the cell count follows the size of the level.
    let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in &vertices {
        for axis in 0..3 {
            low[axis] = low[axis].min(v[axis]);
            high[axis] = high[axis].max(v[axis]);
        }
    }
    if vertices.is_empty() {
        return;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a nav declaration holds metres, well inside f32"
    )]
    let config = NavConfig::new(
        declaration.cell as f32,
        declaration.radius as f32,
        declaration.height as f32,
        declaration.step as f32,
        declaration.slope as f32,
        [low[0] - 0.5, low[1] - 1.0, low[2] - 0.5],
        [
            high[0] + 0.5,
            high[1] + declaration.height as f32 + 1.0,
            high[2] + 0.5,
        ],
    );
    match config.and_then(|config| NavMesh::build(&vertices, &indices, &config)) {
        Ok(mesh) => {
            state.walkable_cells = mesh.walkable_count();
            state.error = None;
            remember(state.walkable_cells, None);
            let mesh = Arc::new(mesh);
            state.built = Some(Arc::clone(&mesh));
            if let Some(world) = runtime.world.as_mut() {
                world.set_nav(Some(mesh));
            }
        }
        Err(error) => {
            state.walkable_cells = 0;
            state.error = Some(error.to_string());
            remember(0, state.error.clone());
            if let Some(world) = runtime.world.as_mut() {
                world.set_nav(None);
            }
        }
    }
}
