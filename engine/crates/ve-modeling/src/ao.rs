//! Deterministic, scene-wide ambient visibility bake for render vertices.
mod bvh;

use crate::{Fail, Part, Quality, fail};
use bvh::Bvh;

const CACHE_LIMIT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct Cache {
    signature: Option<String>,
    channels: Vec<Vec<f32>>,
}

impl Cache {
    fn replace(&mut self, signature: String, parts: &[Part]) {
        let bytes = parts
            .iter()
            .map(|part| part.occlusion.len() * size_of::<f32>())
            .sum::<usize>();
        if bytes <= CACHE_LIMIT_BYTES {
            self.signature = Some(signature);
            self.channels = parts.iter().map(|part| part.occlusion.clone()).collect();
        } else {
            *self = Self::default();
        }
    }
}

/// Bakes visibility against every output mesh. Placement sets share a prototype bake by design.
pub(crate) fn bake(parts: &mut [Part], quality: Quality, cache: &mut Cache) -> Result<bool, Fail> {
    let signature = signature(parts, quality);
    if cache.signature.as_ref() == Some(&signature) && cache.channels.len() == parts.len() {
        for (part, channel) in parts.iter_mut().zip(&cache.channels) {
            part.occlusion.clone_from(channel);
        }
        return Ok(true);
    }
    let triangle_count = parts
        .iter()
        .map(|part| part.indices.len() / 3)
        .sum::<usize>();
    if triangle_count > 1_000_000 {
        return Err(fail(
            "budget.ao_triangles",
            format!("ambient occlusion is limited to 1,000,000 triangles; got {triangle_count}"),
        ));
    }
    // A lone planar sheet has no geometry above its surface to occlude its
    // cosine hemisphere. Avoid building a large BVH for common floors/walls.
    if parts.len() == 1 && is_planar(&parts[0].positions) {
        parts[0].occlusion = vec![1.0; parts[0].positions.len()];
        cache.replace(signature, parts);
        return Ok(false);
    }
    let bvh = Bvh::from_parts(parts);
    let rays = match quality {
        Quality::Preview => 8,
        Quality::Final => 32,
    };
    for part in parts.iter_mut() {
        part.occlusion = bvh.bake_vertices(&part.positions, &part.indices, rays);
    }
    cache.replace(signature, parts);
    Ok(false)
}

