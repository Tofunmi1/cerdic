//! River node specifications.
use super::*;

pub(super) fn carve() -> Option<OperatorSpec> {
    spec(
        "river.carve@1",
        "Cuts a river channel into a terrain.tile along a path: a bed below the ground, banks that blend back into the land, and the data the water needs (flow direction and speed, water height, channel and bank layers). The path is a list of world x,z points written as text, in the direction the water flows, for example path=\"10,64; 60,70; 118,64\"; give every tile of a world the same path and the river continues across tiles. Do erosion (terrain.erode) before this node, not after: erosion rebuilds the ground and would fill the channel. Follow it with river.water to draw the water.",
        vec![super::super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "path",
                ty: ParamType::Str { max_len: 4000 },
                default: Some(Value::Str(String::new())),
                doc: "The river's course as `x,z; x,z; ...` in world metres, downstream order, at least two points, for example \"0,64; 100,80; 256,64\". It is smoothed into a curve.",
            },
            ParamSpec {
                name: "level",
                ty: ParamType::Choice {
                    options: &["local", "descending"],
                },
                default: Some(Value::Str("local".into())),
                doc: "local follows the ground beside each cell and lets tiles of one world agree; descending makes water fall steadily along the whole path and stay level across the river, suited to terraced, ridged or mountainous ground and lakes in valleys (build it on the whole grid).",
            },
            number(
                "width",
                Unit::Length,
                8.0,
                0.5,
                500.0,
                "Width of the water and channel floor in metres: 3 a stream, 8 a river, 30 a great river.",
            ),
            number(
                "depth",
                Unit::Length,
                2.5,
                0.1,
                100.0,
                "How far the bed lies below the surrounding ground in metres.",
            ),
            number(
                "bank_slope",
                Unit::None,
                35.0,
                5.0,
                85.0,
                "Steepness of the banks in DEGREES: 20 gentle, 35 natural, 70 a gorge.",
            ),
            number(
                "bank_width",
                Unit::Length,
                6.0,
                0.0,
                200.0,
                "Extra width on each side over which the banks blend into the land.",
            ),
            number(
                "flatten",
                Unit::Ratio,
                0.3,
                0.0,
                1.0,
                "How much the valley floor is smoothed across the channel.",
            ),
            number(
                "water_offset",
                Unit::Length,
                0.4,
                0.0,
                20.0,
                "How far the water surface lies below the bank top in metres.",
            ),
        ],
    )
}

pub(super) fn water() -> Option<OperatorSpec> {
    spec(
        "river.water@1",
        "The water surface of the rivers carved into a terrain.tile by river.carve: a smooth sheet at the water height with flow direction, depth and shore foam carried per vertex, drawn with a flowing, depth-tinted, foamy water look that needs no texture. It fades to nothing where it meets the bank, so it has no hard edge. Connect the river.carve node (or terrain nodes after it) and give the result its own output.",
        vec![super::super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Poly),
        look(1.0),
    )
}

pub(super) fn fill() -> Option<OperatorSpec> {
    let mut params = vec![
        ParamSpec {
            name: "at",
            ty: ParamType::Vec2 {
                unit: Unit::Length,
                min: -super::super::EXTENT,
                max: super::super::EXTENT,
            },
            default: None,
            doc: "Seed world x,z in metres; omitted uses the input tile centre. Its cell must be below level.",
        },
        number(
            "level",
            Unit::Length,
            0.0,
            -10000.0,
            20000.0,
            "Water surface height in world metres.",
        ),
        number(
            "shore",
            Unit::Length,
            3.0,
            0.1,
            50.0,
            "Width of the wet bank in metres.",
        ),
    ];
    params.extend(look(0.0));
    params.extend(super::super::water::sim_parameters());
    spec(
        "water.fill@1",
        "Fills the connected hollow below level at a world x,z seed on a terrain.tile. Makes a flat open water sheet with depth tint and shore foam, a one-cell skirt and no side walls. Dig the hollow with terrain.flatten first; give the water its own output. A level above the rim floods to the tile border. Use this for lakes on a landscape.",
        vec![super::super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Poly),
        params,
    )
}

fn look(flow: f64) -> Vec<ParamSpec> {
    vec![
        number(
            "direction",
            Unit::Angle,
            0.0,
            -std::f64::consts::TAU,
            std::f64::consts::TAU,
            "Main wave travel direction in degrees; the per-vertex river current still drives streaming.",
        ),
        whole("seed", 0, 0, 1_000_000, "Repeatable wave pattern seed."),
        colour(
            "shallow",
            [0.16, 0.42, 0.45],
            "Colour of clear shallow water [r, g, b] (sRGB).",
        ),
        colour("deep", [0.02, 0.12, 0.2], "Colour of deep water."),
        number(
            "clarity",
            Unit::Length,
            3.0,
            0.2,
            50.0,
            "How many metres of depth it takes to go from the shallow to the deep colour: small is murky, large is clear.",
        ),
        number(
            "flow",
            Unit::Ratio,
            flow,
            0.0,
            4.0,
            "How fast the surface pattern streams along the current (1 is natural).",
        ),
        number(
            "ripple",
            Unit::Ratio,
            0.5,
            0.0,
            2.0,
            "Strength of the small waves and glints on the surface.",
        ),
        number(
            "foam",
            Unit::Ratio,
            0.5,
            0.0,
            1.0,
            "How much white foam forms at the banks and in fast water.",
        ),
        number(
            "opacity",
            Unit::Ratio,
            0.85,
            0.1,
            1.0,
            "How opaque the water is in its deepest parts.",
        ),
    ]
}
