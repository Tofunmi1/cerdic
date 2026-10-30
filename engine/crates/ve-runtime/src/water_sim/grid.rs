//! Rasterize the existing sheet's wet area and depth across render chunks.
use super::*;
use bevy_asset::Handle;
use bevy_math::Vec3;
use bevy_mesh::VertexAttributeValues;

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "bounded grid (at most 256 cells per side), finite mesh bounds"
)]
pub(super) fn build(
    meshes: &Assets<Mesh>,
    chunks: &[(Entity, Handle<Mesh>, SurfaceWater, GlobalTransform, bool)],
    signature: Vec<u64>,
    id: u64,
) -> Option<Part> {
    let look = chunks.first()?.2;
    let mut triangles = Vec::new();
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for (_, handle, _, transform, fallback_depth) in chunks {
        let mesh = meshes.get(handle)?;
        let VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)?
        else {
            return None;
        };
        let colours = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
            Some(VertexAttributeValues::Float32x4(c)) => Some(c),
            _ => None,
        };
        let vertices: Vec<_> = positions
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let p = transform.transform_point(Vec3::from_array(*p)).to_array();
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(p[axis]);
                    hi[axis] = hi[axis].max(p[axis]);
                }
                (
                    p,
                    if *fallback_depth {
                        look.sim_depth
                    } else {
                        colours
                            .and_then(|c| c.get(i))
                            .map_or(look.sim_depth, |c| c[2] * 6.0)
                    },
                )
            })
            .collect();
        let indices: Vec<usize> = mesh.indices().map_or_else(
            || (0..vertices.len()).collect(),
            |indices| indices.iter().collect(),
        );
        for tri in indices.chunks_exact(3) {
            triangles.push([vertices[tri[0]], vertices[tri[1]], vertices[tri[2]]]);
        }
    }
    let longer = (hi[0] - lo[0]).max(hi[2] - lo[2]);
    if !longer.is_finite() || longer <= 0.0 {
        return None;
    }
    let resolution = look.sim_resolution.clamp(32, 256) as f32;
    let cell = longer / resolution;
    // Division can round a longer side just above the authored resolution.
    let width = ((hi[0] - lo[0]) / cell).ceil().clamp(1.0, resolution) as usize;
    let depth = ((hi[2] - lo[2]) / cell).ceil().clamp(1.0, resolution) as usize;
    let mut depths = vec![0.0_f32; width * depth];
    // Rasterize each triangle only over its footprint; top-facing triangles exclude slab bottoms.
    for tri in triangles {
        let [a, b, c] = tri.map(|v| v.0);
        let area = (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]);
        if area <= 1e-10 {
            continue;
        }
        let min = [a[0].min(b[0]).min(c[0]), a[2].min(b[2]).min(c[2])];
        let max = [a[0].max(b[0]).max(c[0]), a[2].max(b[2]).max(c[2])];
        let x0 = ((min[0] - lo[0]) / cell).floor().max(0.0) as usize;
        let z0 = ((min[1] - lo[2]) / cell).floor().max(0.0) as usize;
        let x1 = (((max[0] - lo[0]) / cell).ceil() as usize).min(width);
        let z1 = (((max[1] - lo[2]) / cell).ceil() as usize).min(depth);
        for z in z0..z1 {
            for x in x0..x1 {
                let p = [
                    lo[0] + (x as f32 + 0.5) * cell,
                    lo[2] + (z as f32 + 0.5) * cell,
                ];
                let u = ((b[2] - p[1]) * (c[0] - p[0]) - (b[0] - p[0]) * (c[2] - p[1])) / area;
                let v = ((c[2] - p[1]) * (a[0] - p[0]) - (c[0] - p[0]) * (a[2] - p[1])) / area;
                let w = 1.0 - u - v;
                if u >= -1e-5 && v >= -1e-5 && w >= -1e-5 {
                    depths[z * width + x] = (u * tri[0].1 + v * tri[1].1 + w * tri[2].1).max(0.0);
                }
            }
        }
    }
    let sim = WaterSim::new(
        &depths,
        width,
        depth,
        cell,
        SimOptions {
            gravity: 9.81,
            damping: look.sim_damping,
            absorb_edges: true,
        },
    )
    .ok()?;
    let volume = if look.volumetric && look.sim {
        build_volume(&depths, width, depth, cell, [lo[0], lo[2]], hi[1], look)
    } else {
        None
    };
    Some(Part {
        signature,
        sim,
        look,
        origin: [lo[0], lo[2]],
        size: [width as f32 * cell, depth as f32 * cell],
        dims: [width as u32, depth as u32],
        level: hi[1],
        rain: Rain::new(look.seed),
        id,
        version: 0,
        steps: 0,
        volume,
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "water grids are capped at 256 cells per side and values are finite"
)]
fn build_volume(
    depths: &[f32],
    width: usize,
    depth: usize,
    cell: f32,
    origin: [f32; 2],
    level: f32,
    look: SurfaceWater,
) -> Option<ve_world::water_sim::volume::Volume> {
    use ve_world::water_sim::volume::{Options, Particle, Volume};
    let wet_volume = depths.iter().sum::<f32>() * cell * cell;
    if !wet_volume.is_finite() || wet_volume <= 0.0 {
        return None;
    }
    // Bound the CPU reference solver in a browser; broad lakes use a coarser particle spacing.
    let target = 384.0_f32;
    let spacing = (wet_volume / target).cbrt().max(cell * 1.25);
    let stride = (spacing / cell).ceil().max(1.0) as usize;
    let spacing = stride as f32 * cell;
    let max_depth = depths.iter().copied().fold(0.0_f32, f32::max);
    let mut particles = Vec::new();
    for z in (0..depth).step_by(stride) {
        for x in (0..width).step_by(stride) {
            let floor = depths[z * width + x];
            if floor < spacing * 0.5 {
                continue;
            }
            let mut y = level - floor + spacing * 0.5;
            while y < level {
                if particles.len() == ve_world::water_sim::volume::MAX_PARTICLES {
                    return None;
                }
                particles.push(Particle {
                    position: [
                        origin[0] + (x as f32 + 0.5) * cell,
                        y,
                        origin[1] + (z as f32 + 0.5) * cell,
                    ],
                    velocity: [0.0; 3],
                });
                y += spacing;
            }
        }
    }
    if particles.is_empty() {
        return None;
    }
    let smoothing_radius = spacing * 2.0;
    let mut volume = Volume::new(
        particles,
        Options {
            min: [origin[0], level - max_depth, origin[1]],
            max: [
                origin[0] + width as f32 * cell,
                level + smoothing_radius,
                origin[1] + depth as f32 * cell,
            ],
            smoothing_radius,
            rest_density: 1000.0,
            particle_mass: 1000.0 * spacing.powi(3),
            gravity: [0.0, -9.81, 0.0],
            damping: look.sim_damping,
            iterations: 3,
        },
    )
    .ok()?;
    let particle_radius = smoothing_radius * 0.25;
    let bed: Vec<_> = depths
        .iter()
        .map(|depth| level - depth.max(particle_radius))
        .collect();
    volume
        .set_floor_grid(origin, cell, width, depth, bed)
        .then_some(volume)
}
