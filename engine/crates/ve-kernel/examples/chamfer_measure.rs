//! B6 chamfer cost measurement: a 1 m box across widths, then a 76-triangle stool union.
#![expect(
    clippy::expect_used,
    clippy::print_stdout,
    reason = "a measurement example that reports through stdout and stops at its first failure"
)]

use std::time::Instant;

use ve_kernel::Solid;

fn main() {
    let box_solid = Solid::cube([1.0, 1.0, 1.0], true).expect("box");
    for width in [0.02, 0.05, 0.1] {
        let start = Instant::now();
        let cut = box_solid.chamfer(width).expect("chamfer");
        println!(
            "box width={width:.2} tris={} volume={:.6} ms={:.2}",
            cut.triangle_count(),
            cut.volume(),
            start.elapsed().as_secs_f64() * 1.0e3
        );
    }
    let seat = Solid::cube([0.44, 0.06, 0.36], true)
        .expect("seat")
        .translate([0.0, 0.45, 0.0])
        .expect("seat up");
    let leg = Solid::cube([0.045, 0.45, 0.045], true).expect("leg");
    let mut stool = seat;
    for (x, z) in [(-0.18, -0.14), (-0.18, 0.14), (0.18, -0.14), (0.18, 0.14)] {
        let placed = leg.translate([x, 0.225, z]).expect("leg");
        stool = stool.union(&placed).expect("union");
    }
    println!(
        "stool sharp tris={} volume={:.6}",
        stool.triangle_count(),
        stool.volume()
    );
    let start = Instant::now();
    let cut = stool.chamfer(0.01).expect("chamfer stool");
    println!(
        "stool width=0.01 tris={} volume={:.6} ms={:.2}",
        cut.triangle_count(),
        cut.volume(),
        start.elapsed().as_secs_f64() * 1.0e3
    );
}
