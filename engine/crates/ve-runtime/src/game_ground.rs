//! The ground a rolling ball runs on (ADR 0068): a grid of heights and surface ids built from the triangles of the authored parts a game's
//! `terrain` line names (and the footprints of the parts each `surface` line names), given to the game world and rebuilt when those parts change.
use std::sync::Arc;

use bevy_asset::Assets;
use bevy_ecs::{
    entity::Entity,
    resource::Resource,
    system::{Query, Res, ResMut},
};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_transform::components::Transform;
use ve_game::GroundField;

use crate::{game::GameRuntime, game_nav::append, projection::Authored};

/// Most height samples in a field (a 500 m course at 0.25 m); a bigger request is refused, not thinned.
const MAX_SAMPLES: usize = 4_000_000;

thread_local! {
    /// `(samples, error)` of the last build, for the status report.
    static REPORT: std::cell::RefCell<(usize, Option<String>)> = const { std::cell::RefCell::new((0, None)) };
}

/// `(samples in the field, the error of the last build if it failed)`.
pub(crate) fn report() -> (usize, Option<String>) {
    REPORT.with(|report| report.borrow().clone())
}

fn remember(samples: usize, error: Option<String>) {
    REPORT.with(|report| *report.borrow_mut() = (samples, error));
}

/// What the field was last built from.
#[derive(Resource, Default)]
pub(crate) struct GroundState {
    key: Option<(Vec<String>, Vec<Entity>)>,
    built: Option<Arc<GroundField>>,
}

type Triangles = (Vec<[f32; 3]>, Vec<u32>);

/// Covers the sample points inside each triangle's footprint, calling `visit(sample index, y)`.
fn rasterise(
    triangles: &Triangles,
    origin: [f32; 2],
    cell: f32,
    nx: usize,
    nz: usize,
    mut visit: impl FnMut(usize, f32),
) {
    let (vertices, indices) = triangles;
    for tri in indices.chunks_exact(3) {
        let (a, b, c) = (
            vertices[tri[0] as usize],
            vertices[tri[1] as usize],
            vertices[tri[2] as usize],
        );
        let denominator = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
        if denominator.abs() < 1e-12 {
            continue;
        }
        let (min_x, max_x) = (a[0].min(b[0]).min(c[0]), a[0].max(b[0]).max(c[0]));
        let (min_z, max_z) = (a[2].min(b[2]).min(c[2]), a[2].max(b[2]).max(c[2]));
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "grid indices of points inside the grid"
        )]
        let (i0, i1, j0, j1) = (
            (((min_x - origin[0]) / cell).ceil().max(0.0)) as usize,
            (((max_x - origin[0]) / cell).floor().max(0.0) as usize).min(nx - 1),
            (((min_z - origin[1]) / cell).ceil().max(0.0)) as usize,
            (((max_z - origin[1]) / cell).floor().max(0.0) as usize).min(nz - 1),
        );
        for j in j0..=j1 {
            for i in i0..=i1 {
                #[expect(clippy::cast_precision_loss, reason = "grid indices are small")]
                let (x, z) = (origin[0] + i as f32 * cell, origin[1] + j as f32 * cell);
                let w0 = ((b[2] - c[2]) * (x - c[0]) + (c[0] - b[0]) * (z - c[2])) / denominator;
                let w1 = ((c[2] - a[2]) * (x - c[0]) + (a[0] - c[0]) * (z - c[2])) / denominator;
                let w2 = 1.0 - w0 - w1;
                let slack = -1e-5;
                if w0 >= slack && w1 >= slack && w2 >= slack {
                    visit(j * nx + i, w0 * a[1] + w1 * b[1] + w2 * c[1]);
                }
            }
        }
    }
}

