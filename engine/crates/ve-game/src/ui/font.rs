//! Procedural skeleton fonts: no font files. Each glyph is a few strokes (polylines in a grid four units wide, six tall for capitals,
//! baseline at zero) drawn with a round or square pen. The style's axes (weight, width, slant, spacing, pen, snap, wobble) change how the
//! same skeletons are drawn, so a handful of presets and any blend between them come from one table. Strokes are returned as
//! polylines, so the renderer draws crisp text at any size.
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// One polyline in glyph units (x right, y up).
pub(super) type Stroke = Vec<[f64; 2]>;

const CAP_HEIGHT: f64 = 6.0;

/// The skeletons. `;` separates strokes, spaces separate points, `x,y` is a point.
const SKELETONS: &[(char, &str)] = &[
    ('A', "0,0 2,6 4,0;0.7,2 3.3,2"),
    (
        'B',
        "0,0 0,6 2.6,6 3.6,5.2 3.6,4 2.6,3.1 0,3;2.6,3.1 3.8,2.2 3.8,1 2.8,0 0,0",
    ),
    ('C', "4,5 3,6 1,6 0,5 0,1 1,0 3,0 4,1"),
    ('D', "0,0 0,6 2.4,6 4,4.6 4,1.4 2.4,0 0,0"),
    ('E', "4,6 0,6 0,0 4,0;0,3 3,3"),
    ('F', "4,6 0,6 0,0;0,3 3,3"),
    ('G', "4,5 3,6 1,6 0,5 0,1 1,0 3,0 4,1 4,3 2.2,3"),
    ('H', "0,0 0,6;4,0 4,6;0,3 4,3"),
    ('I', "1,6 3,6;2,6 2,0;1,0 3,0"),
    ('J', "4,6 4,1 3,0 1,0 0,1"),
    ('K', "0,0 0,6;4,6 0,2.6;1.2,3.5 4,0"),
    ('L', "0,6 0,0 4,0"),
    ('M', "0,0 0,6 2,2.5 4,6 4,0"),
    ('N', "0,0 0,6 4,0 4,6"),
    ('O', "1,0 0,1 0,5 1,6 3,6 4,5 4,1 3,0 1,0"),
    ('P', "0,0 0,6 3,6 4,5 4,3.8 3,2.8 0,2.8"),
    ('Q', "1,0 0,1 0,5 1,6 3,6 4,5 4,1 3,0 1,0;2.4,1.8 4,0"),
    ('R', "0,0 0,6 3,6 4,5 4,3.8 3,2.8 0,2.8;1.8,2.8 4,0"),
    ('S', "4,5 3,6 1,6 0,5 0,4 1,3.2 3,2.8 4,2 4,1 3,0 1,0 0,1"),
    ('T', "0,6 4,6;2,6 2,0"),
    ('U', "0,6 0,1 1,0 3,0 4,1 4,6"),
    ('V', "0,6 2,0 4,6"),
    ('W', "0,6 1,0 2,3.5 3,0 4,6"),
    ('X', "0,6 4,0;4,6 0,0"),
    ('Y', "0,6 2,3 4,6;2,3 2,0"),
    ('Z', "0,6 4,6 0,0 4,0"),
    (
        'a',
        "3.4,4 3.4,0;3.4,2.9 2.4,4 1,4 0,3 0,1 1,0 2.4,0 3.4,1.1",
    ),
    ('b', "0,6 0,0;0,2.9 1,4 2.4,4 3.4,3 3.4,1 2.4,0 1,0 0,1.1"),
    ('c', "3.4,3.2 2.6,4 1,4 0,3 0,1 1,0 2.6,0 3.4,0.8"),
    (
        'd',
        "3.4,6 3.4,0;3.4,2.9 2.4,4 1,4 0,3 0,1 1,0 2.4,0 3.4,1.1",
    ),
    ('e', "0,2 3.4,2 3.4,3 2.4,4 1,4 0,3 0,1 1,0 2.6,0 3.4,0.8"),
    ('f', "3,5.4 2.4,6 1.6,6 1,5.2 1,0;0,4 2.6,4"),
    (
        'g',
        "3.4,4 3.4,-0.8 2.6,-2 1,-2 0.2,-1.4;3.4,2.9 2.4,4 1,4 0,3 0,1 1,0 2.4,0 3.4,1.1",
    ),
    ('h', "0,6 0,0;0,2.8 1,4 2.4,4 3.4,3 3.4,0"),
    ('i', "1.2,4 1.2,0;1.2,5.4 1.2,5.8"),
    ('j', "2,4 2,-1 1.4,-2 0.4,-2;2,5.4 2,5.8"),
    ('k', "0,6 0,0;3.2,4 0,1.6;1,2.4 3.4,0"),
    ('l', "1.2,6 1.2,0.6 1.8,0"),
    (
        'm',
        "0,4 0,0;0,3 0.8,4 1.8,4 2.6,3 2.6,0;2.6,3 3.4,4 4.4,4 5.2,3 5.2,0",
    ),
    ('n', "0,4 0,0;0,2.8 1,4 2.4,4 3.4,3 3.4,0"),
    ('o', "1,0 0,1 0,3 1,4 2.4,4 3.4,3 3.4,1 2.4,0 1,0"),
    ('p', "0,4 0,-2;0,2.9 1,4 2.4,4 3.4,3 3.4,1 2.4,0 1,0 0,1.1"),
    (
        'q',
        "3.4,4 3.4,-2;3.4,2.9 2.4,4 1,4 0,3 0,1 1,0 2.4,0 3.4,1.1",
    ),
    ('r', "0,4 0,0;0,2.6 1,3.8 2,4 3,3.8"),
    (
        's',
        "3.2,3.4 2.4,4 1,4 0,3.2 0.2,2.4 1.2,2 2.2,1.8 3.2,1 3.4,0.6 2.4,0 1,0 0,0.8",
    ),
    ('t', "1.2,5.6 1.2,0.8 1.8,0 2.6,0;0,4 2.6,4"),
    ('u', "0,4 0,1 1,0 2.4,0 3.4,1;3.4,4 3.4,0"),
    ('v', "0,4 1.7,0 3.4,4"),
    ('w', "0,4 1,0 2.1,3 3.2,0 4.2,4"),
    ('x', "0,4 3.4,0;3.4,4 0,0"),
    ('y', "0,4 1.7,0;3.4,4 1.2,-2 0.2,-2"),
    ('z', "0,4 3.4,4 0,0 3.4,0"),
    ('0', "1,0 0,1 0,5 1,6 2.6,6 3.6,5 3.6,1 2.6,0 1,0"),
    ('1', "0.6,4.6 2,6 2,0;0.8,0 3.2,0"),
    ('2', "0,5 1,6 2.6,6 3.6,5 3.6,3.8 0,0 3.8,0"),
    (
        '3',
        "0,5 1,6 2.6,6 3.6,5 3.6,4 2.6,3.2 1.2,3.2;2.6,3.2 3.6,2.4 3.6,1 2.6,0 1,0 0,1",
    ),
    ('4', "3,0 3,6 0,1.8 3.8,1.8"),
    (
        '5',
        "3.6,6 0.4,6 0,3.2 1.4,3.8 2.6,3.8 3.6,3 3.6,1 2.6,0 1,0 0,1",
    ),
    (
        '6',
        "3.4,5.4 2.6,6 1.2,6 0,4.8 0,1 1,0 2.6,0 3.6,1 3.6,2.6 2.6,3.4 1.2,3.4 0,2.6",
    ),
    ('7', "0,6 3.8,6 1.4,0"),
    (
        '8',
        "1,3.2 0.1,4 0.1,5 1,6 2.6,6 3.5,5 3.5,4 2.6,3.2 1,3.2 0,2.4 0,1 1,0 2.6,0 3.6,1 3.6,2.4 2.6,3.2",
    ),
    (
        '9',
        "0.2,0.6 1,0 2.4,0 3.6,1.2 3.6,5 2.6,6 1,6 0,5 0,3.6 1,2.8 2.4,2.8 3.6,3.6",
    ),
    ('!', "1,6 1,2;1,0.4 1,0.6"),
    ('"', "0.4,6 0.4,4.6;1.8,6 1.8,4.6"),
    ('#', "1,0 1.6,6;2.4,0 3,6;0,2 3.8,2;-0.2,4 3.6,4"),
    (
        '$',
        "3.4,4.8 2.4,5.6 1,5.6 0,4.6 0.2,3.6 1.6,3 2.6,2.6 3.6,1.6 3.4,0.6 2.4,0 1,0 0,1;1.8,6.6 1.8,-0.6",
    ),
    (
        '%',
        "0,0 3.8,6;0.2,5 0.2,6 1.2,6 1.2,5 0.2,5;2.6,1 2.6,0 3.6,0 3.6,1 2.6,1",
    ),
    (
        '&',
        "3.6,0 0.6,3.2 0.4,4.8 1.4,6 2.4,5 2.2,3.8 0.4,1.8 0.2,0.6 1.2,0 2.4,0.4 3.8,2.6",
    ),
    ('\'', "1,6 1,4.6"),
    ('(', "2,6.4 1,5 1,1 2,-0.4"),
    (')', "0,6.4 1,5 1,1 0,-0.4"),
    ('*', "1,5.8 1,3.4;0,5 2,4.2;0,4.2 2,5"),
    ('+', "0.4,3 3.2,3;1.8,1.6 1.8,4.4"),
    (',', "1,0.6 0.4,-1"),
    ('-', "0.4,2.6 2.8,2.6"),
    ('.', "1,0 1,0.2"),
    ('/', "0,-0.4 3,6.4"),
    (':', "1,4 1,4.2;1,0.8 1,1"),
    (';', "1,4 1,4.2;1,1 0.4,-0.6"),
    ('<', "3,5 0.4,3 3,1"),
    ('=', "0.4,4 3.2,4;0.4,2 3.2,2"),
    ('>', "0.4,5 3,3 0.4,1"),
    ('?', "0,5 1,6 2.6,6 3.6,5 3.6,4 2,3 1.8,2;1.8,0.4 1.8,0.6"),
    (
        '@',
        "4,1.6 4,4.6 3,6 1,6 0,5 0,1 1,0 3,0;3.1,3.2 2.6,3.9 1.8,3.9 1.5,3.2 1.8,2.2 2.6,2.2 3.1,3.2 3.1,2 3.6,2",
    ),
    ('[', "2,6.4 0.6,6.4 0.6,-0.4 2,-0.4"),
    ('\\', "0,6.4 3,-0.4"),
    (']', "0,6.4 1.4,6.4 1.4,-0.4 0,-0.4"),
    ('^', "0.2,3.8 1.8,6 3.4,3.8"),
    ('_', "0,-0.8 3.6,-0.8"),
    ('`', "0.4,6 1.4,4.8"),
    (
        '{',
        "2,6.4 1.2,6.2 1,5.4 1,3.6 0.2,3 1,2.4 1,0.6 1.2,-0.2 2,-0.4",
    ),
    ('|', "1,6.4 1,-0.4"),
    (
        '}',
        "0,6.4 0.8,6.2 1,5.4 1,3.6 1.8,3 1,2.4 1,0.6 0.8,-0.2 0,-0.4",
    ),
    ('~', "0,2.6 1,3.6 2.6,2.4 3.6,3.4"),
];

