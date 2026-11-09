//! Ready-made group kits: text definitions built only from existing operators.
//!
//! A kit is a `group` block. Put its source ahead of a program and call it like any group, for example
//! `d = desk(color=[0.3, 0.2, 0.1])`. Each kit exposes only the parameters listed in its `expose` lines.

/// One reusable kit.
#[derive(Clone, Copy, Debug)]
pub struct Kit {
    /// The group name used to call it.
    pub name: &'static str,
    /// One-line description with the footprint in metres.
    pub doc: &'static str,
    /// The group definition in the text language.
    pub source: &'static str,
}

/// Every kit, ordered by name.
pub const KITS: &[Kit] = &[
    Kit {
        name: "adobe_hut",
        doc: "Adobe hut 4.9 x 2.97 x 4.4 m: plaster walls with rounded edges, a door and a window opening and five projecting roof beams; `plaster_color` and `wood_color`; origin at floor centre.",
        source: include_str!("../kits/adobe_hut.txt"),
    },
    Kit {
        name: "arch",
        doc: "Arch 1.6 x 1.6 x 0.34 m: a wall with a 0.8 x 1.1 m arched opening and a keystone; `wall`, `portal` and `keystone` are their box sizes; origin at floor centre.",
        source: include_str!("../kits/arch.txt"),
    },
    Kit {
        name: "barrel",
        doc: "Barrel 0.64 x 0.8 x 0.64 m with sixteen staves, two end boards and two hoops; `staves` is the count, `radius` the stave ring and `hoop` the hoop radius; origin at floor centre.",
        source: include_str!("../kits/barrel.txt"),
    },
    Kit {
        name: "bed",
        doc: "Bed 1.4 x 0.8 x 2.04 m with frame, mattress and headboard; `frame`, `mattress` and `headboard` are their box sizes; origin at floor centre.",
        source: include_str!("../kits/bed.txt"),
    },
    Kit {
        name: "bench",
        doc: "Bench 1.5 x 0.98 x 0.45 m with seat, four legs, two back posts and a back rail; `seat` and `back` are their box sizes; origin at floor centre.",
        source: include_str!("../kits/bench.txt"),
    },
    Kit {
        name: "cart",
        doc: "Hand cart 3.9 x 1.2 x 1.54 m: a plank bed with sides and tail board, two shafts, an axle and two rimmed wheels with hubs; origin on the bed centre at the floor.",
        source: include_str!("../kits/cart.txt"),
    },
    Kit {
        name: "chair",
        doc: "Chair 0.45 x 0.98 x 0.455 m with seat, four legs and a back; `seat` and `back` are their box sizes; origin at floor centre.",
        source: include_str!("../kits/chair.txt"),
    },
    Kit {
        name: "column",
        doc: "Column 0.52 x 2.4 x 0.52 m with base, tapered shaft and capital; `radius`/`top_radius` shape the shaft and `segments` its facets; origin at floor centre.",
        source: include_str!("../kits/column.txt"),
    },
    Kit {
        name: "crate",
        doc: "Crate 0.64 x 0.64 x 0.64 m with corner posts and two bands; `body`, `post` and `band` are their box sizes; origin at floor centre.",
        source: include_str!("../kits/crate.txt"),
    },
    Kit {
        name: "desk",
        doc: "Writing desk 1.4 x 0.76 x 0.7 m with a side drawer; origin at floor centre.",
        source: include_str!("../kits/desk.txt"),
    },
    Kit {
        name: "door",
        doc: "Door 1.0 x 2.1 x 0.14 m with frame, hinged leaf and handle; `swing` is [x, y, z] degrees.",
        source: include_str!("../kits/door.txt"),
    },
    Kit {
        name: "fence",
        doc: "Fence 3.08 x 1.2 x 0.08 m with five posts, two rails and nineteen jittered boards; `posts` and `boards` are counts, `spacing` the post spacing and `seed` the board variation; origin at floor centre.",
        source: include_str!("../kits/fence.txt"),
    },
    Kit {
        name: "jar",
        doc: "Clay jar 0.46 x 0.6 x 0.46 m: a hollow turned vessel with a narrow neck; `color` and `roughness`; origin at floor centre.",
        source: include_str!("../kits/jar.txt"),
    },
    Kit {
        name: "lamp_floor",
        doc: "Floor lamp 0.36 x 1.65 x 0.36 m with a round base, slim pole and tapered shade; `base_radius`, `pole_radius` and `shade_radius` size its parts; origin at floor centre.",
        source: include_str!("../kits/lamp_floor.txt"),
    },
    Kit {
        name: "pipe_run",
        doc: "L-shaped pipe run 1.06 x 0.12 x 0.86 m with a 1.0 m main, 0.8 m branch and elbow; `main_radius`, `branch_radius` and `elbow` size its parts; origin at floor centre.",
        source: include_str!("../kits/pipe_run.txt"),
    },
    Kit {
        name: "railing",
        doc: "Railing 2.06 x 0.93 x 0.06 m with five posts and two rails; `posts` is the count per side, `spacing` the post spacing and `span` the half-rail size; origin at floor centre.",
        source: include_str!("../kits/railing.txt"),
    },
    Kit {
        name: "roof_gable",
        doc: "Gable roof 3.2 x 1.1 x 4.5 m over a 3.0 x 4.0 m building (0.1 m eaves, 0.25 m gable overhang); `section` points set the eaves and pitch, `length` the roof depth and `ridge` the cap size; origin at floor centre.",
        source: include_str!("../kits/roof_gable.txt"),
    },
    Kit {
        name: "ruin_wall",
        doc: "Ruined brick wall 4 x 2.6 x 0.4 m: a window opening and a broken, notched top with rounded edges; `wall` is its box size; origin at floor centre.",
        source: include_str!("../kits/ruin_wall.txt"),
    },
    Kit {
        name: "shelf",
        doc: "Bookcase 0.85 x 1.8 x 0.3 m with six boards and a row of leaning books; `arrangement` is the seed.",
        source: include_str!("../kits/shelf.txt"),
    },
    Kit {
        name: "sign_post",
        doc: "Sign post 0.8 x 2.0 x 0.08 m with a square post and a board; `post` and `sign` are their box sizes; origin at floor centre.",
        source: include_str!("../kits/sign_post.txt"),
    },
    Kit {
        name: "stairs",
        doc: "Stairs 1.0 x 1.08 x 1.82 m: six treads with a 0.18 m rise and 0.28 m run; `size` is [width, rise, tread depth] and `steps` the count; origin at floor centre.",
        source: include_str!("../kits/stairs.txt"),
    },
    Kit {
        name: "table",
        doc: "Table 1.2 x 0.78 x 0.7 m with a top and four turned legs; `top` is its box size, `radius`/`segments` shape the legs; origin at floor centre.",
        source: include_str!("../kits/table.txt"),
    },
    Kit {
        name: "tent",
        doc: "Open-ended canvas tent 2.8 x 1.9 x 3.18 m: a thin gabled shell with two wooden poles; `cloth_color` and `wood_color`; origin at floor centre.",
        source: include_str!("../kits/tent.txt"),
    },
    Kit {
        name: "wall_window",
        doc: "Wall segment 2.4 x 2.6 x 0.2 m with a 1.0 x 1.2 m window, frame, bars and glass; origin at floor centre.",
        source: include_str!("../kits/wall_window.txt"),
    },
    Kit {
        name: "well",
        doc: "Stone well 1.8 x 2.35 x 1.8 m: a rounded ring wall, two posts and a crossbeam; `radius` is the ring radius; origin at floor centre.",
        source: include_str!("../kits/well.txt"),
    },
    Kit {
        name: "window",
        doc: "Window 1.0 x 1.2 x 0.12 m with frame, cross bars and glass; centred on the origin.",
        source: include_str!("../kits/window.txt"),
    },
];

