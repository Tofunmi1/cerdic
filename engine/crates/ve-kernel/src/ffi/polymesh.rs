//! Audited FFI for polygon-mesh operators built on Blender's bmesh.
use crate::KernelError;
use crate::bevel::{BevelOptions, PolygonMesh};
use crate::mesh_ops::MeshSelection;
use core::ptr::NonNull;
use ve_kernel_sys as sys;

struct Handle(NonNull<sys::VePolymesh>);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: this is the unique owning pointer returned by the C ABI.
        unsafe { sys::ve_polymesh_free(self.0.as_ptr()) };
    }
}

/// Bevels `edges` (vertex pairs) of the mesh.
pub(crate) fn bevel(
    mesh: &PolygonMesh<'_>,
    edges: &[[u32; 2]],
    options: &BevelOptions,
) -> Result<crate::bevel::BeveledMesh, KernelError> {
    let flat_positions = mesh.positions.as_flattened();
    let flat_edges = edges.as_flattened();
    let input = sys::VeBevelInput {
        struct_size: core::mem::size_of::<sys::VeBevelInput>(),
        mesh: sys::VePolymeshInput {
            struct_size: core::mem::size_of::<sys::VePolymeshInput>(),
            positions: flat_positions.as_ptr(),
            vertex_count: mesh.positions.len(),
            face_sizes: mesh.face_sizes.as_ptr(),
            face_count: mesh.face_sizes.len(),
            corners: mesh.corners.as_ptr(),
            corner_count: mesh.corners.len(),
        },
        edges: flat_edges.as_ptr(),
        edge_count: edges.len(),
        width: options.width,
        segments: i32::try_from(options.segments).unwrap_or(i32::MAX),
        offset_type: options.offset_type.code(),
        profile: options.profile,
    };
    let mut status = sys::VE_OK;
    // SAFETY: `input` points at slices that outlive the call (positions hold `3 * vertex_count` floats,
    // `face_sizes` and `corners` the counts passed with them, `edges` two indices per edge); the C side
    // validates every index and copies what it keeps. `status` is a valid out-pointer.
    let ptr = unsafe { sys::ve_bevel(&raw const input, &raw mut status) };
    let handle = match NonNull::new(ptr) {
        Some(ptr) if status == sys::VE_OK => Handle(ptr),
        Some(ptr) => {
            drop(Handle(ptr));
            return Err(KernelError::from_status(status));
        }
        None => return Err(KernelError::from_status(status)),
    };
    read(&handle)
}

/// Runs one of Blender's bmesh operators (`VE_MESH_OP_*`) on the mesh.
pub(crate) fn mesh_op(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    code: i32,
    params: [f64; 12],
    flags: i32,
) -> Result<crate::bevel::BeveledMesh, KernelError> {
    let flat_positions = mesh.positions.as_flattened();
    let flat_edges = selection.edges.as_flattened();
    let input = sys::VeMeshOpInput {
        struct_size: core::mem::size_of::<sys::VeMeshOpInput>(),
        mesh: sys::VePolymeshInput {
            struct_size: core::mem::size_of::<sys::VePolymeshInput>(),
            positions: flat_positions.as_ptr(),
            vertex_count: mesh.positions.len(),
            face_sizes: mesh.face_sizes.as_ptr(),
            face_count: mesh.face_sizes.len(),
            corners: mesh.corners.as_ptr(),
            corner_count: mesh.corners.len(),
        },
        op: code,
        faces: selection.faces.as_ptr(),
        face_count: selection.faces.len(),
        edges: flat_edges.as_ptr(),
        edge_count: selection.edges.len(),
        vertices: selection.vertices.as_ptr(),
        vertex_count: selection.vertices.len(),
        params,
        flags,
    };
    let mut status = sys::VE_OK;
    // SAFETY: `input` points at slices that outlive the call (positions hold `3 * vertex_count` floats,
    // `face_sizes` and `corners` the counts passed with them, `faces` one index per selected face and
    // `edges` two indices per selected edge); the C side validates every index and copies what it
    // keeps. `status` is a valid out-pointer.
    let ptr = unsafe { sys::ve_mesh_op(&raw const input, &raw mut status) };
    match NonNull::new(ptr) {
        Some(ptr) if status == sys::VE_OK => read(&Handle(ptr)),
        Some(ptr) => {
            drop(Handle(ptr));
            Err(KernelError::from_status(status))
        }
        None => Err(KernelError::from_status(status)),
    }
}

fn read(handle: &Handle) -> Result<crate::bevel::BeveledMesh, KernelError> {
    let ptr = handle.0.as_ptr().cast_const();
    // SAFETY: `ptr` is a live mesh owned by `handle`; the count queries only read it.
    let (vertices, faces, corners) = unsafe {
        (
            sys::ve_polymesh_num_vertices(ptr),
            sys::ve_polymesh_num_faces(ptr),
            sys::ve_polymesh_num_corners(ptr),
        )
    };
    let mut positions = vec![[0.0_f32; 3]; vertices];
    let mut face_sizes = vec![0_u32; faces];
    let mut corner_indices = vec![0_u32; corners];
    let mut vertex_origin = vec![0_u32; vertices];
    let mut face_parent = vec![0_u32; faces];
    // SAFETY: every destination buffer holds exactly the capacity passed with it, taken from the counts
    // the same mesh just reported; the copy writes no more than that and retains nothing.
    let status = unsafe {
        sys::ve_polymesh_copy(
            ptr,
            positions.as_mut_ptr().cast::<f32>(),
            3 * vertices,
            face_sizes.as_mut_ptr(),
            faces,
            corner_indices.as_mut_ptr(),
            corners,
            vertex_origin.as_mut_ptr(),
            vertices,
            face_parent.as_mut_ptr(),
            faces,
        )
    };
    if status != sys::VE_OK {
        return Err(KernelError::from_status(status));
    }
    let mut face_role = vec![0_u32; faces];
    // SAFETY: `face_role` holds exactly `faces` entries, the capacity passed; `ptr` is the live mesh.
    let status = unsafe { sys::ve_polymesh_copy_roles(ptr, face_role.as_mut_ptr(), faces) };
    if status != sys::VE_OK {
        return Err(KernelError::from_status(status));
    }
    Ok(crate::bevel::BeveledMesh {
        positions,
        face_sizes,
        corners: corner_indices,
        vertex_origin,
        face_parent,
        face_role,
    })
}
