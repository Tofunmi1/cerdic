//! A vertical sheet pinned along its top edge must hang below the pins, at the frame rates and settings a live `cloth.sim` uses.
#![expect(clippy::unwrap_used, reason = "fixtures")]
use ve_world::cloth::cloth_from_mesh;

fn hang(dt: f32, substeps: u32, bend: f32, seconds: f32) -> (f32, f32, f32) {
    let n = 10usize;
    let mut pos = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            pos.push([i as f32 / n as f32 - 0.5, 2.4 - j as f32 / n as f32, 0.0]);
        }
    }
    let mut tris = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let a = (j * (n + 1) + i) as u32;
            let (b, c, d) = (a + 1, a + (n + 1) as u32, a + (n + 1) as u32 + 1);
            tris.push([a, c, b]);
            tris.push([b, c, d]);
        }
    }
    let mut s = cloth_from_mesh(&pos, &tris, 0.3, 0.0, bend).unwrap();
    s.set_triangles(tris);
    s.settings.substeps = substeps;
    for i in 0..=n {
        s.pin(i);
    }
    for _ in 0..(seconds / dt) as usize {
        assert!(s.step(dt));
    }
    let top = s.pos.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    let bottom = s.pos.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let depth = s.pos.iter().map(|p| p[2].abs()).fold(0.0, f32::max);
    (top, bottom, depth)
}

#[test]
fn a_pinned_sheet_hangs_below_its_pins() {
    for (dt, sub) in [(1.0 / 60.0, 6), (1.0 / 30.0, 6), (1.0 / 60.0, 8)] {
        let (top, bottom, depth) = hang(dt, sub, 0.05, 5.0);
        eprintln!("dt {dt:.4} substeps {sub}: top {top:.3} bottom {bottom:.3} depth {depth:.3}");
        assert!(top <= 2.4 + 1e-3, "nothing rises above the pins: {top}");
        assert!(bottom < 1.6, "the sheet hangs down: {bottom}");
    }
}