pub(super) struct Glyph {
    pub strokes: Vec<Stroke>,
    pub advance: f64,
}

fn glyphs() -> &'static BTreeMap<char, Glyph> {
    static TABLE: OnceLock<BTreeMap<char, Glyph>> = OnceLock::new();
    TABLE.get_or_init(|| {
        SKELETONS
            .iter()
            .map(|(ch, spec)| {
                let strokes: Vec<Stroke> = spec
                    .split(';')
                    .map(|stroke| {
                        stroke
                            .split_whitespace()
                            .filter_map(|point| {
                                let (x, y) = point.split_once(',')?;
                                Some([x.parse().ok()?, y.parse().ok()?])
                            })
                            .collect()
                    })
                    .collect();
                let width = strokes
                    .iter()
                    .flatten()
                    .map(|p| p[0])
                    .fold(0.0_f64, f64::max);
                (
                    *ch,
                    Glyph {
                        strokes,
                        advance: width.max(1.6),
                    },
                )
            })
            .collect()
    })
}

/// How the skeletons are drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Style {
    /// Pen thickness, 0 (hairline) to 1 (heavy).
    pub weight: f64,
    /// Horizontal stretch, around 1.
    pub width: f64,
    /// Shear, -0.5 to 0.5.
    pub slant: f64,
    /// Extra space between glyphs, in glyph units.
    pub spacing: f64,
    /// Round pen ends and joins (else square).
    pub round: bool,
    /// Snap points to a grid of this many glyph units (0 for none): the pixel look.
    pub snap: f64,
    /// Deterministic wobble, in glyph units (the hand-drawn look).
    pub wobble: f64,
}

