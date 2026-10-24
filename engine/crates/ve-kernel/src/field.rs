//! Bounded, deterministic instruction tapes for scalar fields.
use crate::{ExecutionContext, KernelError, ffi};

/// Maximum sampled grid cells in a final field result.
pub const FIELD_MAX_CELLS: usize = 6_000_000;
/// Maximum tape length.
pub const FIELD_MAX_INSTRUCTIONS: usize = 4_096;
/// Maximum estimated instruction-by-cell work.
pub const FIELD_MAX_OPERATIONS: usize = 1_500_000_000;
const FIELD_MAX_PADDED_SLOTS: usize = 16_777_216;
const FIELD_MAX_CELLS_F64: f64 = 6_000_000.0;

/// A register-tape operation. Field values are positive inside and zero on the surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FieldOp {
    /// Sphere: center XYZ and radius W.
    Sphere = 1,
    /// Rounded box: half extents XYZ and round radius W.
    Box = 2,
    /// Y-up torus: major radius X, tube radius Y, centered at zero.
    Torus = 3,
    /// Y-axis capsule: radius X and half segment length Y.
    Capsule = 4,
    /// Y-up cylinder: radius X and half-height Y.
    Cylinder = 5,
    /// Half space: non-zero normal XYZ and signed offset W.
    Plane = 6,
    /// Axis-aligned ellipsoid at the origin; radii XYZ.
    Ellipsoid = 7,
    /// Internal tube endpoint data; not a field result by itself.
    TubePoint = 8,
    /// Exact radial distance to the segment between two earlier tube-point entries.
    TubeSegment = 9,
    /// Union of two earlier registers.
    Union = 10,
    /// Intersection of two earlier registers.
    Intersection = 11,
    /// Difference of A minus B.
    Difference = 12,
    /// Smooth union of two earlier registers; blend radius X.
    SmoothUnion = 13,
    /// Translate the child by XYZ.
    Move = 20,
    /// Uniformly scale the child by X.
    Scale = 21,
    /// Mirror the child across the axes selected by X's low three bits.
    Mirror = 22,
    /// Offset the child by X.
    Offset = 23,
    /// Create a symmetric shell with half-width X.
    Shell = 24,
    /// Fold coordinates into a bounded repeat; XYZ are periods and `input_b` packs counts.
    Repeat = 25,
    /// Seeded bounded fractal noise displacement; constants are amplitude, scale, persistence, packed mode/octaves.
    Noise = 26,
    /// Axis warp: kind, axis and toward packed in W; from, to, start and end in XYZW (W packing takes precedence impossible).
    Warp = 27,
}

/// One instruction; inputs must reference registers earlier in the same tape.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct FieldInstruction {
    /// Operation tag.
    pub opcode: FieldOp,
    /// First input register, where the operation takes an input.
    pub input_a: u32,
    /// Second input register, where the operation takes two inputs.
    pub input_b: u32,
    /// Operation-specific constant.
    pub x: f64,
    /// Operation-specific constant.
    pub y: f64,
    /// Operation-specific constant.
    pub z: f64,
    /// Operation-specific constant.
    pub w: f64,
}

impl FieldInstruction {
    /// Creates a leaf primitive instruction with no input registers.
    #[must_use]
    pub const fn leaf(opcode: FieldOp, values: [f64; 4]) -> Self {
        Self {
            opcode,
            input_a: 0,
            input_b: 0,
            x: values[0],
            y: values[1],
            z: values[2],
            w: values[3],
        }
    }

    /// Creates a unary instruction that consumes an earlier register.
    #[must_use]
    pub const fn unary(opcode: FieldOp, input: u32, values: [f64; 4]) -> Self {
        Self {
            opcode,
            input_a: input,
            input_b: 0,
            x: values[0],
            y: values[1],
            z: values[2],
            w: values[3],
        }
    }

    /// Creates a binary instruction that consumes earlier registers.
    #[must_use]
    pub const fn binary(opcode: FieldOp, a: u32, b: u32, values: [f64; 4]) -> Self {
        Self {
            opcode,
            input_a: a,
            input_b: b,
            x: values[0],
            y: values[1],
            z: values[2],
            w: values[3],
        }
    }
}

/// Why bounded field meshing failed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum FieldError {
    /// A boundary sample lies inside the field; its location is provided.
    #[error("field reaches the bounds at [{point:?}]")]
    Clipped {
        /// First sampled point on the bounds where the field is positive.
        point: [f64; 3],
    },
    /// No surface crossed the sampled volume.
    #[error("field produced an empty solid")]
    Empty,
    /// A cell, padded storage or operation cap was exceeded.
    #[error("field meshing exceeded a work or memory limit")]
    Budget,
    /// The underlying geometry kernel failed.
    #[error(transparent)]
    Kernel(#[from] KernelError),
}

