use wasm_bindgen::prelude::*;

fn triangle(out: &mut Vec<f32>, a: [f32; 3], b: [f32; 3], c: [f32; 3]) {
    out.extend_from_slice(&a);
    out.extend_from_slice(&b);
    out.extend_from_slice(&c);
}

/// Unit primitives are authored in Rust; object transforms compose varied shapes.
#[wasm_bindgen]
pub fn box_vertices() -> Vec<f32> {
    let mut out = Vec::with_capacity(108);
    let p = [
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    for [a, b, c, d] in [
        [4, 5, 6, 7],
        [1, 0, 3, 2],
        [0, 4, 7, 3],
        [5, 1, 2, 6],
        [3, 7, 6, 2],
        [0, 1, 5, 4],
    ] {
        triangle(&mut out, p[a], p[b], p[c]);
        triangle(&mut out, p[a], p[c], p[d]);
    }
    out
}

#[wasm_bindgen]
pub fn plane_vertices() -> Vec<f32> {
    let mut out = Vec::new();
    triangle(
        &mut out,
        [-0.5, 0.0, -0.5],
        [-0.5, 0.0, 0.5],
        [0.5, 0.0, 0.5],
    );
    triangle(
        &mut out,
        [-0.5, 0.0, -0.5],
        [0.5, 0.0, 0.5],
        [0.5, 0.0, -0.5],
    );
    out
}

#[wasm_bindgen]
pub fn cylinder_vertices(segments: u32) -> Vec<f32> {
    let segments = segments.clamp(3, 128);
    let mut out = Vec::with_capacity(segments as usize * 36);
    for i in 0..segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let b = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let p = [a.cos() * 0.5, -0.5, a.sin() * 0.5];
        let q = [b.cos() * 0.5, -0.5, b.sin() * 0.5];
        let r = [a.cos() * 0.5, 0.5, a.sin() * 0.5];
        let s = [b.cos() * 0.5, 0.5, b.sin() * 0.5];
        triangle(&mut out, p, r, s);
        triangle(&mut out, p, s, q);
        triangle(&mut out, [0.0, 0.5, 0.0], s, r);
        triangle(&mut out, [0.0, -0.5, 0.0], p, q);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn factories_return_finite_triangles() {
        for vertices in [box_vertices(), plane_vertices(), cylinder_vertices(32)] {
            assert_eq!(vertices.len() % 9, 0);
            assert!(vertices.iter().all(|v| v.is_finite() && v.abs() <= 0.5));
        }
        assert_eq!(box_vertices().len(), 108);
        assert_eq!(plane_vertices().len(), 18);
        assert_eq!(cylinder_vertices(32).len(), 32 * 36);
    }
    #[test]
    fn cylinder_segments_are_bounded() {
        assert_eq!(cylinder_vertices(0).len(), 3 * 36);
        assert_eq!(cylinder_vertices(u32::MAX).len(), 128 * 36);
    }
    #[test]
    fn plane_faces_upward() {
        let v = plane_vertices();
        let ab = [v[3] - v[0], v[4] - v[1], v[5] - v[2]];
        let ac = [v[6] - v[0], v[7] - v[1], v[8] - v[2]];
        assert!(ab[2] * ac[0] - ab[0] * ac[2] > 0.0);
    }
}
