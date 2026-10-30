//! Window and door construction rules for the facade grammar.

use super::super::Rng;
use super::mesh::{block_on_edge, face_outward, outside, point, wall_panel};
use super::styles::profile;
use super::{HeroAsset, HeroAssetParams};

pub(super) fn window_opening(
    asset: &mut HeroAsset,
    params: &HeroAssetParams,
    a: [f32; 2],
    b: [f32; 2],
    floor: u8,
    bay: u8,
    edge: usize,
    base_y: f32,
    rng: &mut Rng,
) {
    if floor == 0 && edge == 0 && bay == params.bays / 2 {
        door_opening(asset, params, a, b);
        return;
    }
    let style = profile(&params.style);
    let bays = f32::from(params.bays);
    let start = f32::from(bay) / bays;
    let end = f32::from(bay + 1) / bays;
    let mut width = (end - start) * style.window_width;
    if rng.next() > 0.8 {
        width *= 0.96;
    }
    let centre = (start + end) * 0.5;
    let edge_length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    let u0 = centre - width * 0.5;
    let u1 = centre + width * 0.5;
    let window_height = if floor == 0 {
        params.floor_height * style.ground_window_height
    } else {
        params.floor_height * style.upper_window_height
    };
    let sill = base_y + params.floor_height * style.sill;
    let top = (sill + window_height).min(base_y + params.floor_height * 0.91);
    let margin = (0.1 / edge_length).min((u0 - start) * 0.5);
    let left = u0 - margin;
    let right = u1 + margin;
    wall_panel(
        &mut asset.masonry,
        a,
        b,
        start,
        left,
        base_y,
        base_y + params.floor_height,
    );
    wall_panel(
        &mut asset.masonry,
        a,
        b,
        right,
        end,
        base_y,
        base_y + params.floor_height,
    );
    wall_panel(&mut asset.masonry, a, b, left, right, base_y, sill);
    wall_panel(
        &mut asset.masonry,
        a,
        b,
        left,
        right,
        top,
        base_y + params.floor_height,
    );

    let normal = outside(a, b);
    let offset = |p: [f32; 3], distance: f32| {
        [
            p[0] + normal[0] * distance,
            p[1],
            p[2] + normal[1] * distance,
        ]
    };
    let corners = [
        offset(point(a, b, u0, sill), 0.035),
        offset(point(a, b, u0, top), 0.035),
        offset(point(a, b, u1, top), 0.035),
        offset(point(a, b, u1, sill), 0.035),
    ];
    face_outward(
        &mut asset.glass,
        [corners[0], corners[1], corners[2], corners[3]],
        [normal[0], 0.0, normal[1]],
    );

    // Residential families get shallow, rail-equipped balconies on the street facade.
    if style.balconies && floor > 0 && edge == 0 {
        let balcony_width = edge_length * width;
        let balcony_y = sill - 0.12;
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            centre,
            0.34,
            [balcony_width, 0.12, 0.82],
            balcony_y,
        );
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            centre,
            0.72,
            [balcony_width, 0.07, 0.07],
            balcony_y + 0.88,
        );
        for u in [u0 + 0.025, u1 - 0.025] {
            block_on_edge(
                &mut asset.trim,
                a,
                b,
                u,
                0.72,
                [0.055, 0.92, 0.055],
                balcony_y,
            );
        }
    }

    let frame = style.frame;
    let depth = if params.style == "curtainwall" {
        0.10
    } else {
        0.16
    };
    for u in [u0, u1] {
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            u,
            0.08,
            [frame, top - sill + frame * 2.0, depth],
            sill - frame,
        );
    }
    for y in [sill - frame, top] {
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            centre,
            0.08,
            [edge_length * width + frame * 2.0, frame, depth],
            y,
        );
    }
    if params.detail >= 1 || params.style == "curtainwall" {
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            centre,
            0.06,
            [frame * 0.65, top - sill, depth * 0.7],
            sill,
        );
    }
    if params.detail >= 2 || params.style == "curtainwall" {
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            centre,
            0.065,
            [edge_length * width, frame * 0.65, depth * 0.7],
            (sill + top) * 0.5,
        );
        let sill_size = if params.style == "art_deco" || params.style == "classical" {
            0.22
        } else {
            0.12
        };
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            centre,
            0.12,
            [edge_length * width + 0.18, sill_size, depth * 1.2],
            sill - sill_size,
        );
    }
    if params.detail >= 3 && params.style != "curtainwall" {
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            centre,
            0.12,
            [edge_length * width + 0.25, 0.12, depth * 1.5],
            top,
        );
    }
}

fn door_opening(asset: &mut HeroAsset, params: &HeroAssetParams, a: [f32; 2], b: [f32; 2]) {
    let edge_length = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    let bays = f32::from(params.bays);
    let start = f32::from(params.bays / 2) / bays;
    let end = f32::from(params.bays / 2 + 1) / bays;
    let bay_width = edge_length * (end - start);
    let door_width = (bay_width * 0.44).clamp(0.25, 2.2).min(bay_width * 0.8);
    let width = door_width / edge_length;
    let centre = (start + end) * 0.5;
    let u0 = centre - width * 0.5;
    let u1 = centre + width * 0.5;
    let height = (params.floor_height * 0.68).clamp(2.0, 2.45);
    let frame = 0.12;
    wall_panel(
        &mut asset.masonry,
        a,
        b,
        start,
        u0,
        0.0,
        params.floor_height,
    );
    wall_panel(&mut asset.masonry, a, b, u1, end, 0.0, params.floor_height);
    wall_panel(
        &mut asset.masonry,
        a,
        b,
        u0,
        u1,
        height + frame,
        params.floor_height,
    );
    let normal = outside(a, b);
    let offset = |p: [f32; 3]| [p[0] + normal[0] * 0.045, p[1], p[2] + normal[1] * 0.045];
    face_outward(
        &mut asset.doors,
        [
            offset(point(a, b, u0 + 0.035 / edge_length, 0.035)),
            offset(point(a, b, u0 + 0.035 / edge_length, height)),
            offset(point(a, b, u1 - 0.035 / edge_length, height)),
            offset(point(a, b, u1 - 0.035 / edge_length, 0.035)),
        ],
        [normal[0], 0.0, normal[1]],
    );
    for u in [u0, u1] {
        block_on_edge(
            &mut asset.trim,
            a,
            b,
            u,
            0.075,
            [frame, height + frame, 0.22],
            0.0,
        );
    }
    block_on_edge(
        &mut asset.trim,
        a,
        b,
        centre,
        0.075,
        [door_width + frame * 2.0, frame, 0.22],
        height,
    );
    block_on_edge(
        &mut asset.doors,
        a,
        b,
        centre,
        0.22,
        [door_width + 0.42, 0.12, 0.5],
        -0.12,
    );
    block_on_edge(
        &mut asset.doors,
        a,
        b,
        u1 - 0.12 / edge_length,
        0.19,
        [0.045, 0.14, 0.05],
        height * 0.48,
    );
}