pub(crate) fn from_tape(
    instructions: &[FieldInstruction],
    bounds: [[f64; 3]; 2],
    edge_length: f64,
    context: &ExecutionContext,
) -> Result<ffi::Handle, FieldError> {
    preflight(instructions, bounds, edge_length)?;
    ffi::field::from_tape(instructions, bounds, edge_length, context).map_err(|error| match error {
        ffi::field::Failure::Clipped(point) => FieldError::Clipped { point },
        ffi::field::Failure::Empty => FieldError::Empty,
        ffi::field::Failure::Budget => FieldError::Budget,
        ffi::field::Failure::Kernel(error) => FieldError::Kernel(error),
    })
}

/// Evaluates one point with the C++ interpreter used by field meshing.
///
/// This is for conformance checks and field inspection. Level-set sampling calls the interpreter
/// directly inside C++ and does not cross the ABI per sample.
///
/// # Errors
/// Returns an error for invalid instructions, a non-finite point or an invalid tape.
pub fn evaluate_tape(
    instructions: &[FieldInstruction],
    point: [f64; 3],
) -> Result<f64, KernelError> {
    if instructions.is_empty()
        || instructions.len() > FIELD_MAX_INSTRUCTIONS
        || point.into_iter().any(|value| !value.is_finite())
    {
        return Err(KernelError::InvalidArgument("invalid field sample"));
    }
    ffi::field::evaluate_tape(instructions, point)
}

fn preflight(
    instructions: &[FieldInstruction],
    bounds: [[f64; 3]; 2],
    edge_length: f64,
) -> Result<(), FieldError> {
    if instructions.is_empty() || instructions.len() > FIELD_MAX_INSTRUCTIONS {
        return Err(FieldError::Budget);
    }
    if !edge_length.is_finite() || edge_length <= 0.0 {
        return Err(
            KernelError::InvalidArgument("field edge length must be positive and finite").into(),
        );
    }
    let mut grid = [0_usize; 3];
    for axis in 0..3 {
        let min = bounds[0][axis];
        let max = bounds[1][axis];
        if !min.is_finite() || !max.is_finite() || min >= max || min.abs().max(max.abs()) > 1.0e6 {
            return Err(
                KernelError::InvalidArgument("field bounds must be finite and ordered").into(),
            );
        }
        let ratio = (max - min) / edge_length;
        if !ratio.is_finite() || ratio < 1.0 {
            return Err(KernelError::InvalidArgument(
                "field bounds need at least two samples per axis",
            )
            .into());
        }
        if ratio > FIELD_MAX_CELLS_F64 {
            return Err(FieldError::Budget);
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "positive ratio is bounded by two million before conversion"
        )]
        let axis_samples = ratio as usize + 1;
        grid[axis] = axis_samples;
    }
    let Some(cells) = grid[0]
        .checked_mul(grid[1])
        .and_then(|n| n.checked_mul(grid[2]))
    else {
        return Err(FieldError::Budget);
    };
    let weighted_instructions = instructions.iter().fold(0_usize, |work, instruction| {
        let extra = if instruction.opcode == FieldOp::Noise {
            noise_octaves(instruction.w)
        } else {
            0
        };
        work.saturating_add(1 + extra.saturating_sub(1))
    });
    let Some(work) = cells.checked_mul(weighted_instructions) else {
        return Err(FieldError::Budget);
    };
    if cells > FIELD_MAX_CELLS || work > FIELD_MAX_OPERATIONS {
        return Err(FieldError::Budget);
    }
    let padded = grid.map(|n| (n + 2).next_power_of_two());
    let Some(slots) = padded[0]
        .checked_mul(padded[1])
        .and_then(|n| n.checked_mul(padded[2]))
    else {
        return Err(FieldError::Budget);
    };
    if slots > FIELD_MAX_PADDED_SLOTS {
        return Err(FieldError::Budget);
    }
    Ok(())
}

fn noise_octaves(packed: f64) -> usize {
    let value = if packed >= 9.0 { packed - 8.0 } else { packed };
    if value <= 1.0 {
        1
    } else if value <= 2.0 {
        2
    } else if value <= 3.0 {
        3
    } else if value <= 4.0 {
        4
    } else if value <= 5.0 {
        5
    } else {
        6
    }
}