/// Plant kits, written in the language like the others but organic: sizes are approximate (the footprint in the doc is a guide, not a
/// 2% contract), shapes vary with their `seed`, and they are meant to be placed in thousands (`scatter.place`, `inst.vary`).
pub const PLANT_KITS: &[Kit] = &[
    Kit {
        name: "grass_tuft",
        doc: "Grass tuft about 0.1 x 0.45 x 0.1 m: seven bent blades in a ring, 0.45 m tall; `count` is the blade count and `color` the green; place it by the thousand with scatter.place and vary it with inst.vary; origin at the ground.",
        source: include_str!("../kits/grass_tuft.txt"),
    },
    Kit {
        name: "oak",
        doc: "Oak about 9.6 x 10 x 9.6 m: a broad lumpy crown of three blended blobs on a tapering curved trunk, made from fields and a tube (no species code); `seed` changes the crown's lumps, `edge_length` the detail (0.35 is 7,400 triangles, 0.8 is 1,600, 1.2 is 800), `leaf_color` and `bark_color` the colours; place it with scatter.place and vary its size with inst.vary; origin at the foot of the trunk.",
        source: include_str!("../kits/oak.txt"),
    },
    Kit {
        name: "wildflower",
        doc: "Wildflower about 0.26 x 0.5 x 0.14 m: a bent stem, two leaves and a head of petals around a centre, swaying in the wind; `petal_color`, `centre_color` and `stem_color` recolour it and `petals` sets the count (5 to 10). Plant several colourings with scatter.place `mix` and `pick` for a meadow.",
        source: include_str!("../kits/wildflower.txt"),
    },
    Kit {
        name: "fern",
        doc: "Fern about 1.5 x 0.35 x 1.5 m: six arching fronds, each a curved midrib with twelve pairs of leaflets that shorten toward the tip (about 11,000 triangles); `color` recolours it, `fronds` sets the count and `seed` varies the fronds' turn and length. Ferns like shade: plant them under trees.",
        source: include_str!("../kits/fern.txt"),
    },
    Kit {
        name: "mushroom",
        doc: "Mushroom about 0.12 x 0.12 x 0.12 m: a rounded stem and a domed cap with spots; `cap_color`, `stem_color` and `spot_color` recolour it (red with white spots, brown, yellow) and `spots` sets the spot count. Plant in clusters at the feet of trees and on logs.",
        source: include_str!("../kits/mushroom.txt"),
    },
    Kit {
        name: "log",
        doc: "Fallen log about 2.4 x 0.5 x 0.5 m lying along x: a bumpy trunk with furrowed bark, dirt in the creases and moss on top; `length`, `radius` and `seed` change it. Turn it with scatter.place `spin` and let moss, mushrooms and ferns grow near it.",
        source: include_str!("../kits/log.txt"),
    },
    Kit {
        name: "stump",
        doc: "Tree stump about 0.8 x 0.44 x 0.8 m: a flared base, bark, a pale cut top and a little moss; `seed` changes its lumps.",
        source: include_str!("../kits/stump.txt"),
    },
    Kit {
        name: "reed",
        doc: "Reeds about 0.2 x 1.4 x 0.2 m: nine tall bent blades and a brown cattail on a stalk, swaying strongly in the wind; `count` is the blade count, `color` the green and `head_color` the cattail. Plant along shores with a terrain.mask of low or wet ground.",
        source: include_str!("../kits/reed.txt"),
    },
    Kit {
        name: "flowering_bush",
        doc: "Flowering bush about 1.2 x 0.9 x 1.1 m: a lumpy green mound of three blended blobs, textured, with ninety blossoms scattered over it; `flower_color` recolours the blossoms (pink, yellow, white), `blooms` sets how many and `seed` changes the lumps.",
        source: include_str!("../kits/flowering_bush.txt"),
    },
    Kit {
        name: "rock",
        doc: "Mossy rock about 1.0 x 0.6 x 0.8 m: a rough boulder with grey stone, dark creases and moss on its top; `seed` changes its shape. Plant by the hundred with scatter.place (sink 0.1 to 0.2 beds it in, align 0.5 follows slopes) and scale each with `scale`.",
        source: include_str!("../kits/rock.txt"),
    },
    Kit {
        name: "pine",
        doc: "Pine about 4.8 x 9.4 x 4.8 m: four stacked cones on a short trunk with a ragged outline; `seed` changes the ragged outline, `ragged` its strength (0.25 is 2,700 triangles, 0 is a clean 280), `scale` is a per-axis size factor (1 is 9.4 m tall), `needle_color` and `bark_color` the colours; origin at the foot of the trunk.",
        source: include_str!("../kits/pine.txt"),
    },
];

