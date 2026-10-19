//! Release-only `LevelSet` benchmark: `field_bench [axis_samples] [tape_instructions]`.
use std::time::Instant;
use ve_kernel::{ExecutionContext, FieldInstruction, FieldOp, Solid};

#[expect(
    clippy::print_stdout,
    reason = "benchmark output is captured as measurement evidence"
)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let samples = args.get(1).map_or(Ok(100_usize), |v| v.parse())?;
    let tape_count = args.get(2).map_or(Ok(1_usize), |v| v.parse())?;
    if samples < 2 || tape_count == 0 {
        return Err("samples must be at least 2 and tape length must be nonzero".into());
    }
    let mut tape = vec![FieldInstruction::leaf(
        FieldOp::Sphere,
        [0.0, 0.0, 0.0, 0.75],
    )];
    for _ in 1..tape_count {
        let previous = u32::try_from(tape.len() - 1)?;
        tape.push(FieldInstruction::unary(
            FieldOp::Offset,
            previous,
            [0.000_001, 0.0, 0.0, 0.0],
        ));
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "sample count is bounded by the two-million-cell field cap"
    )]
    let intervals = (samples - 1) as f64;
    let edge = 2.0 / intervals;
    let context = ExecutionContext::new()?;
    let start = Instant::now();
    let solid = Solid::from_field(&tape, [[-1.0; 3], [1.0; 3]], edge, &context)?;
    println!(
        "axis_samples={samples} cells={} tape={tape_count} elapsed_ms={} verts={} tris={} volume={:.17}",
        samples.pow(3),
        start.elapsed().as_millis(),
        solid.to_mesh()?.positions.len(),
        solid.triangle_count(),
        solid.volume()
    );
    Ok(())
}
