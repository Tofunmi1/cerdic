//! Bounded heap-size estimate for an expression tree.
use super::model::{Field, FieldExpr};

impl Field {
    /// Approximate heap bytes owned by this expression tree.
    #[must_use]
    pub fn estimated_bytes(&self) -> usize {
        expr_bytes(&self.expr)
    }
}

fn expr_bytes(expr: &FieldExpr) -> usize {
    let node = std::mem::size_of::<FieldExpr>();
    match expr {
        FieldExpr::Combine { a, b, .. } | FieldExpr::SmoothUnion { a, b, .. } => node
            .saturating_add(expr_bytes(a))
            .saturating_add(expr_bytes(b)),
        FieldExpr::Transform { child, .. }
        | FieldExpr::Repeat { child, .. }
        | FieldExpr::Offset { child, .. }
        | FieldExpr::Shell { child, .. }
        | FieldExpr::Noise { child, .. } => node.saturating_add(expr_bytes(child)),
        FieldExpr::Warp { child, .. } => node.saturating_add(expr_bytes(child)),
        FieldExpr::Sphere { .. }
        | FieldExpr::Ellipsoid { .. }
        | FieldExpr::Box { .. }
        | FieldExpr::Torus { .. }
        | FieldExpr::Capsule { .. }
        | FieldExpr::Cylinder { .. }
        | FieldExpr::Plane { .. } => node,
        FieldExpr::Tube { points, .. } => {
            node.saturating_add(points.len().saturating_mul(std::mem::size_of::<[f64; 3]>()))
        }
    }
}
