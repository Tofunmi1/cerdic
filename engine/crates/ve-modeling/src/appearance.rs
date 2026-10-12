//! `solid.material`: the shape passes through unchanged and carries a surface.
use crate::{Fail, Material, cache::Value, fail};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};

fn triple(node: &Node, name: &str) -> Option<[f32; 3]> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return None;
    };
    let mut out = [0.0_f32; 3];
    for (slot, item) in out.iter_mut().zip(items) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "colour channels are 0..=100"
        )]
        {
            *slot = item.as_f64()? as f32;
        }
    }
    (items.len() == 3).then_some(out)
}

fn scalar(node: &Node, name: &str) -> Option<f32> {
    #[expect(clippy::cast_possible_truncation, reason = "factors are 0..=1")]
    node.params.get(name)?.as_f64().map(|v| v as f32)
}

pub(crate) fn material(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "solid.material needs a solid"))?;
    let (Some(color), Some(metallic), Some(roughness), Some(emissive)) = (
        triple(node, "color"),
        scalar(node, "metallic"),
        scalar(node, "roughness"),
        triple(node, "emissive"),
    ) else {
        return Err(fail(
            "kernel.invalid_input",
            "solid.material needs color, metallic, roughness and emissive",
        ));
    };
    Ok(Value {
        mesh: input.mesh.clone(),
        volume: input.volume,
        loops: Vec::new(),
        material: Some(Material {
            color,
            metallic,
            roughness,
            emissive,
        }),
    })
}
