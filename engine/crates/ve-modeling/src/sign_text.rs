//! `sign.text`: text in the 5 by 7 pixel font as small boxes (neon and LED signs), centred on x = 0, standing on y = 0, facing +z, with a
//! dark housing and an emissive glow in the sign's colour.

use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, KernelError, Solid};
use ve_world::pixel_text::layout;

use crate::detail_panels::{cuboid, union_many};
use crate::{Fail, Material, MaterialRecipe, fail};

fn number(node: &Node, name: &str) -> Option<f64> {
    node.params.get(name).and_then(Param::as_f64)
}

fn text(node: &Node) -> &str {
    match node.params.get("text") {
        Some(Param::Str(text)) => text.as_str(),
        _ => "",
    }
}

/// The lit rectangles of the sign in sign space (`[x0, y0, x1, y1]` in metres, centred on x, standing on y = 0), with horizontal runs of
/// neighbouring pixels merged into one box.
fn runs(node: &Node) -> Result<(Vec<[f64; 4]>, f64), KernelError> {
    let height = number(node, "height").ok_or(KernelError::InvalidArgument("height is missing"))?;
    #[expect(clippy::cast_possible_truncation, reason = "a pixel size in metres")]
    let pixel = (height / 7.0) as f32;
    let words = layout(text(node), pixel);
    if words.rects.is_empty() {
        return Err(KernelError::InvalidArgument("the text has no lit pixel: write at least one visible character"));
    }
    let pixel = f64::from(pixel);
    let mut rects: Vec<[f64; 4]> = words
        .rects
        .iter()
        .map(|r| {
            // `layout` puts row 0 at the top of the glyph: flip so the text rises from y = 0
            let (x, w, h) = (f64::from(r[0]), f64::from(r[2]), f64::from(r[3]));
            let top = f64::from(words.height) - f64::from(r[1]);
            [x, top - h, x + w, top]
        })
        .collect();
    rects.sort_by(|a, b| a[1].total_cmp(&b[1]).then(a[0].total_cmp(&b[0])));
    let mut merged: Vec<[f64; 4]> = Vec::new();
    for rect in rects {
        match merged.last_mut() {
            Some(last) if (last[1] - rect[1]).abs() < pixel * 1e-3 && (last[2] - rect[0]).abs() < pixel * 1e-3 => last[2] = rect[2],
            _ => merged.push(rect),
        }
    }
    let half = f64::from(words.width) * 0.5;
    for r in &mut merged {
        r[0] -= half;
        r[2] -= half;
    }
    Ok((merged, half))
}

/// The sign's letters as one solid.
pub(crate) fn build(node: &Node, context: &ExecutionContext) -> Result<Solid, KernelError> {
    let depth = number(node, "depth").ok_or(KernelError::InvalidArgument("depth is missing"))?;
    let (rects, _) = runs(node)?;
    let boxes: Result<Vec<Solid>, KernelError> = rects.iter().map(|r| cuboid([r[0], r[1], 0.0], [r[2], r[3], depth])).collect();
    union_many(boxes?, context)
}

/// The sign's surface: a dark housing in its colour and an emissive glow of that colour.
pub(crate) fn material(node: &Node) -> Result<Material, Fail> {
    let colour = match node.params.get("color") {
        Some(Param::List(items)) if items.len() == 3 => {
            let mut out = [0.0_f32; 3];
            for (slot, item) in out.iter_mut().zip(items) {
                #[expect(clippy::cast_possible_truncation, reason = "colour channels are 0..=100")]
                {
                    *slot = item.as_f64().ok_or_else(|| fail("param.type", "color must be three numbers"))? as f32;
                }
            }
            out
        }
        _ => return Err(fail("param.type", "color must be three numbers")),
    };
    #[expect(clippy::cast_possible_truncation, reason = "a glow factor of at most 20")]
    let glow = number(node, "glow").ok_or_else(|| fail("param.type", "glow must be a number"))? as f32;
    Ok(Material {
        color: colour.map(|c| c * 0.15),
        opacity: 1.0,
        metallic: 0.0,
        roughness: 0.4,
        emissive: colour.map(|c| c * glow),
        emits_light: false,
        recipe: MaterialRecipe::None,
        texture_scale: 1.0,
        texture_tile: 0.0,
        no_repeat: false,
        no_repeat_scale: 1.0,
        texture_size: 0,
        toon: None,
        texture_program: None,
        texture_style: None,
        foliage: None,
        finish: None,
    })
}
