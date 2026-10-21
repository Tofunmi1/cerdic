//! Shallow-water hydraulic erosion with virtual pipes.
use super::{ErosionError, Grid};

/// Hydraulic erosion settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hydraulic {
    /// Rainfall depth per iteration, in metres.
    pub rain: f32,
    /// Fraction of water evaporated each iteration.
    pub evaporation: f32,
    /// Sediment capacity coefficient.
    pub capacity: f32,
    /// Dissolving rate.
    pub dissolve: f32,
    /// Deposition rate.
    pub deposit: f32,
    /// Number of iterations.
    pub iterations: u32,
}

/// Water depth and suspended sediment after simulation.
#[derive(Debug, Clone, PartialEq)]
pub struct Hydrology {
    /// Water depth in metres.
    pub water: Vec<f32>,
    /// Suspended sediment in metres of height.
    pub sediment: Vec<f32>,
}

struct StepState<'a> {
    water: &'a [f32],
    sediment: &'a [f32],
    flux: &'a [[f32; 4]],
    velocity: &'a mut [[f32; 2]],
    next_water: &'a mut [f32],
    next_sediment: &'a mut [f32],
}

/// Runs shallow-water erosion using four closed-boundary virtual pipes.
///
/// # Errors
/// Returns shape, cancellation, or invalid parameter errors.
pub fn hydraulic(
    grid: &mut Grid<'_>,
    params: &Hydraulic,
    cancel: &dyn Fn() -> bool,
) -> Result<Hydrology, ErosionError> {
    grid.check()?;
    validate(params)?;
    let count = grid.heights.len();
    let mut water = vec![0.0; count];
    let mut sediment = vec![0.0; count];
    let mut next_water = vec![0.0; count];
    let mut next_sediment = vec![0.0; count];
    let mut flux = vec![[0.0_f32; 4]; count];
    let mut velocity = vec![[0.0_f32; 2]; count];
    for _ in 0..params.iterations {
        add_rain(grid, params.rain, cancel, &mut water)?;
        calculate_flux(grid, cancel, &water, &mut flux)?;
        let mut state = StepState {
            water: &water,
            sediment: &sediment,
            flux: &flux,
            velocity: &mut velocity,
            next_water: &mut next_water,
            next_sediment: &mut next_sediment,
        };
        update_water_and_sediment(grid, params, cancel, &mut state)?;
        std::mem::swap(&mut water, &mut next_water);
        std::mem::swap(&mut sediment, &mut next_sediment);
    }
    Ok(Hydrology { water, sediment })
}

fn validate(params: &Hydraulic) -> Result<(), ErosionError> {
    for (name, value, maximum) in [
        ("rain", params.rain, 0.1),
        ("evaporation", params.evaporation, 1.0),
        ("capacity", params.capacity, f32::MAX),
        ("dissolve", params.dissolve, 1.0),
        ("deposit", params.deposit, 1.0),
    ] {
        if !value.is_finite() || value < 0.0 || value > maximum {
            return Err(ErosionError::BadParameter {
                name,
                why: "outside documented range".to_owned(),
            });
        }
    }
    Ok(())
}

fn add_rain(
    grid: &Grid<'_>,
    rain: f32,
    cancel: &dyn Fn() -> bool,
    water: &mut [f32],
) -> Result<(), ErosionError> {
    for z in 0..grid.depth {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        let row = z * grid.width;
        for x in 0..grid.width {
            water[row + x] += rain;
        }
    }
    Ok(())
}

fn calculate_flux(
    grid: &Grid<'_>,
    cancel: &dyn Fn() -> bool,
    water: &[f32],
    flux: &mut [[f32; 4]],
) -> Result<(), ErosionError> {
    flux.fill([0.0; 4]);
    for z in 0..grid.depth {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        for x in 0..grid.width {
            let i = z * grid.width + x;
            let surface = grid.heights[i] + water[i];
            let mut total = 0.0;
            for (direction, outflow) in flux[i].iter_mut().enumerate() {
                if let Some(j) = neighbour(x, z, direction, grid.width, grid.depth) {
                    let drop = (surface - grid.heights[j] - water[j]).max(0.0);
                    *outflow = drop;
                    total += drop;
                }
            }
            if total > 0.0 {
                let scale = (water[i] / total).min(1.0) * 0.25;
                for outflow in &mut flux[i] {
                    *outflow *= scale;
                }
            }
        }
    }
    Ok(())
}

