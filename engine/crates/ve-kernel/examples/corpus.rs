//! Deterministic kernel corpus. `cargo xtask parity` runs this natively and as a wasm binary under
//! Node's WASI and requires identical output, which is how we know native and browser agree.
#![expect(
    clippy::print_stdout,
    reason = "the corpus reports through stdout for comparison"
)]

use ve_kernel::{KernelError, Mesh, Solid, simplify};

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

fn run() -> Result<(), KernelError> {
    let cube = Solid::cube([1.0, 1.0, 1.0], true)?;
    let sphere = Solid::sphere(0.65, 48)?;
    let shifted = cube.translate([0.5, 0.25, 0.0])?;
    report("cube", &cube)?;
    report("sphere", &sphere)?;
    report("cylinder", &Solid::cylinder(2.0, 0.5, 0.5, 64, false)?)?;
    report("cone", &Solid::cylinder(2.0, 0.5, 0.0, 64, false)?)?;
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
