//! A modern district from a seed: streets, blocks, lots and a building on every lot (ADR 0056's world family; the algorithms are `ve-world::city` and `ve-world::building`).
use super::spec;
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn number(
    name: &'static str,
    unit: Unit,
    default: f64,
    min: f64,
    max: f64,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num { unit, min, max },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn pair(name: &'static str, x: f64, z: f64, min: f64, max: f64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec2 {
            unit: Unit::Length,
            min,
            max,
        },
        default: Some(Value::List(vec![Value::Num(x), Value::Num(z)])),
        doc,
    }
}

fn vector3(
    name: &'static str,
    value: [f64; 3],
    min: f64,
    max: f64,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 {
            unit: Unit::Length,
            min,
            max,
        },
        default: Some(Value::List(value.into_iter().map(Value::Num).collect())),
        doc,
    }
}

/// The parameters that lay the district out; every city node takes the same ones, so a node of any family computes the same streets and lots.
fn layout_params() -> Vec<ParamSpec> {
    vec![
        pair(
            "size",
            300.0,
            300.0,
            60.0,
            1000.0,
            "Width along X and depth along Z of the district in metres, centred on the origin.",
        ),
        pair(
            "centre",
            0.0,
            0.0,
            -1000.0,
            1000.0,
            "Where downtown is (X, Z): the tallest towers stand here and buildings fall away from it.",
        ),
        number(
            "block",
            Unit::Length,
            40.0,
            15.0,
            200.0,
            "Street spacing along X in metres, centre to centre: 40 is a dense Tokyo-like grid, 80 a spread-out modern district.",
        ),
        number(
            "block_depth",
            Unit::Length,
            0.0,
            0.0,
            400.0,
            "Street spacing along Z. 0 (the default) means the same as block, a square grid; 15 or more makes long thin blocks: block 80 with block_depth 270 is the shape of Manhattan's.",
        ),
        number(
            "street_width",
            Unit::Length,
            9.0,
            2.0,
            40.0,
            "Metres between facing buildings: kerb to kerb with the two pavements.",
        ),
        number(
            "lot_min",
            Unit::Length,
            6.0,
            3.0,
            40.0,
            "The smallest building plot side wanted, in metres.",
        ),
        number(
            "lot_max",
            Unit::Length,
            14.0,
            4.5,
            80.0,
            "Blocks are split into plots until their long side is at most this; at least 1.5 times lot_min.",
        ),
        number(
            "irregularity",
            Unit::Ratio,
            0.5,
            0.0,
            1.0,
            "0 is a square grid, 1 bends the streets like an old city.",
        ),
        ParamSpec {
            name: "seed",
            ty: ParamType::Int {
                min: 0,
                max: 1_000_000,
            },
            default: Some(Value::Int(1)),
            doc: "The same seed gives the same district.",
        },
    ]
}

fn ground() -> Option<OperatorSpec> {
    let mut params = layout_params();
    params.push(ParamSpec {
        name: "part",
        ty: ParamType::Choice { options: &["road", "pavement"] },
        default: Some(Value::Str("road".to_owned())),
        doc: "road: the asphalt under the whole district. pavement: the raised pavement of every block. Make one node of each and give them materials.",
    });
    spec(
        "city.ground@1",
        "The ground of a modern district: the road surface under everything and the raised pavement block by block, from the same streets as city.buildings (use the same size, block, street_width, lot sizes, irregularity and seed). UVs are metres / 8, so a tiling texture keeps its scale.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        params,
    )
}

