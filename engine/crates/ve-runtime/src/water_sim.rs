//! Per-part interactive water, sharing the animation clock and uploading once per frame.
#[path = "water_sim/grid.rs"]
mod grid;
use crate::{animation::AnimationClock, projection::Authored};
use bevy_asset::Assets;
use bevy_ecs::prelude::*;
use bevy_mesh::{Mesh, Mesh3d};
use bevy_transform::components::GlobalTransform;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use ve_render::water_sim::{WaterSimFrame, WaterSimTextures};
use ve_render::{SurfaceWater, VeMaterial};
use ve_world::water_sim::{SimOptions, WaterSim, rain::Rain};

struct Part {
    signature: Vec<u64>,
    sim: WaterSim,
    look: SurfaceWater,
    origin: [f32; 2],
    size: [f32; 2],
    dims: [u32; 2],
    level: f32,
    rain: Rain,
    id: u64,
    version: u64,
    steps: u64,
    volume: Option<ve_world::water_sim::volume::Volume>,
}
#[derive(Resource, Default)]
pub(crate) struct Simulations {
    parts: BTreeMap<String, Part>,
    next_id: u64,
    last_time: f32,
    budget: f32,
}

/// Builds a single grid across all render chunks of each part, and advances bounded fixed steps.
pub(crate) fn update(world: &mut World) {
    let mut state = world.remove_resource::<Simulations>().unwrap_or_default();
    let clock = *world.resource::<AnimationClock>();
    if clock.seconds < state.last_time {
        state.parts.clear();
        state.budget = 0.0;
    }
    for mut material in world.query::<&mut VeMaterial>().iter_mut(world) {
        if material.finish.water.is_some_and(|water| water.sim_id != 0)
            && let Some(water) = material.finish.water.as_mut()
        {
            water.sim_id = 0;
            water.sim_bounds = [0.0; 4];
        }
    }
    let mut query = world.query::<(Entity, &Authored, &Mesh3d, &VeMaterial, &GlobalTransform)>();
    let mut groups: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for (entity, authored, mesh, material, transform) in query.iter(world) {
        if let Some(look) = material.finish.water.filter(|w| w.sim) {
            groups.entry(authored.name().to_owned()).or_default().push((
                entity,
                mesh.0.clone(),
                look,
                *transform,
                material.recipe == ve_render::ProceduralRecipe::Water,
            ));
        }
    }
    // ECS query order can change as unrelated entities move between archetypes. A water part may
    // span several render chunks; keep both its signature and rasterization order stable.
    for chunks in groups.values_mut() {
        chunks.sort_by_key(|(entity, ..)| entity.to_bits());
    }
    let alive: BTreeSet<_> = groups.keys().cloned().collect();
    state.parts.retain(|name, _| alive.contains(name));
    for (name, chunks) in &groups {
        let signature: Vec<u64> = chunks
            .iter()
            .flat_map(|(entity, _, look, transform, _)| {
                std::iter::once(entity.to_bits())
                    .chain(
                        transform
                            .to_matrix()
                            .to_cols_array()
                            .map(|n| u64::from(n.to_bits())),
                    )
                    .chain([
                        u64::from(look.sim_resolution),
                        u64::from(look.sim_damping.to_bits()),
                        u64::from(look.sim_depth.to_bits()),
                        u64::from(look.rain.to_bits()),
                        u64::from(look.seed),
                        u64::from(look.volumetric),
                    ])
            })
            .collect();
        if let Some(part) = state
            .parts
            .get_mut(name)
            .filter(|p| p.signature == signature)
        {
            if let Some(chunk) = chunks.first() {
                part.look = chunk.2;
            }
            continue;
        }
        // A replacement that has no usable water footprint must not retain the old grid.
        state.parts.remove(name);
        state.next_id += 1;
        if let Some(part) = grid::build(
            world.resource::<Assets<Mesh>>(),
            chunks,
            signature,
            state.next_id,
        ) {
            state.parts.insert(name.clone(), part);
        }
    }
    let elapsed = clock.seconds - state.last_time;
    state.last_time = clock.seconds;
    if elapsed.is_finite() && elapsed > 0.0 && clock.playing {
        state.budget = (state.budget + elapsed).min(4.0 / 60.0);
    }
    let mut steps = 0;
    while state.budget >= 1.0 / 60.0 && steps < 4 {
        for part in state.parts.values_mut() {
            for [x, z] in part.rain.drops(1.0 / 60.0, part.look.rain, part.size) {
                if part.sim.is_water(x, z) {
                    #[expect(clippy::cast_precision_loss, reason = "at most 256 cells per side")]
                    let radius = 0.15_f32.max(part.size[0] / part.dims[0] as f32);
                    if let Some(volume) = part.volume.as_mut() {
                        volume.impulse_column(
                            [part.origin[0] + x, part.level, part.origin[1] + z],
                            radius * 2.0,
                            [0.0, -0.04, 0.0],
                        );
                    }
                    part.sim.disturb(x, z, radius, 0.006);
                }
            }
            if let Some(volume) = part.volume.as_mut() {
                let _ = volume.step(1.0 / 60.0);
            }
            part.sim.step(1.0 / 60.0);
            part.steps += 1;
            part.version += 1;
        }
        state.budget -= 1.0 / 60.0;
        steps += 1;
    }
    let mut textures = WaterSimTextures::default();
    for (name, chunks) in groups {
        let Some(part) = state.parts.get(&name) else {
            continue;
        };
        for (entity, _, _, _, _) in chunks {
            if let Some(mut material) = world.get_mut::<VeMaterial>(entity) {
                if let Some(water) = material.finish.water.as_mut() {
                    water.sim_id = part.id;
                    water.sim_bounds = [part.origin[0], part.origin[1], part.size[0], part.size[1]];
                }
            }
        }
        // Arc snapshots are reused when neither the clock nor an API disturbance changed the field.
        let previous = world.resource::<WaterSimTextures>().0.get(&part.id);
        let frame = previous
            .filter(|f| f.version == part.version)
            .cloned()
            .unwrap_or_else(|| WaterSimFrame {
                size: part.dims,
                version: part.version,
                heights: Arc::from(part.sim.heights()),
            });
        textures.0.insert(part.id, frame);
    }
    world.insert_resource(textures);
    world.insert_resource(state);
}

