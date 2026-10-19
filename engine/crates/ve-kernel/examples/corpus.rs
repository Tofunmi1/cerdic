//! Deterministic kernel corpus. `cargo xtask parity` runs this natively and as a wasm binary under
//! Node's WASI and requires identical output, which is how we know native and browser agree.
#![expect(
    clippy::print_stdout,
    reason = "the corpus reports through stdout for comparison"
)]

use ve_kernel::{
    BevelOffset, BevelOptions, KernelError, Mesh, PolygonMesh, Profile, Solid, UnwrapOptions,
    UvMethod, bevel_edges, simplify, unwrap_uvs,
};

fn profiles() -> Result<(), KernelError> {
    let rectangle = Profile::polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 3.0], [1.0, 3.0]])?;
    report(
        "revolve-full",
        &rectangle.revolve(64, std::f64::consts::TAU)?,
    )?;
    report(
        "revolve-half",
        &rectangle.revolve(64, std::f64::consts::PI)?,
    )?;
    let square = Profile::polygon(&[[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]])?;
    report(
        "extrude-twisted",
        &square.extrude(2.0, 128, std::f64::consts::FRAC_PI_2, [1.0; 2])?,
    )?;
    let offset = square.offset(-0.1)?;
    println!(
        "profile-offset area={:.6} loops={:?}",
        offset.area(),
        offset.loops()?
    );
    let rounded = square.offset_round(0.1)?;
    println!(
        "profile-offset-round area={:.6} points={}",
        rounded.area(),
        rounded.loops()?.iter().map(Vec::len).sum::<usize>()
    );
    let rim = square.difference(&offset)?;
    report("extrude-with-hole", &rim.extrude(0.1, 1, 0.0, [1.0; 2])?)?;
    Ok(())
}

fn report(name: &str, solid: &Solid) -> Result<(), KernelError> {
    let size = solid.bounds()?.size();
    println!(
        "{name} tris={} verts={} parts={} volume={:.4} area={:.4} size={:.4},{:.4},{:.4}",
        solid.triangle_count(),
        solid.vertex_count(),
        solid.component_count(),
        solid.volume(),
        solid.surface_area(),
        size[0],
        size[1],
        size[2],
    );
    Ok(())
}

/// `MikkTSpace` tangents for a carved mesh, hashed over every output bit so parity covers all of it.
/// Normals and UVs use only exactly rounded operations (`sqrt`, `+ - * /`), so the inputs match everywhere.
fn tangent_case(mesh: &Mesh) -> Result<(), KernelError> {
    let normals: Vec<[f32; 3]> = mesh
        .positions
        .iter()
        .map(|p| {
            let length = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1.0e-6);
            [p[0] / length, p[1] / length, p[2] / length]
        })
        .collect();
    let uvs: Vec<[f32; 2]> = mesh
        .positions
        .iter()
        .map(|p| [p[0] + 0.5 * p[2], p[1] - 0.25 * p[2]])
        .collect();
    let result = ve_kernel::tangents(&mesh.positions, &normals, &uvs, &mesh.indices)?;
    let hash = result
        .iter()
        .flatten()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, value| {
            (hash ^ u64::from(value.to_bits())).wrapping_mul(0x0100_0000_01b3)
        });
    println!("tangents corners={} hash={hash:016x}", result.len());
    Ok(())
}

