//! Tile-aware heightfield grids, generators, masks and mesh output (ADR 0056). Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::many_single_char_names,
    reason = "fixtures unwrap valid data, build grids from small indices, and compare exact copies of the same computation"
)]
use ve_world::terrain::{
    Combine, HeightGrid, Mask, MeshOut, Noise, StampShape, TerrainError, carve_path, combine,
    compute_mask, domain_warp, edge_falloff, generate, pack_splat, remap, smooth, stamp, terrace,
    to_height_texture, to_mesh,
};

const NEVER: &dyn Fn() -> bool = &|| false;

fn noise(seed: u32) -> Noise {
    Noise {
        seed,
        scale: 60.0,
        octaves: 5,
        roughness: 0.5,
        lacunarity: 2.0,
        ridged: false,
        warp: 0.0,
        basis: None,
    }
}

fn generated(width: usize, depth: usize, origin: [f32; 2], seed: u32) -> HeightGrid {
    let mut grid = HeightGrid::new(width, depth, 2.0, origin).unwrap();
    generate(&mut grid, &noise(seed), 120.0, NEVER).unwrap();
    grid
}

fn plane(width: usize, depth: usize, slope_x: f32, slope_z: f32) -> HeightGrid {
    let mut grid = HeightGrid::new(width, depth, 1.0, [0.0, 0.0]).unwrap();
    for j in 0..depth {
        for i in 0..width {
            grid.heights_mut()[j * width + i] = 100.0 + slope_x * i as f32 + slope_z * j as f32;
        }
    }
    grid
}

// ---------- the grid (implemented by the owner-side session) ----------

#[test]
fn a_grid_is_positioned_in_the_world_and_samples_bilinearly() {
    let mut grid = HeightGrid::new(4, 4, 2.0, [10.0, -4.0]).unwrap();
    grid.heights_mut()[0] = 4.0; // cell (0,0) at world (10, -4)
    assert!((grid.sample(10.0, -4.0) - 4.0).abs() < 1e-6);
    assert!(
        (grid.sample(11.0, -4.0) - 2.0).abs() < 1e-6,
        "halfway to the next cell"
    );
    assert!(
        (grid.sample(-100.0, -100.0) - 4.0).abs() < 1e-6,
        "clamps at the edge"
    );
    assert_eq!(grid.origin(), [10.0, -4.0]);
}

#[test]
fn the_normal_of_a_plane_leans_against_its_slope() {
    let grid = plane(16, 16, 0.5, 0.0);
    let n = grid.normal(8.0, 8.0);
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    assert!((length - 1.0).abs() < 1e-5);
    assert!(n[0] < -0.4 && n[1] > 0.8 && n[2].abs() < 1e-5, "{n:?}");
}

#[test]
fn layers_check_their_length_and_list_sorted() {
    let mut grid = HeightGrid::new(8, 8, 1.0, [0.0, 0.0]).unwrap();
    grid.set_layer("wet", vec![0.0; 64]).unwrap();
    grid.set_layer("rock", vec![1.0; 64]).unwrap();
    assert_eq!(
        grid.layer_names(),
        vec!["rock".to_string(), "wet".to_string()]
    );
    assert!(matches!(
        grid.set_layer("bad", vec![0.0; 10]),
        Err(TerrainError::BadShape { .. })
    ));
    assert!(matches!(
        grid.set_layer("", vec![0.0; 64]),
        Err(TerrainError::BadLayer { .. })
    ));
    assert!(grid.layer("rock").is_some() && grid.layer("none").is_none());
}

#[test]
fn a_tile_over_the_limit_is_refused_with_the_fix() {
    let error = HeightGrid::new(2049, 16, 1.0, [0.0, 0.0]).unwrap_err();
    assert!(matches!(
        error,
        TerrainError::TooLarge {
            side: 2049,
            max: 2048
        }
    ));
    assert!(error.to_string().contains("smaller tiles"));
}