fn buildings() -> Option<OperatorSpec> {
    let mut params = layout_params();
    params.extend([
        ParamSpec {
            name: "part",
            ty: ParamType::Choice { options: &["shop", "walls", "roof", "metal", "awning"] },
            default: Some(Value::Str("walls".to_owned())),
            doc: "shop: the ground floors (and stoops). walls: the shafts above them, with their setback tiers and cornices. roof: roofs, ledges, rooftop clutter, water towers and spires. metal: iron fire escapes (needs detail 0.34 or more). awning: shop awnings (needs detail 0.2 or more). One node per part, each with its own material.",
        },
        ParamSpec {
            name: "styles",
            ty: ParamType::Int { min: 1, max: 8 },
            default: Some(Value::Int(4)),
            doc: "How many facade styles the buildings are split into: each building belongs to one, picked from its plot. Make one node per style.",
        },
        ParamSpec {
            name: "style",
            ty: ParamType::Int { min: 0, max: 7 },
            default: Some(Value::Int(0)),
            doc: "Which style this node builds, 0 to styles - 1: only the buildings of that style are in it.",
        },
        number("height_scale", Unit::Ratio, 1.0, 0.3, 2.0, "Scales every building's height: 0.5 gives a low district, 1.5 a skyline."),
        number("height_min", Unit::Length, 3.0, 3.0, 300.0, "No building is lower than this many metres, after height_scale. Raise it for a district of even, mid-rise streets (Paris is about 20 m)."),
        number("height_max", Unit::Length, 300.0, 3.0, 300.0, "No building is taller than this many metres, after height_scale: 25 gives a district with no towers. Must not be below height_min."),
        number("floor", Unit::Length, 3.6, 2.4, 6.0, "Storey height in metres; sets how often the facade texture's floors repeat."),
        number("clutter", Unit::Ratio, 0.8, 0.0, 1.0, "Rooftop clutter: air-conditioning units, water tanks, stair cores and antennas. 0 gives flat roofs."),
        number("detail", Unit::Ratio, 0.0, 0.0, 1.0, "New York street detail, 0 (none, the default) to 1: shop awnings from 0.2, a stoop on houses under 16 m from 0.4, fire escapes on buildings of 18 to 32 m from 0.34, cornices and rooftop wooden water towers from 0.5, and on towers over 90 m a second setback and a stepped crown with a mast from 0.3. Fire escapes are in part metal, awnings in part awning."),
        number("mansard", Unit::Ratio, 0.0, 0.0, 1.0, "The share of buildings from about 19 m to 50 m tall that get a Paris-style mansard roof: a steep slope one storey high with dormer windows, drawn in part roof (the dormer fronts are in part walls). 0, the default, keeps every roof flat; 1 gives them all one."),
        number("courtyards", Unit::Ratio, 0.0, 0.0, 1.0, "The share of big rectangular plots (16 m or more on each side) built as an L or a U round a courtyard instead of one box: a street wing at full height and lower wings behind it. 0, the default, keeps one box on every plot."),
    ]);
    spec(
        "city.buildings@1",
        "Every building of a modern district: a shop floor, a shaft that steps back in tiers when tall, and a flat roof with rooftop clutter, on every plot of an irregular street grid, tallest downtown. Wall UVs are facade units (u every 8 m along a wall, v every three floors) so ONE tiling facade texture fits every building. The part (shop, walls, roof) and the style choose which meshes this node returns, so build each part and style as its own node and texture it: for example three parts times four styles. About 100 triangles a building: the default district is under 150,000.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        params,
    )
}

fn building_asset() -> Option<OperatorSpec> {
    let mut params = vec![
        {
            vector3(
                "at",
                [0.0, 0.0, 0.0],
                -1000.0,
                1000.0,
                "Building origin in metres (x, y, z), so open facade meshes can be placed without a solid boolean transform.",
            )
        },
        number(
            "width",
            Unit::Length,
            24.0,
            8.0,
            80.0,
            "Building frontage in metres.",
        ),
        number(
            "depth",
            Unit::Length,
            18.0,
            8.0,
            80.0,
            "Building depth in metres.",
        ),
        ParamSpec {
            name: "floors",
            ty: ParamType::Int { min: 2, max: 40 },
            default: Some(Value::Int(8)),
            doc: "Number of storeys.",
        },
        ParamSpec {
            name: "bays",
            ty: ParamType::Int { min: 2, max: 16 },
            default: Some(Value::Int(6)),
            doc: "Window bays on each facade.",
        },
        number(
            "floor_height",
            Unit::Length,
            3.6,
            2.8,
            5.5,
            "Storey height in metres.",
        ),
        ParamSpec {
            name: "detail",
            ty: ParamType::Int { min: 0, max: 3 },
            default: Some(Value::Int(2)),
            doc: "Facade detail tier: mullions, sills and cornice details.",
        },
        ParamSpec {
            name: "style",
            ty: ParamType::Choice {
                options: &[
                    "art_deco",
                    "classical",
                    "tenement",
                    "brownstone",
                    "precast",
                    "curtainwall",
                    "industrial",
                    "brutalist",
                    "modernist",
                    "mediterranean",
                    "victorian",
                    "warehouse",
                ],
            },
            default: Some(Value::Str("art_deco".to_owned())),
            doc: "Architectural facade grammar. Each style changes opening proportions, floor rhythm, vertical elements, setbacks and roof equipment.",
        },
        ParamSpec {
            name: "part",
            ty: ParamType::Choice {
                options: &["masonry", "glass", "trim", "roof", "doors"],
            },
            default: Some(Value::Str("masonry".to_owned())),
            doc: "Material-separated geometry group, including entry doors. Make one node per part to assign distinct materials.",
        },
        ParamSpec {
            name: "seed",
            ty: ParamType::Int {
                min: 0,
                max: 1_000_000,
            },
            default: Some(Value::Int(1)),
            doc: "Stable variation seed.",
        },
    ];
    params.shrink_to_fit();
    spec(
        "building.asset@1",
        "A close-view, rule-built building asset. Twelve architectural grammars change window proportions, floor rhythm, vertical elements and roof equipment. Floor and bay rules create openings, recessed panes, mullions, framed street entries and separate masonry/glass/trim/roof/door meshes. Width and depth support long slabs through narrow towers; geometry and facade UVs are deterministic.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        params,
    )
}