/// Blender's bmesh bevel on a box: every edge, one edge, a loop. Positions and corners are hashed bit
/// for bit, so parity also proves the deterministic maths and the vendored Blender code agree.
fn bevel_cases() -> Result<(), KernelError> {
    let positions = [
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    let corners = [
        0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
    ];
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &[4; 6],
        corners: &corners,
    };
    let all: [[u32; 2]; 12] = [
        [0, 3],
        [3, 2],
        [2, 1],
        [1, 0],
        [4, 5],
        [5, 6],
        [6, 7],
        [7, 4],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];
    let cases: [(&str, &[[u32; 2]], u32, f32); 4] = [
        ("bevel-all-3", &all, 3, 0.2),
        ("bevel-all-1", &all, 1, 0.15),
        ("bevel-top-loop", &[[2, 3], [6, 7], [2, 6], [3, 7]], 3, 0.1),
        ("bevel-one-edge", &[[4, 5]], 3, 0.25),
    ];
    for (name, edges, segments, width) in cases {
        let options = BevelOptions {
            width,
            segments,
            offset_type: BevelOffset::Offset,
            profile: 0.5,
        };
        let result = bevel_edges(&mesh, edges, &options)?;
        let hash = result
            .positions
            .iter()
            .flatten()
            .map(|value| u64::from(value.to_bits()))
            .chain(result.corners.iter().map(|&c| u64::from(c)))
            .chain(result.face_parent.iter().map(|&p| u64::from(p)))
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, value| {
                (hash ^ value).wrapping_mul(0x0100_0000_01b3)
            });
        println!(
            "{name} vertices={} faces={} hash={hash:016x}",
            result.positions.len(),
            result.face_sizes.len()
        );
    }
    Ok(())
}

/// A point on the unit circle from rational parametrisation only (`+ - * /`), so every platform builds
/// the same input bits: `t = tan(angle / 2)` is taken as `k / (half - k)`.
const CUBE_POSITIONS: [[f32; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];
const CUBE_CORNERS: [u32; 24] = [
    0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
];

fn ratio(numerator: u32, denominator: u32) -> f32 {
    f32::from(u16::try_from(numerator).unwrap_or(u16::MAX))
        / f32::from(u16::try_from(denominator).unwrap_or(u16::MAX))
}

fn circle_point(k: u32, count: u32) -> (f32, f32) {
    let half = count / 2;
    let j = if k <= half { k } else { count - k };
    let (cos, sin) = if j == half {
        (-1.0, 0.0)
    } else {
        let t = ratio(j, half - j);
        ((1.0 - t * t) / (1.0 + t * t), 2.0 * t / (1.0 + t * t))
    };
    (cos, if k > half { -sin } else { sin })
}

/// A UV sphere (pole to pole) with one meridian seam, as positions, faces and seam edges.
fn uv_sphere(rings: u32, segments: u32) -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>, Vec<[u32; 2]>) {
    let ring = |r: u32, s: u32| 1 + (r - 1) * segments + (s % segments);
    let mut positions = vec![[0.0, 1.0, 0.0]];
    for r in 1..rings {
        let w = ratio(r, rings - r);
        let (cos_phi, sin_phi) = ((1.0 - w * w) / (1.0 + w * w), 2.0 * w / (1.0 + w * w));
        for s in 0..segments {
            let (c, sn) = circle_point(s, segments);
            positions.push([sin_phi * c, cos_phi, sin_phi * sn]);
        }
    }
    positions.push([0.0, -1.0, 0.0]);
    let south = u32::try_from(positions.len() - 1).unwrap_or(0);
    let (mut sizes, mut corners) = (Vec::new(), Vec::new());
    let mut face = |vertices: &[u32]| {
        sizes.push(u32::try_from(vertices.len()).unwrap_or(0));
        corners.extend_from_slice(vertices);
    };
    for s in 0..segments {
        face(&[0, ring(1, s + 1), ring(1, s)]);
    }
    for r in 1..rings - 1 {
        for s in 0..segments {
            face(&[
                ring(r, s),
                ring(r, s + 1),
                ring(r + 1, s + 1),
                ring(r + 1, s),
            ]);
        }
    }
    for s in 0..segments {
        face(&[south, ring(rings - 1, s), ring(rings - 1, s + 1)]);
    }
    let mut seams = vec![[0, ring(1, 0)]];
    seams.extend((1..rings - 1).map(|r| [ring(r, 0), ring(r + 1, 0)]));
    seams.push([ring(rings - 1, 0), south]);
    (positions, sizes, corners, seams)
}