#[test]
fn sub_grid_and_paste_move_heights_and_layers_by_world_position() {
    let mut grid = HeightGrid::new(16, 16, 1.0, [100.0, 50.0]).unwrap();
    for (k, h) in grid.heights_mut().iter_mut().enumerate() {
        *h = k as f32;
    }
    grid.set_layer("m", (0..256).map(|k| k as f32 * 2.0).collect())
        .unwrap();
    let window = grid.sub_grid(4, 3, 6, 5).unwrap();
    assert_eq!(window.origin(), [104.0, 53.0]);
    assert!((window.heights()[0] - (3.0 * 16.0 + 4.0)).abs() < 1e-6);
    assert!((window.layer("m").unwrap()[0] - 2.0 * (3.0 * 16.0 + 4.0)).abs() < 1e-6);
    let mut blank = HeightGrid::new(16, 16, 1.0, [100.0, 50.0]).unwrap();
    blank.paste(&window).unwrap();
    assert!((blank.heights()[3 * 16 + 4] - grid.heights()[3 * 16 + 4]).abs() < 1e-6);
    assert!(blank.heights()[0].abs() < 1e-6, "outside the window stays");
    assert!(grid.sub_grid(12, 12, 8, 8).is_err());
}

// ---------- generators ----------

#[test]
fn generate_fills_the_range_and_is_deterministic() {
    let a = generated(96, 96, [0.0, 0.0], 7);
    let b = generated(96, 96, [0.0, 0.0], 7);
    let c = generated(96, 96, [0.0, 0.0], 8);
    assert_eq!(a.heights(), b.heights());
    assert_ne!(a.heights(), c.heights());
    let (lo, hi) = a
        .heights()
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    assert!(
        lo >= 0.0 && hi <= 120.0 + 1e-3 && hi - lo > 20.0,
        "{lo} {hi}"
    );
    assert!(a.heights().iter().all(|v| v.is_finite()));
}

#[test]
fn adjacent_tiles_agree_on_the_shared_edge() {
    // 65 cells of 2 m: the right column of tile A is at world x = 128, the left column of tile B too.
    let a = generated(65, 65, [0.0, 0.0], 3);
    let b = generated(65, 65, [128.0, 0.0], 3);
    for j in 0..65 {
        assert_eq!(a.heights()[j * 65 + 64], b.heights()[j * 65], "row {j}");
    }
}

#[test]
fn a_window_cut_from_a_big_grid_equals_the_same_window_generated_alone() {
    let big = generated(129, 129, [-40.0, 20.0], 11);
    let cut = big.sub_grid(30, 50, 40, 40).unwrap();
    let alone = generated(40, 40, cut.origin(), 11);
    assert_eq!(cut.heights(), alone.heights());
}

#[test]
fn generate_can_be_cancelled_and_refuses_bad_noise() {
    let mut grid = HeightGrid::new(64, 64, 1.0, [0.0, 0.0]).unwrap();
    assert!(matches!(
        generate(&mut grid, &noise(1), 10.0, &|| true),
        Err(TerrainError::Cancelled)
    ));
    let mut bad = noise(1);
    bad.octaves = 0;
    assert!(matches!(
        generate(&mut grid, &bad, 10.0, NEVER),
        Err(TerrainError::BadParameter { .. })
    ));
}

#[test]
fn ridged_noise_differs_from_plain_noise() {
    let mut a = HeightGrid::new(64, 64, 2.0, [0.0, 0.0]).unwrap();
    let mut b = a.clone();
    generate(&mut a, &noise(5), 100.0, NEVER).unwrap();
    let mut ridged = noise(5);
    ridged.ridged = true;
    generate(&mut b, &ridged, 100.0, NEVER).unwrap();
    assert_ne!(a.heights(), b.heights());
}

#[test]
fn terrace_leaves_only_the_requested_levels() {
    let mut grid = generated(64, 64, [0.0, 0.0], 2);
    terrace(&mut grid, 4, 1.0);
    let mut levels: Vec<i64> = grid
        .heights()
        .iter()
        .map(|h| (h * 100.0).round() as i64)
        .collect();
    levels.sort_unstable();
    levels.dedup();
    assert!(levels.len() <= 5, "{} distinct levels", levels.len());
    let mut untouched = generated(64, 64, [0.0, 0.0], 2);
    let before = untouched.clone();
    terrace(&mut untouched, 4, 0.0);
    assert_eq!(
        untouched.heights(),
        before.heights(),
        "sharpness 0 changes nothing"
    );
}

#[test]
fn smoothing_lowers_roughness_and_keeps_the_mean() {
    let mut grid = generated(64, 64, [0.0, 0.0], 9);
    let roughness = |g: &HeightGrid| -> f32 {
        let w = g.width();
        let mut total = 0.0;
        for j in 0..g.depth() {
            for i in 1..w {
                total += (g.heights()[j * w + i] - g.heights()[j * w + i - 1]).abs();
            }
        }
        total
    };
    let mean = |g: &HeightGrid| g.heights().iter().sum::<f32>() / g.heights().len() as f32;
    let (r0, m0) = (roughness(&grid), mean(&grid));
    smooth(&mut grid, 3);
    assert!(roughness(&grid) < r0 * 0.8);
    assert!((mean(&grid) - m0).abs() < 2.0);
}

