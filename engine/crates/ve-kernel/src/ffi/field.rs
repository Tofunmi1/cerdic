//! Checked ownership transfer for the field-meshing C ABI.
use super::{Handle, ensure_initialized};
use crate::{ExecutionContext, KernelError, field::FieldInstruction};
use ve_kernel_sys as sys;

pub(crate) enum Failure {
    Clipped([f64; 3]),
    Empty,
    Budget,
    Kernel(KernelError),
}

pub(crate) fn from_tape(
    instructions: &[FieldInstruction],
    bounds: [[f64; 3]; 2],
    edge_length: f64,
    context: &ExecutionContext,
) -> Result<Handle, Failure> {
    ensure_initialized();
    let ffi_instructions: Vec<_> = instructions
        .iter()
        .map(|instruction| sys::VeFieldInstruction {
            opcode: instruction.opcode as u32,
            input_a: instruction.input_a,
            input_b: instruction.input_b,
            x: instruction.x,
            y: instruction.y,
            z: instruction.z,
            w: instruction.w,
        })
        .collect();
    let input = sys::VeFieldInput {
        struct_size: core::mem::size_of::<sys::VeFieldInput>(),
        instructions: ffi_instructions.as_ptr(),
        instruction_count: ffi_instructions.len(),
        bounds: [
            bounds[0][0],
            bounds[0][1],
            bounds[0][2],
            bounds[1][0],
            bounds[1][1],
            bounds[1][2],
        ],
        edge_length,
        max_cells: crate::field::FIELD_MAX_CELLS,
        max_operations: crate::field::FIELD_MAX_OPERATIONS,
        context: context.0.as_mut_ptr(),
    };
    let mut point = [0.0; 3];
    let mut status = sys::VE_OK;
    // SAFETY: input/instruction slices, context, point and status stay alive through the call;
    // C++ borrows inputs synchronously and returns an owned solid handle.
    let handle =
        unsafe { sys::ve_solid_from_field(&raw const input, point.as_mut_ptr(), &raw mut status) };
    if status == sys::VE_ERR_FIELD_CLIPPED {
        return Err(Failure::Clipped(point));
    }
    if status == sys::VE_ERR_FIELD_EMPTY {
        return Err(Failure::Empty);
    }
    if status == sys::VE_ERR_FIELD_BUDGET {
        return Err(Failure::Budget);
    }
    Handle::from_raw(handle, status).map_err(Failure::Kernel)
}

pub(crate) fn evaluate_tape(
    instructions: &[FieldInstruction],
    point: [f64; 3],
) -> Result<f64, KernelError> {
    ensure_initialized();
    let ffi_instructions: Vec<_> = instructions
        .iter()
        .map(|instruction| sys::VeFieldInstruction {
            opcode: instruction.opcode as u32,
            input_a: instruction.input_a,
            input_b: instruction.input_b,
            x: instruction.x,
            y: instruction.y,
            z: instruction.z,
            w: instruction.w,
        })
        .collect();
    let mut value = 0.0;
    // SAFETY: the instruction and point buffers plus output remain alive through this synchronous
    // query; C++ only reads inputs and writes the scalar output.
    let status = unsafe {
        sys::ve_field_sample(
            ffi_instructions.as_ptr(),
            ffi_instructions.len(),
            point.as_ptr(),
            &raw mut value,
        )
    };
    if status == sys::VE_OK {
        Ok(value)
    } else {
        Err(KernelError::from_status(status))
    }
}