fn signature(parts: &[Part], quality: Quality) -> String {
    let (mut first, mut second) = (0xcbf2_9ce4_8422_2325_u64, 0x8422_2325_cbf2_9ce4_u64);
    let mut write = |value: u32| {
        for byte in value.to_le_bytes() {
            first = (first ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            second = (second ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    };
    write(u32::try_from(parts.len()).unwrap_or(u32::MAX));
    write(match quality {
        Quality::Preview => 8,
        Quality::Final => 32,
    });
    for part in parts {
        write(u32::try_from(part.positions.len()).unwrap_or(u32::MAX));
        write(u32::try_from(part.indices.len()).unwrap_or(u32::MAX));
        for position in &part.positions {
            for coordinate in position {
                write(coordinate.to_bits());
            }
        }
        for index in &part.indices {
            write(*index);
        }
    }
    format!("{first:016x}{second:016x}")
}

fn is_planar(positions: &[[f32; 3]]) -> bool {
    let Some(first) = positions.first() else {
        return false;
    };
    (0..3).any(|axis| {
        positions
            .iter()
            .all(|point| (point[axis] - first[axis]).abs() <= 1.0e-6)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(y: f32, half: f32) -> Part {
        let positions = vec![
            [-half, y, -half],
            [half, y, -half],
            [half, y, half],
            [-half, y, half],
            [0.0, y, 0.0],
        ];
        Part {
            node: ve_graph::NodeId::parse("n_01234567").expect("fixture id"),
            name: "panel".to_owned(),
            positions,
            indices: vec![0, 4, 1, 1, 4, 2, 2, 4, 3, 3, 4, 0],
            uvs: Vec::new(),
            occlusion: Vec::new(),
            volume: 0.0,
            material: None,
            frames: std::collections::BTreeMap::new(),
            animation: Vec::new(),
            behavior: None,
            parent: None,
            bone: None,
            skin: None,
            ik: None,
            body: None,
            transforms: None,
            instance_indices: Vec::new(),
            region: 0,
        }
    }

    #[test]
    fn nearby_ceiling_occludes_the_floor_and_open_floor_stays_visible() {
        let floor = panel(0.0, 0.1);
        let open = bvh::Bvh::from_parts(std::slice::from_ref(&floor));
        let open_visibility = open.bake_vertices(&floor.positions, &floor.indices, 16);
        assert!(open_visibility[4] > 0.99);

        let mut scene = vec![floor, panel(0.5, 1.0)];
        bake(&mut scene, Quality::Final, &mut Cache::default())
            .expect("small scene fits AO budget");
        assert!(
            scene[0].occlusion[4] < 0.3,
            "center visibility: {}",
            scene[0].occlusion[4]
        );
    }

    #[test]
    fn scene_budget_refuses_before_building_the_acceleration_structure() {
        let mut scene = vec![panel(0.0, 1.0)];
        scene[0].indices = vec![0; 3_000_003];
        let error = bake(&mut scene, Quality::Preview, &mut Cache::default())
            .expect_err("AO cap is enforced");
        assert_eq!(error.code, "budget.ao_triangles");
    }

    #[test]
    fn isolated_planar_sheet_receives_full_visibility_without_changing_geometry() {
        let mut scene = vec![panel(0.0, 1.0)];
        let indices = scene[0].indices.clone();
        bake(&mut scene, Quality::Final, &mut Cache::default())
            .expect("single sheet is within the AO budget");
        assert_eq!(scene[0].occlusion, vec![1.0; scene[0].positions.len()]);
        assert_eq!(scene[0].indices, indices);
    }

    #[test]
    fn unchanged_scene_reuses_occlusion_but_geometry_and_quality_changes_rebake() {
        let mut cache = Cache::default();
        let mut scene = vec![panel(0.0, 0.1), panel(0.5, 1.0)];
        assert!(!bake(&mut scene, Quality::Final, &mut cache).expect("first bake succeeds"));
        let first = scene[0].occlusion.clone();
        assert!(bake(&mut scene, Quality::Final, &mut cache).expect("cache hit succeeds"));
        assert_eq!(scene[0].occlusion, first);
        assert!(!bake(&mut scene, Quality::Preview, &mut cache).expect("quality change rebakes"));
        scene[1].positions[4][1] = 0.75;
        assert!(!bake(&mut scene, Quality::Preview, &mut cache).expect("geometry change rebakes"));
    }

    #[test]
    #[ignore = "release-only G9 AO timing and sampled RSS study"]
    fn b2_measure_100k_and_1m_triangle_scenes() {
        for side in [224, 707] {
            let part = curved_grid(side);
            let triangle_count = part.indices.len() / 3;
            let before = rss_kib();
            let job = std::thread::spawn(move || {
                let mut scene = vec![part];
                let mut cache = Cache::default();
                let started = std::time::Instant::now();
                bake(&mut scene, Quality::Final, &mut cache)
                    .expect("benchmark mesh fits the AO budget");
                let elapsed = started.elapsed();
                let cache_started = std::time::Instant::now();
                assert!(bake(&mut scene, Quality::Final, &mut cache).expect("cache hit succeeds"));
                (elapsed, cache_started.elapsed(), scene)
            });
            let mut peak_rss = before;
            while !job.is_finished() {
                peak_rss = peak_rss.max(rss_kib());
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            let (elapsed, cache_elapsed, scene) = job.join().expect("AO worker thread completes");
            peak_rss = peak_rss.max(rss_kib());
            assert_eq!(scene[0].occlusion.len(), scene[0].positions.len());
            println!(
                "G9b AO: {triangle_count} triangles, Final/32 rays, {:.2}s bake, {:.3}s cache hit, {} MiB sampled RSS (delta {} MiB)",
                elapsed.as_secs_f64(),
                cache_elapsed.as_secs_f64(),
                peak_rss / 1024,
                peak_rss.saturating_sub(before) / 1024,
            );
        }
    }

    fn curved_grid(side: u32) -> Part {
        let span = side + 1;
        let mut part = panel(0.0, 1.0);
        part.positions.clear();
        part.indices.clear();
        for z in 0..span {
            for x in 0..span {
                let px = 2.0 * f32::from(u16::try_from(x).expect("grid coordinate fits"))
                    / f32::from(u16::try_from(side).expect("grid width fits"))
                    - 1.0;
                let pz = 2.0 * f32::from(u16::try_from(z).expect("grid coordinate fits"))
                    / f32::from(u16::try_from(side).expect("grid width fits"))
                    - 1.0;
                part.positions
                    .push([px, 0.2 * (px * 3.0).sin() * (pz * 3.0).sin(), pz]);
            }
        }
        for z in 0..side {
            for x in 0..side {
                let a = z * span + x;
                let b = a + 1;
                let c = a + span;
                let d = c + 1;
                part.indices.extend([c, a, b, b, d, c]);
            }
        }
        part
    }

    fn rss_kib() -> usize {
        let pid = std::process::id().to_string();
        let output = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid])
            .output()
            .expect("ps is available for the ignored native benchmark");
        String::from_utf8(output.stdout)
            .expect("ps RSS is UTF-8")
            .trim()
            .parse()
            .expect("ps reports resident KiB")
    }
}
