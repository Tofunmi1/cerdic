//! Terrain node specifications.
use super::*;

pub(super) fn tile() -> Option<OperatorSpec> {
    spec(
        "terrain.tile@1",
        "One tile of a world's ground as an editable polygon mesh with a height grid behind it. Heights come from fractal gradient noise evaluated at WORLD coordinates, so neighbouring tiles (tile_x, tile_z) line up exactly and the same seed always gives the same land, whichever tile you build first. Connect the result to terrain.mask or terrain.erode, to solid.material, or to poly.* nodes. For one small landscape use the default tile; for a big world build a grid of tiles with the same seed and settings.",
        vec![],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "size",
                ty: ParamType::Vec2 {
                    unit: Unit::Length,
                    min: 8.0,
                    max: 4096.0,
                },
                default: Some(Value::List(vec![Value::Num(256.0), Value::Num(256.0)])),
                doc: "Width along X and depth along Z of the tile in metres.",
            },
            number(
                "cell",
                Unit::Length,
                2.0,
                0.25,
                32.0,
                "Metres between grid points. size / cell must stay under 2048 cells a side; 2 m suits a 256 m tile, 0.5 m a close-up 64 m tile.",
            ),
            whole(
                "tile_x",
                0,
                -1000,
                1000,
                "Which tile along X. The tile covers tile_x * size.x to (tile_x + 1) * size.x.",
            ),
            whole("tile_z", 0, -1000, 1000, "Which tile along Z."),
            whole(
                "seed",
                0,
                0,
                1_000_000,
                "World seed: the same seed gives the same land on every tile.",
            ),
            number(
                "height",
                Unit::Length,
                120.0,
                0.0,
                3000.0,
                "Tallest ground above y=0 in metres.",
            ),
            number(
                "scale",
                Unit::Length,
                200.0,
                4.0,
                20000.0,
                "Size of the biggest hills in metres; smaller gives more, tighter hills.",
            ),
            whole(
                "octaves",
                5,
                1,
                8,
                "Layers of finer detail (1 is smooth, 5 to 8 is craggy).",
            ),
            number(
                "roughness",
                Unit::Ratio,
                0.5,
                0.1,
                0.9,
                "How strong each finer layer is relative to the one before.",
            ),
            ParamSpec {
                name: "style",
                ty: ParamType::Choice {
                    options: &["hills", "ridged"],
                },
                default: Some(Value::Str("hills".to_owned())),
                doc: "hills: soft rolling ground. ridged: sharp mountain ridges.",
            },
            ParamSpec {
                name: "basis",
                ty: ParamType::Choice {
                    options: &["classic", "perlin", "simplex", "value", "cellular"],
                },
                default: Some(Value::Str("classic".to_owned())),
                doc: "The noise under the land. classic: the original (default, existing worlds keep their ground). perlin and simplex: smoother, fewer grid-aligned artefacts, simplex the least. value: soft blobs. cellular: round pits and hummocks like a crater field, try style ridged for ridge lines along cell borders.",
            },
            number(
                "warp",
                Unit::Length,
                0.0,
                0.0,
                2000.0,
                "Bends the noise sideways by up to this many metres: 0 is plain, 100 to 400 twists ridges into believable ranges.",
            ),
            number(
                "island_radius",
                Unit::Length,
                0.0,
                0.0,
                100_000.0,
                "0 leaves the land going on forever. Above 0 the land falls to sea level beyond this radius around the world origin (0, 0), so a continent spans many tiles.",
            ),
            number(
                "island_reach",
                Unit::Ratio,
                0.5,
                0.0,
                1.0,
                "How much of the island radius is a coast sloping down (0 a cliff, 1 a cone).",
            ),
            whole(
                "terraces",
                0,
                0,
                64,
                "0 off. Above 0 the ground is cut into this many flat steps (rice terraces, mesas).",
            ),
            number(
                "terrace_sharpness",
                Unit::Ratio,
                1.0,
                0.0,
                1.0,
                "How hard the steps are: 0 none, 1 flat treads with vertical risers.",
            ),
        ],
    )
}

pub(super) fn mask() -> Option<OperatorSpec> {
    spec(
        "terrain.mask@1",
        "Measures the ground of a terrain.tile into a named 0 to 1 layer you can paint materials and scatter plants by: steepness, bumpiness, height, which way the slope faces, or how enclosed a spot is. Look-around masks use the tile's margin, so neighbouring tiles agree at their edge. The mesh does not change; the layer rides along with the tile.",
        vec![super::super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "mask",
                ty: ParamType::Choice {
                    options: &["slope", "curvature", "height", "aspect", "occlusion"],
                },
                default: Some(Value::Str("slope".to_owned())),
                doc: "slope: 0 flat to 1 vertical (cliffs). curvature: 0.5 flat, above on ridges, below in hollows. height: 0 sea level to 1 the tallest ground. aspect: which way the ground faces, 0 north (+z), 0.25 east, 0.5 south, 0.75 west. occlusion: 0 open to 1 deep in a pit.",
            },
            ParamSpec {
                name: "name",
                ty: ParamType::Str { max_len: 32 },
                default: Some(Value::Str(String::new())),
                doc: "Name of the layer; empty uses the mask's own name. Later nodes refer to it by this name.",
            },
            number(
                "from",
                Unit::Ratio,
                0.0,
                -10.0,
                10.0,
                "The mask value that becomes 0 (everything below is 0). Use it to pick a band, for example slope from 0.3 to 0.6.",
            ),
            number(
                "to",
                Unit::Ratio,
                1.0,
                -10.0,
                10.0,
                "The mask value that becomes 1 (everything above is 1).",
            ),
            flag(
                "smooth",
                false,
                "Soften the edges of the band with a smooth curve.",
            ),
            flag(
                "invert",
                false,
                "Swap 0 and 1 (flat ground instead of cliffs).",
            ),
        ],
    )
}

