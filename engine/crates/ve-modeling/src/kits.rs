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
        name: "wall_window",
        doc: "Wall segment 2.4 x 2.6 x 0.2 m with a 1.0 x 1.2 m window, frame, bars and glass; origin at floor centre.",
        source: include_str!("../kits/wall_window.txt"),
    },
    Kit {
        name: "window",
        doc: "Window 1.0 x 1.2 x 0.12 m with frame, cross bars and glass; centred on the origin.",
        source: include_str!("../kits/window.txt"),
    },
];