/// Builds the ground field of the loaded game when it declares terrain and its parts exist, and hands it to the game world.
pub(crate) fn sync(
    mut runtime: ResMut<GameRuntime>,
    mut state: ResMut<GroundState>,
    meshes: Res<Assets<Mesh>>,
    sources: Query<(Entity, &Authored, &Mesh3d, &Transform)>,
) {
    let declaration = runtime
        .world
        .as_ref()
        .and_then(|world| world.terrain_declaration().cloned());
    let Some(declaration) = declaration else {
        if state.key.take().is_some() {
            state.built = None;
            remember(0, None);
            if let Some(world) = runtime.world.as_mut() {
                world.set_ground(None);
            }
        }
        return;
    };
    let surfaces: Vec<(Vec<String>, u8)> = runtime
        .world
        .as_ref()
        .map(|world| {
            world
                .surface_declarations()
                .iter()
                .enumerate()
                .map(|(index, surface)| {
                    (
                        surface.parts.clone(),
                        u8::try_from(index + 1).unwrap_or(u8::MAX),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let named = |names: &[String]| -> Vec<(Entity, &Mesh3d, &Transform)> {
        let mut parts: Vec<(Entity, &Mesh3d, &Transform)> = sources
            .iter()
            .filter(|(_, authored, ..)| names.iter().any(|name| name == authored.name()))
            .map(|(entity, _, mesh, transform)| (entity, mesh, transform))
            .collect();
        parts.sort_by_key(|(entity, ..)| *entity);
        parts
    };
    let terrain_parts = named(&declaration.parts);
    let mut every_name = declaration.parts.clone();
    for (names, _) in &surfaces {
        every_name.extend(names.iter().cloned());
    }
    let key = (
        every_name.clone(),
        named(&every_name)
            .iter()
            .map(|(entity, ..)| *entity)
            .collect::<Vec<_>>(),
    );
    if terrain_parts.is_empty() {
        return;
    }
    if state.key.as_ref() == Some(&key) {
        if let (Some(field), Some(world)) = (state.built.clone(), runtime.world.as_mut())
            && world.ground().is_none()
        {
            world.set_ground(Some(field));
        }
        return;
    }
    state.key = Some(key);
    let triangles_of = |parts: &[(Entity, &Mesh3d, &Transform)]| -> Triangles {
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        for (_, handle, transform) in parts {
            if let Some(mesh) = meshes.get(&handle.0) {
                append(mesh, transform, &mut vertices, &mut indices);
            }
        }
        (vertices, indices)
    };
    let terrain = triangles_of(&terrain_parts);
    if terrain.0.is_empty() {
        return;
    }
    let (mut low, mut high) = ([f32::MAX; 2], [f32::MIN; 2]);
    for v in &terrain.0 {
        low = [low[0].min(v[0]), low[1].min(v[2])];
        high = [high[0].max(v[0]), high[1].max(v[2])];
    }
    #[expect(clippy::cast_possible_truncation, reason = "a cell size in metres")]
    let cell = declaration.cell as f32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of samples along an axis"
    )]
    let (nx, nz) = (
        ((high[0] - low[0]) / cell).ceil() as usize + 1,
        ((high[1] - low[1]) / cell).ceil() as usize + 1,
    );
    if nx < 2 || nz < 2 || nx.saturating_mul(nz) > MAX_SAMPLES {
        remember(
            0,
            Some(format!(
                "the terrain would need {nx} x {nz} samples at {cell} m; the limit is {MAX_SAMPLES} samples"
            )),
        );
        return;
    }
    let mut heights = vec![f32::NAN; nx * nz];
    rasterise(&terrain, low, cell, nx, nz, |index, y| {
        if heights[index].is_nan() || y > heights[index] {
            heights[index] = y;
        }
    });
    let mut ids = vec![0_u8; nx * nz];
    for (names, id) in &surfaces {
        let footprint = triangles_of(&named(names));
        rasterise(&footprint, low, cell, nx, nz, |index, _| ids[index] = *id);
    }
    match GroundField::new(
        [f64::from(low[0]), f64::from(low[1])],
        declaration.cell,
        nx,
        nz,
        heights,
        ids,
    ) {
        Ok(field) => {
            let field = Arc::new(field);
            remember(nx * nz, None);
            state.built = Some(Arc::clone(&field));
            if let Some(world) = runtime.world.as_mut() {
                world.set_ground(Some(field));
            }
        }
        Err(error) => remember(0, Some(error)),
    }
}