#[test]
fn edge_falloff_makes_an_island_in_world_coordinates() {
    let mut grid = HeightGrid::new(65, 65, 2.0, [-64.0, -64.0]).unwrap();
    grid.heights_mut().fill(50.0);
    edge_falloff(&mut grid, [0.0, 0.0], 60.0, 0.5);
    assert!(
        (grid.sample(0.0, 0.0) - 50.0).abs() < 1e-3,
        "inside radius*(1-reach)"
    );
    assert!(grid.sample(62.0, 0.0).abs() < 1e-3, "beyond the radius");
    let mid = grid.sample(45.0, 0.0);
    assert!(mid > 1.0 && mid < 49.0, "falls off between: {mid}");
}

#[test]
fn a_carved_path_lowers_its_channel_and_nothing_beyond() {
    let mut grid = HeightGrid::new(65, 65, 1.0, [0.0, 0.0]).unwrap();
    grid.heights_mut().fill(20.0);
    carve_path(&mut grid, &[[0.0, 32.0], [64.0, 32.0]], 8.0, 5.0, false);
    assert!(
        (grid.sample(32.0, 32.0) - 15.0).abs() < 0.3,
        "centre lowered by depth: {}",
        grid.sample(32.0, 32.0)
    );
    assert!(
        (grid.sample(32.0, 40.0) - 20.0).abs() < 1e-3,
        "beyond the width is untouched"
    );
}

#[test]
fn carving_two_tiles_and_pasting_equals_carving_the_joined_grid() {
    let base = generated(129, 65, [0.0, 0.0], 4);
    let path = [[10.0, 30.0], [100.0, 70.0], [250.0, 40.0]];
    let mut joined = base.clone();
    carve_path(&mut joined, &path, 10.0, 4.0, false);
    let mut left = base.sub_grid(0, 0, 65, 65).unwrap();
    let mut right = base.sub_grid(64, 0, 65, 65).unwrap();
    carve_path(&mut left, &path, 10.0, 4.0, false);
    carve_path(&mut right, &path, 10.0, 4.0, false);
    let mut stitched = base.clone();
    stitched.paste(&left).unwrap();
    stitched.paste(&right).unwrap();
    for (a, b) in joined.heights().iter().zip(stitched.heights()) {
        assert!((a - b).abs() < 1e-4);
    }
}

#[test]
fn a_stamp_adds_its_height_at_the_centre() {
    let mut grid = HeightGrid::new(65, 65, 1.0, [-32.0, -32.0]).unwrap();
    stamp(&mut grid, [0.0, 0.0], 20.0, 30.0, StampShape::Smooth);
    assert!((grid.sample(0.0, 0.0) - 30.0).abs() < 0.5);
    assert!(grid.sample(25.0, 0.0).abs() < 1e-3);
    let mut crater = HeightGrid::new(65, 65, 1.0, [-32.0, -32.0]).unwrap();
    stamp(&mut crater, [0.0, 0.0], 20.0, 30.0, StampShape::Crater);
    assert!(
        crater.sample(0.0, 0.0) < crater.sample(15.0, 0.0),
        "bowl lower than rim"
    );
}

#[test]
fn domain_warp_changes_heights_but_keeps_them_finite_and_in_range() {
    let mut grid = generated(64, 64, [0.0, 0.0], 6);
    let before = grid.clone();
    domain_warp(&mut grid, 3, 40.0, 12.0);
    assert_ne!(grid.heights(), before.heights());
    let hi = before.heights().iter().fold(f32::MIN, |m, v| m.max(*v));
    assert!(
        grid.heights()
            .iter()
            .all(|v| v.is_finite() && *v >= -1e-3 && *v <= hi + 1e-3)
    );
}

// ---------- masks ----------