impl Style {
    pub(super) fn preset(name: &str) -> Option<Self> {
        let base = Self {
            weight: 0.35,
            width: 1.0,
            slant: 0.0,
            spacing: 1.3,
            round: true,
            snap: 0.0,
            wobble: 0.0,
        };
        Some(match name {
            "sans" | "" => base,
            "rounded" => Self {
                weight: 0.6,
                spacing: 1.4,
                ..base
            },
            "techno" => Self {
                weight: 0.4,
                width: 1.15,
                round: false,
                spacing: 1.2,
                ..base
            },
            "pixel" => Self {
                weight: 0.55,
                round: false,
                snap: 1.0,
                spacing: 1.0,
                ..base
            },
            "hand" => Self {
                weight: 0.4,
                slant: 0.15,
                wobble: 0.18,
                ..base
            },
            "bold" => Self {
                weight: 0.85,
                spacing: 1.5,
                ..base
            },
            "italic" => Self {
                slant: 0.25,
                ..base
            },
            _ => return None,
        })
    }
}

/// Text laid out from the top-left corner (y down), `size` being the capital height: the polylines, the width and the pen thickness.
pub(super) struct Laid {
    pub paths: Vec<Vec<[f64; 2]>>,
    pub width: f64,
    pub pen: f64,
}

