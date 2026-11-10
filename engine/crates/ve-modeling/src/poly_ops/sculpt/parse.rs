//! Parser for the serialised brush stroke language.

use ve_graph::Node;

use super::{Brush, MAX_STROKES, Stroke};
use crate::{Fail, poly_ops::errors::refuse};

const BRUSHES: &str = "draw, inflate, smooth, flatten, pinch, grab, mask, unmask";

pub(super) fn parse(node: &Node, source: &str) -> Result<Vec<Stroke>, Fail> {
    let mut strokes = Vec::new();
    for (index, entry) in source
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .enumerate()
    {
        let bad = |message: String| {
            refuse(
                node,
                "poly.sculpt_stroke",
                format!("stroke {index} (\"{entry}\"): {message}"),
            )
        };
        let words: Vec<&str> = entry.split_whitespace().collect();
        let brush = match words[0] {
            "draw" => Brush::Draw,
            "inflate" => Brush::Inflate,
            "smooth" => Brush::Smooth,
            "flatten" => Brush::Flatten,
            "pinch" => Brush::Pinch,
            "grab" => Brush::Grab,
            "mask" => Brush::Mask,
            "unmask" => Brush::Unmask,
            other => {
                return Err(bad(format!(
                    "`{other}` is not a brush; the brushes are {BRUSHES}"
                )));
            }
        };
        let wanted = if brush == Brush::Grab { 8 } else { 6 };
        if words.len() != wanted {
            let shape = if brush == Brush::Grab {
                "grab cx cy cz radius dx dy dz"
            } else {
                "<brush> cx cy cz radius strength"
            };
            return Err(bad(format!(
                "it has {} numbers after the brush name but needs {}: {shape}",
                words.len() - 1,
                wanted - 1
            )));
        }
        let mut values = [0.0_f32; 7];
        for (slot, word) in values.iter_mut().zip(&words[1..]) {
            *slot = word
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(|| bad(format!("`{word}` is not a number")))?;
        }
        let radius = values[3];
        if radius <= 0.0 {
            return Err(bad("the radius must be above 0".to_owned()));
        }
        let amount = if brush == Brush::Grab {
            [values[4], values[5], values[6]]
        } else {
            [values[4], 0.0, 0.0]
        };
        strokes.push(Stroke {
            brush,
            center: [values[0], values[1], values[2]],
            radius,
            amount,
        });
    }
    if strokes.is_empty() {
        return Err(refuse(
            node,
            "poly.sculpt_stroke",
            "there are no strokes; write one such as \"draw 0 1 0 0.3 0.05\"".to_owned(),
        ));
    }
    if strokes.len() > MAX_STROKES {
        return Err(refuse(
            node,
            "poly.sculpt_stroke",
            format!("{} strokes; the most is {MAX_STROKES}", strokes.len()),
        ));
    }
    Ok(strokes)
}