#[test]
fn slope_is_zero_on_flat_ground_and_half_at_forty_five_degrees() {
    let flat = plane(32, 32, 0.0, 0.0);
    let m = compute_mask(&flat, Mask::Slope, NEVER).unwrap();
    assert!(m.iter().all(|v| v.abs() < 1e-4));
    let ramp = plane(32, 32, 1.0, 0.0);
    let m = compute_mask(&ramp, Mask::Slope, NEVER).unwrap();
    assert!((m[16 * 32 + 16] - 0.5).abs() < 0.03, "{}", m[16 * 32 + 16]);
    let steep = plane(32, 32, 20.0, 0.0);
    assert!(compute_mask(&steep, Mask::Slope, NEVER).unwrap()[16 * 32 + 16] > 0.9);
}

#[test]
fn every_mask_stays_in_zero_to_one() {
    let grid = generated(64, 64, [0.0, 0.0], 12);
    for mask in [
        Mask::Slope,
        Mask::Curvature,
        Mask::Height,
        Mask::Aspect,
        Mask::Occlusion,
    ] {
        let m = compute_mask(&grid, mask, NEVER).unwrap();
        assert_eq!(m.len(), 64 * 64);
        assert!(m.iter().all(|v| (0.0..=1.0).contains(v)), "{mask:?}");
    }
}

#[test]
fn height_mask_spans_the_range_and_curvature_is_half_on_flat() {
    let grid = generated(64, 64, [0.0, 0.0], 13);
    let m = compute_mask(&grid, Mask::Height, NEVER).unwrap();
    let (lo, hi) = m
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    assert!(lo < 1e-4 && hi > 1.0 - 1e-4);
    let flat = plane(16, 16, 0.0, 0.0);
    assert!(
        compute_mask(&flat, Mask::Curvature, NEVER)
            .unwrap()
            .iter()
            .all(|v| (v - 0.5).abs() < 1e-4)
    );
}

#[test]
fn aspect_points_downhill_with_north_at_positive_z() {
    // The ground falls toward +z, so the slope faces north: 0.0 (or equivalently 1.0 wrapped).
    let north = compute_mask(&plane(16, 16, 0.0, -1.0), Mask::Aspect, NEVER).unwrap()[8 * 16 + 8];
    assert!(!(0.02..=0.98).contains(&north), "north {north}");
    let east = compute_mask(&plane(16, 16, -1.0, 0.0), Mask::Aspect, NEVER).unwrap()[8 * 16 + 8];
    assert!((east - 0.25).abs() < 0.02, "east {east}");
    let south = compute_mask(&plane(16, 16, 0.0, 1.0), Mask::Aspect, NEVER).unwrap()[8 * 16 + 8];
    assert!((south - 0.5).abs() < 0.02, "south {south}");
    let west = compute_mask(&plane(16, 16, 1.0, 0.0), Mask::Aspect, NEVER).unwrap()[8 * 16 + 8];
    assert!((west - 0.75).abs() < 0.02, "west {west}");
}

#[test]
fn occlusion_is_zero_on_flat_ground_and_high_in_a_pit() {
    let flat = plane(64, 64, 0.0, 0.0);
    assert!(
        compute_mask(&flat, Mask::Occlusion, NEVER)
            .unwrap()
            .iter()
            .all(|v| *v < 1e-4)
    );
    let mut pit = plane(64, 64, 0.0, 0.0);
    for j in 29..35 {
        for i in 29..35 {
            pit.heights_mut()[j * 64 + i] = 90.0;
        }
    }
    assert!(compute_mask(&pit, Mask::Occlusion, NEVER).unwrap()[32 * 64 + 32] > 0.5);
}

#[test]
fn masks_computed_on_a_haloed_window_equal_the_whole_grid_inside() {
    let grid = generated(160, 160, [0.0, 0.0], 14);
    for (mask, halo) in [
        (Mask::Slope, 1usize),
        (Mask::Curvature, 1),
        (Mask::Aspect, 1),
        (Mask::Occlusion, 32),
    ] {
        let whole = compute_mask(&grid, mask, NEVER).unwrap();
        let window = grid
            .sub_grid(60 - halo, 60 - halo, 40 + 2 * halo, 40 + 2 * halo)
            .unwrap();
        let part = compute_mask(&window, mask, NEVER).unwrap();
        let side = 40 + 2 * halo;
        for j in 0..40 {
            for i in 0..40 {
                let a = whole[(60 + j) * 160 + 60 + i];
                let b = part[(j + halo) * side + i + halo];
                assert_eq!(a, b, "{mask:?} at {i},{j}");
            }
        }
    }
}

#[test]
fn masks_can_be_cancelled() {
    let grid = generated(64, 64, [0.0, 0.0], 15);
    assert!(matches!(
        compute_mask(&grid, Mask::Slope, &|| true),
        Err(TerrainError::Cancelled)
    ));
}

