//! Release timing fixture for G7-15. Run natively and as wasm32-wasip1.
#![expect(
    clippy::print_stdout,
    reason = "benchmark reports measurements to the terminal"
)]

use std::time::Instant;
use ve_kernel::{
    ExecutionContext, SubdivisionChannels, subdivide_catmark, subdivide_catmark_with_channels,
};

#[cfg(target_arch = "wasm32")]
fn memory_bytes() -> usize {
    core::arch::wasm32::memory_size(0) * 65_536
}

#[cfg(not(target_arch = "wasm32"))]
fn memory_bytes() -> usize {
    0
}

fn grid(columns: u32, rows: u32) -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>) {
    let mut positions = Vec::with_capacity(((columns + 1) * (rows + 1)) as usize);
    for y in 0..=rows {
        for x in 0..=columns {
            #[expect(
                clippy::cast_precision_loss,
                reason = "benchmark grid dimensions stay below 317 and fit f32 exactly"
            )]
            positions.push([x as f32, 0.0, y as f32]);
        }
    }
    let mut corners = Vec::with_capacity((columns * rows * 4) as usize);
    for y in 0..rows {
        for x in 0..columns {
            let a = y * (columns + 1) + x;
            corners.extend([a, a + 1, a + columns + 2, a + columns + 1]);
        }
    }
    (positions, vec![4; (columns * rows) as usize], corners)
}

fn timed_case(
    name: &str,
    positions: &[[f32; 3]],
    sizes: &[u32],
    corners: &[u32],
    levels: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = ExecutionContext::new()?;
    let start = Instant::now();
    let result = subdivide_catmark(positions, sizes, corners, levels, 1, 0, &context)?;
    println!(
        "{name}: input_faces={} levels={levels} output_vertices={} output_faces={} elapsed_ms={} wasm_memory_bytes={}",
        sizes.len(),
        result.positions.len(),
        result.face_sizes.len(),
        start.elapsed().as_millis(),
        memory_bytes()
    );
    Ok(())
}

fn timed_channel_case(
    mode: &str,
    positions: &[[f32; 3]],
    sizes: &[u32],
    corners: &[u32],
) -> Result<(), Box<dyn std::error::Error>> {
    let channels = match mode {
        "channels1" => 1,
        "channels8" => 8,
        _ => 0,
    };
    let point_values = vec![0.5; positions.len() * channels];
    let fvar_values = if mode == "uv" {
        positions
            .iter()
            .map(|point| [point[0], point[2]])
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let fvar_corner_values = if mode == "uv" {
        corners.to_vec()
    } else {
        Vec::new()
    };
    let context = ExecutionContext::new()?;
    let start = Instant::now();
    let result = subdivide_catmark_with_channels(
        positions,
        sizes,
        corners,
        &[],
        &[],
        SubdivisionChannels {
            point_channels: channels,
            point_values: &point_values,
            fvar_values: &fvar_values,
            fvar_corner_values: &fvar_corner_values,
        },
        4,
        1,
        0,
        &context,
    )?;
    println!(
        "mode={mode}: input_faces={} output_vertices={} output_faces={} output_point_floats={} output_uv_values={} elapsed_ms={} wasm_memory_bytes={}",
        sizes.len(),
        result.positions.len(),
        result.face_sizes.len(),
        result.point_values.len(),
        result.fvar_values.len(),
        start.elapsed().as_millis(),
        memory_bytes()
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(mode) = std::env::args().nth(1) {
        let (positions, sizes, corners) = grid(88, 88);
        return timed_channel_case(&mode, &positions, &sizes, &corners);
    }
    let (positions, sizes, corners) = grid(316, 316);
    timed_case("grid_99856_level1", &positions, &sizes, &corners, 1)?;
    let (positions, sizes, corners) = grid(2, 2);
    timed_case("grid_4_level4", &positions, &sizes, &corners, 4)?;
    // The largest cases the face and vertex caps accept: about two million output faces.
    let (positions, sizes, corners) = grid(88, 88);
    timed_case("grid_7744_level4_near_cap", &positions, &sizes, &corners, 4)?;
    let (positions, sizes, corners) = grid(176, 176);
    timed_case(
        "grid_30976_level3_near_cap",
        &positions,
        &sizes,
        &corners,
        3,
    )
}
