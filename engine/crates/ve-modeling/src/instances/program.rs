//! `inst.program`: copies placed by programs of the copy number: one for where, one for the size, one for the turn about Y.

use glam::{DMat4, DVec3};
use ve_graph::{Node, Value as Param};
use ve_world::expr::{Bindings, Sample, Type, Value as Sampled};

use crate::{Fail, fail, poly_ops::compile_param};

pub(super) fn placements(node: &Node) -> Result<Vec<DMat4>, Fail> {
    let count = match node.params.get("count") {
        Some(Param::Int(n)) => usize::try_from(*n).unwrap_or(0),
        _ => 0,
    };
    let bindings = Bindings {
        fields: vec![
            ("i".to_owned(), Type::Float),
            ("count".to_owned(), Type::Float),
            ("f".to_owned(), Type::Float),
        ],
    };
    let place = compile_param(node, "code", &bindings, Type::Vec3)?;
    let size = compile_param(node, "scale_code", &bindings, Type::Float)?;
    let turn = compile_param(node, "yaw_code", &bindings, Type::Float)?;
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        #[expect(clippy::cast_precision_loss, reason = "copy numbers are at most 16384")]
        let (i, n) = (k as f32, count as f32);
        let f = if count > 1 { i / (n - 1.0) } else { 0.0 };
        let bound = [Sampled::Float(i), Sampled::Float(n), Sampled::Float(f)];
        let sample = Sample {
            p: [i, f, 0.0],
            bound: &bound,
            ..Sample::default()
        };
        let (Sampled::Vec3(at), Sampled::Float(scale), Sampled::Float(yaw)) =
            (place.eval(&sample), size.eval(&sample), turn.eval(&sample))
        else {
            return Err(fail(
                "expr.error",
                "the programs gave the wrong kinds of value",
            ));
        };
        if scale <= 0.0 {
            return Err(fail(
                "expr.error",
                format!("scale_code gave {scale} for copy {k}; sizes must be above 0"),
            ));
        }
        out.push(
            DMat4::from_translation(DVec3::new(
                f64::from(at[0]),
                f64::from(at[1]),
                f64::from(at[2]),
            )) * DMat4::from_rotation_y(f64::from(yaw).to_radians())
                * DMat4::from_scale(DVec3::splat(f64::from(scale))),
        );
    }
    Ok(out)
}
