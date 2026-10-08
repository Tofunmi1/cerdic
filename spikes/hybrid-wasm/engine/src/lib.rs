//! Hybrid spike: Rust engine calling C++ (Manifold, meshoptimizer, our own code)
//! and Rust (Rapier, boolmesh), all inside one wasm32-wasip1 module with a C ABI boundary.
//! Each stage is a separate export so the host can time it.
use rapier3d::prelude::*;
use std::ffi::c_void;

type MManifold = c_void;
type MeshGL = c_void;

extern "C" {
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
    fn meshopt_simplify(
        dst: *mut u32, indices: *const u32, index_count: usize, positions: *const f32,
        vertex_count: usize, stride: usize, target_index_count: usize, target_error: f32,
        options: u32, result_error: *mut f32,
    ) -> usize;
    fn gp_weld(positions: *const f32, vertex_count: u32, epsilon: f32, remap: *mut u32) -> u32;
}

/// WASI reactor entry: run C++ static constructors once. Exporting this also stops
/// wasm-ld from wrapping every export with ctor/dtor calls (which destroys C++ globals).
#[no_mangle]
pub extern "C" fn _initialize() {
    unsafe { __wasm_call_ctors() }
}

#[derive(Default, Clone)]
struct Mesh {
    positions: Vec<f32>,
    indices: Vec<u32>,
}

/// Caller-owned Manifold storage (the C API constructs objects in memory we provide).
struct Slot(Vec<u64>);
impl Slot {
    fn new(bytes: usize) -> Self { Slot(vec![0u64; bytes.div_ceil(8)]) }
    fn ptr(&mut self) -> *mut c_void { self.0.as_mut_ptr().cast() }
}

unsafe fn read_mesh(m: *mut MManifold) -> Mesh {
    let mut g = Slot::new(manifold_meshgl_size());
    let mesh = manifold_get_meshgl(g.ptr(), m);
    let (nprop, nvert, ntri) =
        (manifold_meshgl_num_prop(mesh), manifold_meshgl_num_vert(mesh), manifold_meshgl_num_tri(mesh));
    let mut props = vec![0f32; nvert * nprop];
    let mut indices = vec![0u32; ntri * 3];
    manifold_meshgl_vert_properties(props.as_mut_ptr().cast(), mesh);
    manifold_meshgl_tri_verts(indices.as_mut_ptr().cast(), mesh);
    manifold_destruct_meshgl(mesh);
    let positions = props.chunks(nprop).flat_map(|p| p[..3].to_vec()).collect();
    Mesh { positions, indices }
}

struct State {
    segments: i32,
    cube: Mesh,
    sphere: Mesh,
    result: Mesh,
    report: String,
}
static mut STATE: Option<State> = None;
fn state() -> &'static mut State { unsafe { (*std::ptr::addr_of_mut!(STATE)).as_mut().expect("setup") } }

/// C++ Manifold: cube − sphere, mesh read back into Rust.
fn manifold_csg(segments: i32) -> Mesh {
    unsafe {
        let ms = manifold_manifold_size();
        let (mut a, mut b, mut c) = (Slot::new(ms), Slot::new(ms), Slot::new(ms));
        let cube = manifold_cube(a.ptr(), 1.0, 1.0, 1.0, 1);
        let sphere = manifold_sphere(b.ptr(), 0.65, segments);
        let diff = manifold_difference(c.ptr(), cube, sphere);
        assert_eq!(manifold_status(diff), 0);
        let out = read_mesh(diff);
        for m in [cube, sphere, diff] { manifold_destruct_manifold(m); }
        out
    }
}

/// Rust boolmesh on the same input meshes.
fn boolmesh_csg(cube: &Mesh, sphere: &Mesh) -> Mesh {
    use boolmesh::prelude::*;
    let conv = |m: &Mesh| {
        let p: Vec<f64> = m.positions.iter().map(|&v| v as f64).collect();
        let i: Vec<usize> = m.indices.iter().map(|&v| v as usize).collect();
        Manifold::new(&p, &i).expect("boolmesh input")
    };
    let r = compute_boolean(&conv(cube), &conv(sphere), OpType::Subtract).expect("boolmesh");
    let positions = r.ps.iter().flat_map(|p| [p.x as f32, p.y as f32, p.z as f32]).collect();
    let indices = r.hs.iter().map(|h| h.tail as u32).collect();
    Mesh { positions, indices }
}