/// World API query/forcing; only water parts covering the requested point participate.
#[expect(
    clippy::cast_possible_truncation,
    reason = "finite world coordinates checked before conversion"
)]
pub(crate) fn method(
    world: &mut World,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    // Requests can arrive as soon as projection creates the meshes, before the first sim frame.
    update(world);
    if method == "water.status" {
        let state = world.resource::<Simulations>();
        let parts: Vec<_> = state
            .parts
            .iter()
            .map(|(name, part)| {
                serde_json::json!({
                    "name": name, "id": part.id,
                    "resolution": part.dims[0].max(part.dims[1]),
                    "cell": part.sim.cell(), "steps": part.steps,
                    "time": part.volume.as_ref().map_or_else(|| part.sim.time(), |volume| volume.time()),
                    "max_abs": part.sim.heights().iter().fold(0.0_f32, |a, h| a.max(h.abs())),
                    "volume": part.sim.total_volume(), "wet_cells": part.sim.wet_cells(),
                    "solver": if part.volume.is_some() { "3d_pbf_reference" } else { "shallow_water" },
                    "particles": part.volume.as_ref().map_or(0, |volume| volume.particles().len()),
                    "particle_volume_m3": part.volume.as_ref().map_or(0.0, |volume| volume.particle_volume()),
                    "particle_bounds": part.volume.as_ref().and_then(|volume| volume.particle_bounds()),
                })
            })
            .collect();
        return Ok(serde_json::json!({"parts": parts}));
    }
    let splash = match params.get("splash") {
        None => true,
        Some(value) => value.as_bool().ok_or("splash must be true or false")?,
    };
    let number = |key: &str, default: Option<f64>| -> Result<f32, String> {
        let n = match params.get(key) {
            Some(v) => v.as_f64(),
            None => default,
        }
        .filter(|n| n.is_finite() && n.abs() <= f64::from(f32::MAX))
        .ok_or_else(|| format!("{key} must be a finite number"))?;
        Ok(n as f32)
    };
    let (x, z) = (number("x", None)?, number("z", None)?);
    let radius = number("radius", Some(0.5))?;
    let amount = number("amount", Some(0.04))?;
    if radius <= 0.0 {
        return Err("radius must be positive".into());
    }
    let selected = match params.get("part") {
        None => None,
        Some(v) => Some(v.as_str().ok_or("part must be a name")?),
    };
    let time = world.resource::<AnimationClock>().seconds;
    let mut state = world.resource_mut::<Simulations>();
    let mut count = 0;
    let mut affected_particles = 0;
    let mut height = 0.0;
    let mut simulation_height = 0.0;
    let mut wave_height = 0.0;
    for (name, part) in &mut state.parts {
        let local = [x - part.origin[0], z - part.origin[1]];
        if selected.is_some_and(|s| s != name)
            || local[0] < 0.0
            || local[1] < 0.0
            || local[0] >= part.size[0]
            || local[1] >= part.size[1]
        {
            continue;
        }
        if !part.sim.is_water(local[0], local[1]) {
            continue;
        }
        if method == "water.disturb" {
            if let Some(volume) = part.volume.as_mut() {
                let particles =
                    volume.impulse_column([x, part.level, z], radius, [0.0, amount * 8.0, 0.0]);
                affected_particles += particles;
                count += usize::from(particles > 0);
            }
            part.sim.disturb(local[0], local[1], radius, amount);
            part.version += 1;
            if splash && amount > 0.03 {
                crate::fx_bursts::request(
                    "splash",
                    [f64::from(x), f64::from(part.level), f64::from(z)],
                    1.0,
                );
            }
        }
        let waves = ve_render::water_waves::WaveField::new(
            part.look.seed,
            part.look.ripple,
            part.look.flow,
            part.look.direction,
        );
        simulation_height = part.sim.height(local[0], local[1]);
        wave_height = waves.height(x, z, time);
        height = simulation_height + wave_height;
        if part.volume.is_none() {
            count += 1;
        }
    }
    Ok(if method == "water.height" {
        serde_json::json!({
            "height": height,
            "simulation": simulation_height,
            "wave": wave_height
        })
    } else {
        serde_json::json!({ "disturbed": count, "particles": affected_particles })
    })
}
