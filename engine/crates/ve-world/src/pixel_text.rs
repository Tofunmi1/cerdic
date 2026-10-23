//! Deterministic layout of text as small rectangles.
use crate::pixel_font::glyph;

/// Pixel rectangles and the text extent, in metres.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Lit pixels as `[x, y, width, height]` rectangles.
    pub rects: Vec<[f32; 4]>,
    /// Width of the text in metres.
    pub width: f32,
    /// Height of the text in metres.
    pub height: f32,
}

/// Turn printable text into 5 by 7 pixel glyph rectangles.
#[must_use]
pub fn layout(text: &str, pixel: f32) -> Layout {
    if text.is_empty() {
        return Layout {
            rects: Vec::new(),
            width: 0.0,
            height: 0.0,
        };
    }
    let advance = pixel * 6.0;
    let mut width = -pixel;
    for _ in text.chars() {
        width += advance;
    }
    let mut rects = Vec::new();
    let mut x0 = 0.0;
    for ch in text.chars() {
        let bitmap = glyph(ch);
        for y in 0_u8..7 {
            let row = bitmap[usize::from(y)];
            for x in 0_u8..5 {
                if row & (1_u8 << (4 - x)) != 0 {
                    rects.push([
                        x0 + f32::from(x) * pixel,
                        f32::from(y) * pixel,
                        pixel,
                        pixel,
                    ]);
                }
            }
        }
        x0 += advance;
    }
    Layout {
        rects,
        width,
        height: pixel * 7.0,
    }
}