fn update_water_and_sediment(
    grid: &mut Grid<'_>,
    params: &Hydraulic,
    cancel: &dyn Fn() -> bool,
    state: &mut StepState<'_>,
) -> Result<(), ErosionError> {
    state.next_water.fill(0.0);
    state.next_sediment.fill(0.0);
    for z in 0..grid.depth {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        for x in 0..grid.width {
            update_cell(grid, params, state, x, z);
        }
    }
    Ok(())
}

fn update_cell(
    grid: &mut Grid<'_>,
    params: &Hydraulic,
    state: &mut StepState<'_>,
    x: usize,
    z: usize,
) {
    let index = z * grid.width + x;
    let mut outgoing = 0.0;
    let mut incoming = 0.0;
    let mut velocity_x = 0.0;
    let mut velocity_z = 0.0;
    for direction in 0..4 {
        outgoing += state.flux[index][direction];
        if let Some(neighbor) = neighbour(x, z, direction, grid.width, grid.depth) {
            let opposite = (direction + 2) % 4;
            incoming += state.flux[neighbor][opposite];
        }
        let (dx, dz) = direction_vector(direction);
        velocity_x += state.flux[index][direction] * dx;
        velocity_z += state.flux[index][direction] * dz;
    }
    let depth = (state.water[index] - outgoing + incoming).max(0.0);
    state.velocity[index] = [
        velocity_x / depth.max(1.0e-6),
        velocity_z / depth.max(1.0e-6),
    ];
    let speed = (state.velocity[index][0] * state.velocity[index][0]
        + state.velocity[index][1] * state.velocity[index][1])
        .sqrt();
    let slope = local_slope(grid.heights, x, z, grid.width, grid.depth, grid.cell);
    let capacity = params.capacity * slope * speed;
    let mut ground = grid.heights[index];
    let mut carried = state.sediment[index];
    if carried < capacity {
        let amount = ((capacity - carried) * params.dissolve).min(ground.max(0.0));
        ground -= amount;
        carried += amount;
    } else {
        let amount = ((carried - capacity) * params.deposit).min(carried);
        ground += amount;
        carried -= amount;
    }
    grid.heights[index] = ground;
    let advected = advect_sediment(
        index,
        x,
        z,
        grid.width,
        grid.depth,
        state.flux,
        state.sediment,
        state.water,
        outgoing,
    );
    state.next_sediment[index] = (carried + advected).max(0.0);
    state.next_water[index] = depth * (1.0 - params.evaporation);
}

fn neighbour(x: usize, z: usize, direction: usize, width: usize, depth: usize) -> Option<usize> {
    match direction {
        0 if x + 1 < width => Some(z * width + x + 1),
        1 if z + 1 < depth => Some((z + 1) * width + x),
        2 if x > 0 => Some(z * width + x - 1),
        3 if z > 0 => Some((z - 1) * width + x),
        _ => None,
    }
}

fn direction_vector(direction: usize) -> (f32, f32) {
    match direction {
        0 => (1.0, 0.0),
        1 => (0.0, 1.0),
        2 => (-1.0, 0.0),
        _ => (0.0, -1.0),
    }
}

fn local_slope(heights: &[f32], x: usize, z: usize, width: usize, depth: usize, cell: f32) -> f32 {
    let i = z * width + x;
    let mut slope = 0.0_f32;
    for direction in 0..4 {
        if let Some(j) = neighbour(x, z, direction, width, depth) {
            slope = slope.max(((heights[i] - heights[j]) / cell).abs());
        }
    }
    slope
}

fn advect_sediment(
    i: usize,
    x: usize,
    z: usize,
    width: usize,
    depth: usize,
    flux: &[[f32; 4]],
    sediment: &[f32],
    water: &[f32],
    outgoing: f32,
) -> f32 {
    let carried_out = sediment[i] * (outgoing / water[i].max(1.0e-6)).min(1.0);
    let mut carried_in = 0.0;
    for direction in 0..4 {
        if let Some(j) = neighbour(x, z, direction, width, depth) {
            let opposite = (direction + 2) % 4;
            carried_in += sediment[j] * flux[j][opposite] / water[j].max(1.0e-6);
        }
    }
    carried_in - carried_out
}