/// Macro preludes: text of `def` macros (not a group) pasted once above a program, then called like a line of it. They build whole systems from a few
/// calls, so the program stays short.
pub const MACRO_KITS: &[Kit] = &[
    Kit {
        name: "autorig_human",
        doc: "Autorig for body.human: human_rig(\"body\", hands, free) makes every bone a part of its own with its output named like the bone and its parent link (root, spine, neck, head, arms, legs, optionally all fingers); human_skin(\"body\", \"person\", hands, r, g, b) skins the body with the pack's weights and gives it a colour; human_ik(\"l\") adds reach targets for a hand and a foot. A rigged, skinned character is three lines; the header comments show how to animate a bone.",
        source: include_str!("../kits/autorig_human.txt"),
    },
    Kit {
        name: "face_presets",
        doc: "Facial expressions for body.human as text constants: face_happy, face_sad, face_angry, face_surprised, face_wink, face_blink, face_talk and face_disgust, each a set of ARKit face actions; use body.human@1(face=\"${face_happy}\") (join two with a semicolon, or add more actions).",
        source: include_str!("../kits/face_presets.txt"),
    },
    Kit {
        name: "hair_cards",
        doc: "Hair that moves: hair_cards(name, bone, cx, cy, cz, radius, length, count, width, seed) builds `count` flat strips hanging from a ring of roots (centre cx cy cz, just above a forehead) and simulates them as cloth pinned to `bone`, so the hair swings, trails a turning head and blows in the wind; output named `name`.",
        source: include_str!("../kits/hair_cards.txt"),
    },
];