/// Blender's UV parametrizer and packer: a cube net, six packed cube islands, and a sphere with both
/// methods (rotation included). Every UV bit is hashed, which also exercises the deterministic maths.
fn uv_cases() -> Result<(), KernelError> {
    let cube = PolygonMesh {
        positions: &CUBE_POSITIONS,
        face_sizes: &[4; 6],
        corners: &CUBE_CORNERS,
    };
    let cross = [[0, 4], [1, 5], [2, 6], [3, 7], [4, 5], [5, 6], [4, 7]];
    let all = [
        [0, 3],
        [3, 2],
        [2, 1],
        [1, 0],
        [4, 5],
        [5, 6],
        [6, 7],
        [7, 4],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];
    let (positions, sizes, corners, seams) = uv_sphere(8, 12);
    let sphere = PolygonMesh {
        positions: &positions,
        face_sizes: &sizes,
        corners: &corners,
    };
    let make = |method, pack, rotate| UnwrapOptions {
        method,
        pack,
        rotate,
        margin: 0.01,
    };
    let cases = [
        (
            "uv-cube-cross",
            &cube,
            &cross[..],
            make(UvMethod::Conformal, false, false),
        ),
        (
            "uv-cube-six-packed",
            &cube,
            &all[..],
            make(UvMethod::Conformal, true, true),
        ),
        (
            "uv-sphere-lscm",
            &sphere,
            &seams[..],
            make(UvMethod::Conformal, true, true),
        ),
        (
            "uv-sphere-abf",
            &sphere,
            &seams[..],
            make(UvMethod::AngleBased, true, false),
        ),
    ];
    for (name, mesh, seam_edges, options) in cases {
        let layout = unwrap_uvs(mesh, seam_edges, &options)?;
        let hash = layout
            .uvs
            .iter()
            .flatten()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, value| {
                (hash ^ u64::from(value.to_bits())).wrapping_mul(0x0100_0000_01b3)
            });
        println!(
            "{name} corners={} failed={} hash={hash:016x}",
            layout.uvs.len(),
            layout.failed_islands
        );
    }
    Ok(())
}

fn run() -> Result<(), KernelError> {
    bevel_cases()?;
    uv_cases()?;
    report(
        "rigid-transform",
        &Solid::cube([2.0, 1.0, 3.0], true)?
            .transform([0.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 1.0, 3.0])?,
    )?;
    profiles()?;
    let cube = Solid::cube([1.0, 1.0, 1.0], true)?;
    let sphere = Solid::sphere(0.65, 48)?;
    let shifted = cube.translate([0.5, 0.25, 0.0])?;
    report("cube", &cube)?;
    report("sphere", &sphere)?;
    report("cylinder", &Solid::cylinder(2.0, 0.5, 0.5, 64, false)?)?;
    report("cone", &Solid::cylinder(2.0, 0.5, 0.0, 64, false)?)?;
    report("round", &cube.round(0.15, 12)?)?;
    report("chamfer", &cube.chamfer(0.15)?)?;
    let diff = cube.difference(&sphere)?;
    report("cube-sphere", &diff)?;
    report("union", &cube.union(&shifted)?)?;
    report("intersect", &cube.intersect(&shifted)?)?;
    let context = ve_kernel::ExecutionContext::new()?;
    report(
        "context-union",
        &Solid::union_all_with_context(&[&cube, &shifted], &context)?,
    )?;
    context.cancel();
    if matches!(
        cube.union_with_context(&shifted, &context),
        Err(KernelError::Cancelled)
    ) {
        println!("context-cancelled=true");
    } else {
        return Err(KernelError::InvalidArgument("cancellation parity failed"));
    }
    report(
        "rotated",
        &cube.scale([1.0, 2.0, 3.0])?.rotate([10.0, 20.0, 30.0])?,
    )?;
    let mesh = diff.to_mesh()?;
    report("round-trip", &Solid::from_mesh(&mesh)?)?;
    let simple = simplify(&mesh, 0.5, 0.05)?;
    println!("simplify tris={}", simple.triangle_count());
    tangent_case(&mesh)?;
    let open = Mesh {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        indices: vec![0, 1, 2],
    };
    match Solid::from_mesh(&open) {
        Ok(_) => println!("open-mesh unexpectedly built"),
        Err(error) => println!("open-mesh error={error}"),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        println!("corpus failed: {error}");
        std::process::exit(1);
    }
}