fn weld(m: &Mesh) -> u32 {
    let n = m.positions.len() / 3;
    let mut remap = vec![0u32; n];
    unsafe { gp_weld(m.positions.as_ptr(), n as u32, 1e-5, remap.as_mut_ptr()) }
}

fn simplify(m: &Mesh) -> usize {
    let mut out = vec![0u32; m.indices.len()];
    let mut err = 0f32;
    let target = (m.indices.len() / 4 / 3) * 3;
    unsafe {
        meshopt_simplify(
            out.as_mut_ptr(), m.indices.as_ptr(), m.indices.len(), m.positions.as_ptr(),
            m.positions.len() / 3, 12, target, 0.05, 0, &mut err,
        ) / 3
    }
}

/// Rapier: mesh as static trimesh, drop a ball on a solid corner, step 240 frames (4 s).
fn physics(m: &Mesh) -> f32 {
    let verts: Vec<Vector> = m.positions.chunks(3).map(|p| Vector::new(p[0], p[1], p[2])).collect();
    let idx: Vec<[u32; 3]> = m.indices.chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
    let mut bodies = RigidBodySet::new();
    let mut colliders = ColliderSet::new();
    colliders.insert(ColliderBuilder::trimesh(verts, idx).expect("trimesh").build());
    let ball = bodies.insert(RigidBodyBuilder::dynamic().translation(Vector::new(0.42, 2.0, 0.42)).build());
    colliders.insert_with_parent(ColliderBuilder::ball(0.1).build(), ball, &mut bodies);
    let mut pipeline = PhysicsPipeline::new();
    let (mut islands, mut broad, mut narrow) = (IslandManager::new(), DefaultBroadPhase::new(), NarrowPhase::new());
    let (mut impulses, mut multibody, mut soft, mut ccd) =
        (ImpulseJointSet::new(), MultibodyJointSet::new(), SoftBodySet::new(), CCDSolver::new());
    let params = IntegrationParameters::default();
    let mut landed = f32::NAN;
    for step in 0..240 {
        pipeline.step(
            Vector::new(0.0, -9.81, 0.0), &params, &mut islands, &mut broad, &mut narrow, &mut bodies,
            &mut colliders, &mut impulses, &mut multibody, &mut soft, &mut ccd, &(), &(),
        );
        if step == 45 { landed = bodies[ball].translation().y; }
    }
    landed
}

#[no_mangle]
pub extern "C" fn setup(segments: i32) {
    let (cube, sphere) = unsafe {
        let ms = manifold_manifold_size();
        let (mut a, mut b) = (Slot::new(ms), Slot::new(ms));
        let c = manifold_cube(a.ptr(), 1.0, 1.0, 1.0, 1);
        let s = manifold_sphere(b.ptr(), 0.65, segments);
        let out = (read_mesh(c), read_mesh(s));
        manifold_destruct_manifold(c);
        manifold_destruct_manifold(s);
        out
    };
    unsafe {
        *std::ptr::addr_of_mut!(STATE) =
            Some(State { segments, cube, sphere, result: Mesh::default(), report: String::new() });
    }
}

/// Stage IDs: 0 manifold csg, 1 boolmesh csg, 2 weld, 3 simplify, 4 physics,
/// 5 pipeline C++ csg → weld → simplify → physics, 6 pipeline Rust csg → physics.
#[no_mangle]
pub extern "C" fn run_stage(stage: u32, iterations: u32) -> u32 {
    let s = state();
    let mut out = 0u32;
    for _ in 0..iterations {
        out = match stage {
            0 => { s.result = manifold_csg(s.segments); (s.result.indices.len() / 3) as u32 }
            1 => { s.result = boolmesh_csg(&s.cube, &s.sphere); (s.result.indices.len() / 3) as u32 }
            2 => weld(&s.result),
            3 => simplify(&s.result) as u32,
            4 => (physics(&s.result) * 1000.0) as u32,
            5 => {
                let m = manifold_csg(s.segments);
                weld(&m);
                simplify(&m);
                (physics(&m) * 1000.0) as u32
            }
            6 => (physics(&boolmesh_csg(&s.cube, &s.sphere)) * 1000.0) as u32,
            _ => 0,
        };
    }
    out
}

#[no_mangle]
pub extern "C" fn report() -> usize {
    let s = state();
    s.report = format!(
        "{{\"inputTris\":[{},{}],\"resultTris\":{}}}",
        s.cube.indices.len() / 3, s.sphere.indices.len() / 3, s.result.indices.len() / 3
    );
    s.report.len()
}
#[no_mangle]
pub extern "C" fn report_ptr() -> *const u8 { state().report.as_ptr() }