#[test]
fn remap_and_combine_work_on_plain_slices() {
    let mut v = vec![0.0, 0.5, 1.0, 2.0, -1.0];
    remap(&mut v, 0.0, 1.0, false);
    assert_eq!(v, vec![0.0, 0.5, 1.0, 1.0, 0.0]);
    let mut s = vec![0.5];
    remap(&mut s, 0.0, 1.0, true);
    assert!((s[0] - 0.5).abs() < 1e-6, "smoothstep(0.5) = 0.5");
    let (a, b) = ([0.2, 0.8, 1.0], [0.5, 0.5, 0.5]);
    assert_eq!(combine(&a, &b, Combine::Multiply), vec![0.1, 0.4, 0.5]);
    assert_eq!(combine(&a, &b, Combine::Max), vec![0.5, 0.8, 1.0]);
    assert_eq!(combine(&a, &b, Combine::Min), vec![0.2, 0.5, 0.5]);
    assert_eq!(combine(&a, &b, Combine::Add), vec![0.7, 1.0, 1.0]);
    assert_eq!(combine(&a, &b, Combine::Subtract), vec![0.0, 0.3, 0.5]);
}

// ---------- output ----------

fn triangle_normal_y(mesh: &MeshOut, t: usize) -> f32 {
    let p = |k: usize| mesh.positions[mesh.indices[t * 3 + k] as usize];
    let (a, b, c) = (p(0), p(1), p(2));
    (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2])
}

#[test]
fn the_mesh_has_one_vertex_per_cell_and_faces_up() {
    let grid = generated(33, 17, [5.0, 5.0], 16);
    let mesh = to_mesh(&grid, 1);
    assert_eq!(mesh.positions.len(), 33 * 17);
    assert_eq!(mesh.normals.len(), mesh.positions.len());
    assert_eq!(mesh.uvs.len(), mesh.positions.len());
    assert_eq!(mesh.indices.len(), 2 * 32 * 16 * 3);
    assert!(
        mesh.indices
            .iter()
            .all(|i| (*i as usize) < mesh.positions.len())
    );
    assert!(
        (0..mesh.indices.len() / 3).all(|t| triangle_normal_y(&mesh, t) > 0.0),
        "counter-clockwise from +y"
    );
    assert!((mesh.positions[0][0] - 5.0).abs() < 1e-5 && (mesh.positions[0][2] - 5.0).abs() < 1e-5);
    assert!(
        mesh.normals
            .iter()
            .all(|n| (n[0] * n[0] + n[1] * n[1] + n[2] * n[2] - 1.0).abs() < 1e-3 && n[1] > 0.0)
    );
}

#[test]
fn a_stride_makes_a_coarser_mesh_over_the_same_area() {
    let grid = generated(65, 65, [0.0, 0.0], 17);
    let coarse = to_mesh(&grid, 4);
    assert_eq!(coarse.positions.len(), 17 * 17);
    let max_x = coarse.positions.iter().fold(f32::MIN, |m, p| m.max(p[0]));
    assert!(
        (max_x - 128.0).abs() < 1e-3,
        "still reaches the far edge: {max_x}"
    );
}

#[test]
fn the_height_texture_quantises_over_the_given_range() {
    let mut grid = HeightGrid::new(4, 4, 1.0, [0.0, 0.0]).unwrap();
    grid.heights_mut()[0] = 10.0;
    grid.heights_mut()[1] = 110.0;
    grid.heights_mut()[2] = 60.0;
    let texture = to_height_texture(&grid, 10.0, 110.0);
    assert_eq!(texture.len(), 16);
    assert_eq!((texture[0], texture[1]), (0, 65535));
    assert!((i32::from(texture[2]) - 32768).abs() <= 1);
}

#[test]
fn splat_weights_sum_to_255_per_cell() {
    let (a, b, c) = (
        vec![1.0, 0.0, 0.3],
        vec![0.0, 0.0, 0.3],
        vec![0.0, 0.0, 0.3],
    );
    let packed = pack_splat(&[&a, &b, &c]);
    assert_eq!(packed.len(), 3 * 4);
    for cell in packed.chunks(4) {
        let sum: u32 = cell.iter().map(|v| u32::from(*v)).sum();
        assert!(sum == 255 || sum == 0, "{cell:?}");
    }
    assert_eq!(&packed[0..4], &[255, 0, 0, 0]);
}
