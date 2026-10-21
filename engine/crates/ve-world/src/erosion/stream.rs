//! Implicit stream-power erosion.
use std::collections::VecDeque;

use super::{ErosionError, Flow, Grid, route_flow};

/// `dh/dt = uplift - k * area^m * slope`, solved receiver-first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamPower {
    /// Erodibility, zero or more.
    pub k: f32,
    /// Area exponent: 0.5, 1.0, or 1.5.
    pub m: f32,
    /// Uplift in metres per unit time.
    pub uplift: f32,
    /// Time step.
    pub dt: f32,
    /// Number of iterations.
    pub iterations: u32,
}

/// Runs implicit stream-power erosion. Heights are clamped at zero.
///
/// # Errors
/// Returns shape, cancellation, or invalid parameter errors.
pub fn stream_power(
    grid: &mut Grid<'_>,
    uplift_map: Option<&[f32]>,
    params: &StreamPower,
    cancel: &dyn Fn() -> bool,
) -> Result<(), ErosionError> {
    grid.check()?;
    validate(grid, uplift_map, params)?;
    for _ in 0..params.iterations {
        let flow = route_flow(grid, cancel)?;
        let order = receiver_order(&flow.receiver, grid.heights.len());
        erode_in_order(grid, uplift_map, params, &flow, &order, cancel)?;
    }
    Ok(())
}

fn validate(
    grid: &Grid<'_>,
    uplift_map: Option<&[f32]>,
    params: &StreamPower,
) -> Result<(), ErosionError> {
    if ![0.5, 1.0, 1.5].contains(&params.m) {
        return Err(bad("m", "must be 0.5, 1.0, or 1.5"));
    }
    for (name, value) in [
        ("k", params.k),
        ("uplift", params.uplift),
        ("dt", params.dt),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(bad(name, "must be finite and non-negative"));
        }
    }
    let invalid_uplift_map = uplift_map.is_some_and(|map| {
        map.len() != grid.heights.len()
            || map
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    });
    if invalid_uplift_map {
        return Err(bad(
            "uplift_map",
            "must match the grid and contain values in 0..=1",
        ));
    }
    Ok(())
}

fn receiver_order(receiver: &[u32], count: usize) -> Vec<usize> {
    let mut children = vec![0_usize; count];
    for &downstream in receiver {
        if let Ok(downstream) = usize::try_from(downstream)
            && downstream < count
        {
            children[downstream] += 1;
        }
    }
    let mut offsets = vec![0_usize; count + 1];
    for index in 0..count {
        offsets[index + 1] = offsets[index] + children[index];
        children[index] = 0;
    }
    let mut upstream = vec![0_usize; count];
    for (index, &downstream) in receiver.iter().enumerate() {
        if let Ok(downstream) = usize::try_from(downstream)
            && downstream < count
        {
            upstream[offsets[downstream] + children[downstream]] = index;
            children[downstream] += 1;
        }
    }
    topological_order(receiver, &offsets, &upstream)
}

fn topological_order(receiver: &[u32], offsets: &[usize], upstream: &[usize]) -> Vec<usize> {
    let mut order = Vec::with_capacity(receiver.len());
    let mut queue = VecDeque::new();
    for (index, &downstream) in receiver.iter().enumerate() {
        if downstream == u32::MAX {
            queue.push_back(index);
        }
    }
    while let Some(index) = queue.pop_front() {
        order.push(index);
        queue.extend(upstream[offsets[index]..offsets[index + 1]].iter().copied());
    }
    order
}

fn erode_in_order(
    grid: &mut Grid<'_>,
    uplift_map: Option<&[f32]>,
    params: &StreamPower,
    flow: &Flow,
    order: &[usize],
    cancel: &dyn Fn() -> bool,
) -> Result<(), ErosionError> {
    for &index in order {
        if index % grid.width == 0 && cancel() {
            return Err(ErosionError::Cancelled);
        }
        erode_cell(grid, uplift_map, params, flow, index);
    }
    Ok(())
}

fn erode_cell(
    grid: &mut Grid<'_>,
    uplift_map: Option<&[f32]>,
    params: &StreamPower,
    flow: &Flow,
    index: usize,
) {
    let downstream = flow.receiver[index];
    let uplift = params.uplift * uplift_map.map_or(1.0, |map| map[index]);
    if downstream == u32::MAX || params.k == 0.0 {
        grid.heights[index] = (grid.heights[index] + params.dt * uplift).max(0.0);
        return;
    }
    let Ok(downstream) = usize::try_from(downstream) else {
        return;
    };
    let dx = (index % grid.width).abs_diff(downstream % grid.width);
    let dz = (index / grid.width).abs_diff(downstream / grid.width);
    let diagonal = dx != 0 && dz != 0;
    let distance = grid.cell
        * if diagonal {
            std::f32::consts::SQRT_2
        } else {
            1.0
        };
    let area = flow.area[index];
    let area_power = match params.m {
        0.5 => area.sqrt(),
        1.0 => area,
        _ => area * area.sqrt(),
    };
    let coefficient = params.dt * params.k * area_power / distance;
    grid.heights[index] =
        ((grid.heights[index] + params.dt * uplift + coefficient * grid.heights[downstream])
            / (1.0 + coefficient))
            .max(0.0);
}

fn bad(name: &'static str, why: &str) -> ErosionError {
    ErosionError::BadParameter {
        name,
        why: why.to_owned(),
    }
}
