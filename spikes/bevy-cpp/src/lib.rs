//! Spike: C++ Manifold (wasip1-compiled) + Bevy mesh/ecs/render crates in one wasm32-unknown-unknown module.
use bevy_asset::RenderAssetUsages;
use bevy_mesh::{Indices, Mesh, PrimitiveTopology};
use std::ffi::c_void;
use wasm_bindgen::prelude::*;

type MManifold = c_void;
type MeshGL = c_void;
unsafe extern "C" {
    fn __wasm_call_ctors();
    fn manifold_manifold_size() -> usize;
    fn manifold_meshgl_size() -> usize;
    fn manifold_cube(mem: *mut c_void, x: f64, y: f64, z: f64, center: i32) -> *mut MManifold;
    fn manifold_sphere(mem: *mut c_void, r: f64, segments: i32) -> *mut MManifold;
    fn manifold_difference(mem: *mut c_void, a: *mut MManifold, b: *mut MManifold) -> *mut MManifold;
    fn manifold_status(m: *mut MManifold) -> i32;
    fn manifold_get_meshgl(mem: *mut c_void, m: *mut MManifold) -> *mut MeshGL;
    fn manifold_meshgl_num_prop(m: *mut MeshGL) -> usize;
    fn manifold_meshgl_num_vert(m: *mut MeshGL) -> usize;
    fn manifold_meshgl_num_tri(m: *mut MeshGL) -> usize;
    fn manifold_meshgl_vert_properties(mem: *mut c_void, m: *mut MeshGL) -> *mut f32;
    fn manifold_meshgl_tri_verts(mem: *mut c_void, m: *mut MeshGL) -> *mut u32;
    fn manifold_destruct_manifold(m: *mut MManifold);
    fn manifold_destruct_meshgl(m: *mut MeshGL);
}

struct Slot(Vec<u64>);
impl Slot {
    fn new(bytes: usize) -> Self { Slot(vec![0u64; bytes.div_ceil(8)]) }
    fn ptr(&mut self) -> *mut c_void { self.0.as_mut_ptr().cast() }
}

/// cube - sphere through the C++ Manifold C API, returned as (positions, indices).
fn manifold_csg(segments: i32) -> (Vec<[f32; 3]>, Vec<u32>) {
    unsafe {
        let ms = manifold_manifold_size();
        let (mut a, mut b, mut c, mut g) = (Slot::new(ms), Slot::new(ms), Slot::new(ms), Slot::new(manifold_meshgl_size()));
        let cube = manifold_cube(a.ptr(), 1.0, 1.0, 1.0, 1);
        let sphere = manifold_sphere(b.ptr(), 0.65, segments);
        let diff = manifold_difference(c.ptr(), cube, sphere);
        assert_eq!(manifold_status(diff), 0);
        let mesh = manifold_get_meshgl(g.ptr(), diff);
        let (np, nv, nt) = (manifold_meshgl_num_prop(mesh), manifold_meshgl_num_vert(mesh), manifold_meshgl_num_tri(mesh));
        let mut props = vec![0f32; nv * np];
        let mut idx = vec![0u32; nt * 3];
        manifold_meshgl_vert_properties(props.as_mut_ptr().cast(), mesh);
        manifold_meshgl_tri_verts(idx.as_mut_ptr().cast(), mesh);
        manifold_destruct_meshgl(mesh);
        for m in [cube, sphere, diff] { manifold_destruct_manifold(m); }
        (props.chunks(np).map(|p| [p[0], p[1], p[2]]).collect(), idx)
    }
}

#[wasm_bindgen]
pub fn boot() -> String {
    unsafe { __wasm_call_ctors() };
    let (pos, idx) = manifold_csg(48);
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.clone());
    mesh.insert_indices(Indices::U32(idx.clone()));
    mesh.compute_normals();
    let has_normals = mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some();
    format!("manifold(C++) -> bevy_mesh::Mesh: {} verts, {} tris, normals={}", pos.len(), idx.len() / 3, has_normals)
}