pub(super) fn biomes() -> Option<OperatorSpec> {
    spec(
        "terrain.biomes@1",
        "Sorts the ground of a terrain.tile into biomes that blend into each other: forest where it is low, damp and gentle, meadow where it is drier, scree where it is steep, snow above the tree line. Each biome is written as soft windows over the ground's height (metres), steepness (degrees from flat) and moisture (a slow noise, 0 dry to 1 wet): `forest: height 0..80, slope 0..30, wet 0.45..1; meadow: slope 0..20, wet 0..0.5; rock: slope 35..` (either end of a window may be left out). Every biome becomes a 0 to 1 layer called `biome_<name>` (change the prefix with `prefix`), faded across its edges by `blend` and shared between biomes that overlap so a place never holds more than 1 in total; plant by it with scatter.place layer=\"biome_forest\" (density follows the layer, so species thin out into each other instead of cutting along a line) and paint it with terrain.material. Also written: `moisture` (the noise) and `slope_degrees`. Neighbouring tiles with the same settings agree at their edge. Up to 12 biomes.",
        vec![super::super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "rules",
                ty: ParamType::Str { max_len: 2048 },
                default: Some(Value::Str(
                    "forest: height 0..80, slope 0..30, wet 0.45..1; meadow: slope 0..25, wet 0..0.55; rock: slope 35..".to_owned(),
                )),
                doc: "The biomes, separated by `;`: `name: height low..high, slope low..high, wet low..high`. height in metres, slope in degrees from flat, wet 0 to 1. A missing window means anywhere; a missing end is open.",
            },
            number(
                "blend",
                Unit::Ratio,
                0.25,
                0.0,
                1.0,
                "How soft the edges are, as a share of a window's width: 0 cuts sharply, 1 fades across the whole window.",
            ),
            number(
                "wet_scale",
                Unit::Length,
                300.0,
                10.0,
                20000.0,
                "Size of the moisture patches in metres.",
            ),
            whole("seed", 0, 0, 1_000_000, "Picks a different moisture pattern; the same seed gives the same one on every tile."),
            ParamSpec {
                name: "prefix",
                ty: ParamType::Str { max_len: 16 },
                default: Some(Value::Str("biome_".to_owned())),
                doc: "Layers are named prefix + biome name.",
            },
        ],
    )
}

pub(super) fn erode() -> Option<OperatorSpec> {
    spec(
        "terrain.erode@1",
        "Weathers a terrain.tile like real mountains: rivers cut valleys (large scale, the same for every tile), loose rock slides down steep slopes, and rain moves soil downhill (fine detail). Neighbouring tiles eroded with the same settings meet without a seam. Erode before terrain.mask: it drops layers made earlier, and adds `water` and `sediment` layers you can use for damp ground. Heavy: the cost grows with the iterations; start small.",
        vec![super::super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            number(
                "river_erosion",
                Unit::Ratio,
                0.5,
                0.0,
                1.0,
                "How strongly rivers carve valleys: 0 off, 0.5 moderate, 1 deep gorges.",
            ),
            whole(
                "river_iterations",
                30,
                1,
                200,
                "Passes of river carving (more is deeper and more drained).",
            ),
            number(
                "talus",
                Unit::None,
                35.0,
                0.0,
                89.0,
                "Steepest slope that loose rock holds, in DEGREES (not radians); steeper slides down. 0 turns it off. 30 to 40 is natural.",
            ),
            whole("thermal_iterations", 20, 1, 100, "Passes of rock sliding."),
            number(
                "rain",
                Unit::Ratio,
                0.01,
                0.0,
                0.1,
                "Rainfall per pass in metres for the fine water erosion; 0 turns it off.",
            ),
            whole(
                "hydraulic_iterations",
                20,
                1,
                100,
                "Passes of rain erosion.",
            ),
            number(
                "pull",
                Unit::Ratio,
                0.3,
                0.0,
                1.0,
                "How strongly fine detail is held to the large-scale valleys (0 free, 1 locked to them).",
            ),
            number(
                "coarse_cell",
                Unit::Length,
                16.0,
                4.0,
                128.0,
                "Metres between points of the large-scale erosion map; smaller is finer and slower.",
            ),
            number(
                "world_size",
                Unit::Length,
                4096.0,
                256.0,
                16384.0,
                "Width of the world square around the origin that the large-scale erosion covers. Use the same value on every tile.",
            ),
        ],
    )
}