fn axis_param() -> ParamSpec {
    ParamSpec {
        name: "axis",
        ty: ParamType::Choice {
            options: &["x", "z"],
        },
        default: Some(Value::Str("x".to_owned())),
        doc: "Cardinal alignment; x runs along X and z runs along Z, keeping repeated street modules deterministic.",
    }
}

fn street_road() -> Option<OperatorSpec> {
    let mut params = vec![vector3(
        "at",
        [0.0, 0.0, 0.0],
        -1000.0,
        1000.0,
        "Road segment centre in metres (x, y, z).",
    )];
    params.extend([
        number("length", Unit::Length, 80.0, 8.0, 240.0, "Segment length in metres."),
        number("width", Unit::Length, 12.0, 3.0, 36.0, "Asphalt width in metres, excluding pavement."),
        ParamSpec { name: "lanes", ty: ParamType::Int { min: 1, max: 8 }, default: Some(Value::Int(2)), doc: "Lane count; paint is spaced evenly with dashed dividers and a double centre line on two-way roads." },
        axis_param(),
        ParamSpec { name: "part", ty: ParamType::Choice { options: &["asphalt", "markings", "crosswalk", "kerb", "pavement"] }, default: Some(Value::Str("asphalt".to_owned())), doc: "Build each part as its own node, then assign asphalt, paint, stone and concrete materials." },
    ]);
    spec(
        "street.road@1",
        "A straight, metre-scaled road kit module: asphalt, lane paint, crosswalks, paired raised kerbs and pavements. Match length, width, axis and centre when making its material-separated parts; join segments to build streets and intersections.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        params,
    )
}

fn street_prop() -> Option<OperatorSpec> {
    let mut params = vec![vector3(
        "at",
        [0.0, 0.0, 0.0],
        -1000.0,
        1000.0,
        "Prop base position in metres (x, y, z).",
    )];
    params.extend([
        number("height", Unit::Length, 6.0, 1.0, 18.0, "Mast or prop height in metres; fixed-size furniture ignores it."),
        axis_param(),
        ParamSpec { name: "kind", ty: ParamType::Choice { options: &["mast", "streetlight", "traffic_signal", "sign", "utility_mast", "bollard", "hydrant", "bench", "bin", "bus_shelter"] }, default: Some(Value::Str("streetlight".to_owned())), doc: "Reusable street object: lighting, signal mast, utility pole, sign, pedestrian furniture or shelter." },
        ParamSpec { name: "part", ty: ParamType::Choice { options: &["body", "accent", "lens"] }, default: Some(Value::Str("body".to_owned())), doc: "Separate structural body and accents for materials. lens is available on streetlight and traffic_signal." },
    ]);
    spec(
        "street.prop@1",
        "A deterministic, placeable street prop kit. Build matching kind/position/axis nodes for body, accents and (for lights) lenses to assign materials. Supports lamp posts, traffic and utility masts, signs, bollards, hydrants, benches, bins and bus shelters.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        params,
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        ground(),
        buildings(),
        building_asset(),
        street_road(),
        street_prop(),
    ]
}