pub(super) fn layout(text: &str, size: f64, style: &Style) -> Laid {
    let scale = size / CAP_HEIGHT;
    let mut cursor = 0.0;
    let mut paths = Vec::new();
    for (index, ch) in text.chars().enumerate() {
        let Some(glyph) = (if ch == ' ' {
            None
        } else {
            glyphs().get(&ch).or_else(|| glyphs().get(&'?'))
        }) else {
            cursor += (2.6 * style.width + style.spacing) * scale;
            continue;
        };
        for stroke in &glyph.strokes {
            let path = stroke
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let (mut x, mut y) = (p[0], p[1]);
                    if style.snap > 0.0 {
                        x = (x / style.snap).round() * style.snap;
                        y = (y / style.snap).round() * style.snap;
                    }
                    if style.wobble > 0.0 {
                        let h = (index * 31 + i * 17 + usize::from(ch as u8)) % 97;
                        x += ((h % 11) as f64 / 5.0 - 1.0) * style.wobble;
                        y += (((h / 11) % 9) as f64 / 4.0 - 1.0) * style.wobble;
                    }
                    [
                        cursor + (x * style.width + style.slant * y) * scale,
                        (CAP_HEIGHT - y) * scale,
                    ]
                })
                .collect();
            paths.push(path);
        }
        cursor += (glyph.advance * style.width + style.spacing) * scale;
    }
    let width = (cursor - style.spacing * scale).max(0.0);
    Laid {
        paths,
        width,
        pen: size * (0.07 + 0.16 * style.weight),
    }
}
