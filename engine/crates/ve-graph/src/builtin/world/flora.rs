//! Flora node specifications.
use super::*;

pub(super) fn tree() -> Option<OperatorSpec> {
    let mut params = vec![
        ParamSpec {
            name: "kind",
            ty: ParamType::Choice {
                options: &["oak", "pine", "birch", "willow", "shrub", "palm"],
            },
            default: Some(Value::Str("oak".to_owned())),
            doc: "The species: oak is broad, pine is a tall cone with drooping whorls, birch slender, willow long and drooping, shrub low with many stems, palm a curved trunk with fronds.",
        },
        number("height", Unit::Length, 8.0, 0.3, 80.0, "Height in metres."),
        whole("seed", 0, 0, 2_147_483_647, "Picks a different tree of the same species."),
        whole(
            "sides",
            12,
            3,
            16,
            "Sides of the trunk tube at its thickest: 6 is plenty at a distance, 12 for a trunk you can touch.",
        ),
    ];
    params.extend(trunk_params());
    params.extend(foliage_params());
    spec(
        "flora.tree@1",
        "A real tree: a branching trunk with species bark, smooth curved limbs and a lobed root flare, and leaf cards with cut-out leaf shapes that let the sun through, grown from a named species (oak, pine, birch, willow, shrub, palm) at the height you ask, the same tree for the same seed. Its base is at y = 0, so it plants straight into scatter.place. Colours, how full the leaves are, and a level of detail are parameters: use lod 0 for trees near the viewer and 2 or 3 for trees far away (each level has about half the triangles of the one before). One call per kind of tree; use scatter.place for the forest.",
        vec![],
        OutputSpec::Fixed(PortType::Solid),
        params,
    )
}

/// What the trunk and limbs look like: bark colour and pattern, the root flare and how smooth the limbs are.
fn trunk_params() -> Vec<ParamSpec> {
    vec![
        colour(
            "bark",
            [0.3, 0.2, 0.13],
            "Bark colour [r, g, b] (sRGB). Left at the default, each species wears its own bark (furrowed brown oak, white birch with dark marks, plated red pine); any other colour recolours the bark pattern.",
        ),
        ParamSpec {
            name: "bark_style",
            ty: ParamType::Choice {
                options: &["auto", "furrowed", "white", "plated", "rings", "smooth"],
            },
            default: Some(Value::Str("auto".to_owned())),
            doc: "The bark pattern: auto picks the species' own. furrowed: deep vertical ridges. white: pale with dark horizontal marks (birch). plated: scaly plates (pine). rings: horizontal rings (palm). smooth: even bark (young stems).",
        },
        number(
            "flare",
            Unit::Ratio,
            1.0,
            0.0,
            3.0,
            "How much the trunk swells into lobed roots where it meets the ground (0 a plain cylinder, 1 about double the radius at the soil).",
        ),
        whole(
            "smooth",
            2,
            0,
            6,
            "Extra rings between branch nodes on a smooth curve, so limbs bend instead of kinking (0 keeps straight segments; each ring adds triangles).",
        ),
    ]
}

/// What the leaves look like and how the tree moves and thins out with distance.
fn foliage_params() -> Vec<ParamSpec> {
    vec![
        colour(
            "leaves",
            [0.2, 0.42, 0.12],
            "Leaf colour [r, g, b] (sRGB); leaves vary a little around it, and a few lean yellow.",
        ),
        ParamSpec {
            name: "season",
            ty: ParamType::Choice {
                options: &["summer", "spring", "autumn", "winter"],
            },
            default: Some(Value::Str("summer".to_owned())),
            doc: "The time of year, when `leaves` is left at its default green: summer is full green, spring a pale fresh green, autumn orange and red with yellow leaves among them, winter bare branches. Any other `leaves` colour wins over the season (winter is always bare).",
        },
        ParamSpec {
            name: "leaf_style",
            ty: ParamType::Choice {
                options: &["clumps", "crossed", "quads"],
            },
            default: Some(Value::Str("clumps".to_owned())),
            doc: "clumps: several cards fanned around each leaf point (full, soft crowns). crossed: two cards crossed (reads from any side). quads: one card (cheapest).",
        },
        number(
            "leaf_size",
            Unit::Ratio,
            1.0,
            0.3,
            3.0,
            "Multiplier of the leaf card size: larger covers the crown more fully.",
        ),
        number(
            "sway",
            Unit::Ratio,
            0.5,
            0.0,
            1.0,
            "How much the leaves move in the scene's wind (0 still).",
        ),
        whole(
            "lod",
            0,
            0,
            4,
            "Level of detail: 0 is the full tree, each level up has about half the triangles.",
        ),
    ]
}
