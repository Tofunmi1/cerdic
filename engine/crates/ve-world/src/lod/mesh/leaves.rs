//! Leaf cards: one quad, two crossed quads, or a fan of quads around each leaf point.
use super::super::{LeafCards, PlantMesh, Vertex, WindData};
use super::helpers::{cross, math};
use crate::flora::{Leaf, Skeleton};

pub(super) fn add_leaf(mesh: &mut PlantMesh, skeleton: &Skeleton, leaf: &Leaf, mode: LeafCards) {
    let direction = math::unit(leaf.direction);
    let mut across = math::perpendicular(direction);
    match mode {
        LeafCards::Quads { aspect, bend_deg } => {
            add_card(mesh, skeleton, leaf, direction, across, aspect, bend_deg);
        }
        LeafCards::Crossed { aspect } => {
            add_card(mesh, skeleton, leaf, direction, across, aspect, 0.0);
            across = math::unit(cross(direction, across));
            add_card(mesh, skeleton, leaf, direction, across, aspect, 0.0);
        }
        LeafCards::Clumps {
            leaves,
            spread,
            aspect,
        } => {
            for clump in 0..leaves {
                let angle = spread * core::f32::consts::PI / 180.0
                    * (f32::from(u16::try_from(clump).unwrap_or(0))
                        / f32::from(u16::try_from(leaves).unwrap_or(1))
                        - 0.5);
                let side = math::rotate(across, direction, angle);
                add_card(mesh, skeleton, leaf, direction, side, aspect, 0.0);
            }
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "mesh vertex count is capped at 400,000"
)]
fn add_card(
    mesh: &mut PlantMesh,
    skeleton: &Skeleton,
    leaf: &Leaf,
    direction: [f32; 3],
    across: [f32; 3],
    aspect: f32,
    bend_deg: f32,
) {
    let normal = math::unit(cross(across, direction));
    let width = leaf.size * aspect * 0.5;
    let base = leaf.position;
    let tip = math::add(base, math::scale(direction, leaf.size));
    let bend_angle = bend_deg * core::f32::consts::PI / 180.0;
    let (bend_sine, bend_cosine) = math::sin_cos(bend_angle);
    let bend = math::scale(
        normal,
        leaf.size * bend_sine / bend_cosine.abs().max(0.01) * 0.15,
    );
    let positions = [
        math::sub(base, math::scale(across, width)),
        math::add(base, math::scale(across, width)),
        math::add(tip, math::add(math::scale(across, width * 0.1), bend)),
        math::add(tip, math::add(math::scale(across, -width * 0.1), bend)),
    ];
    let node = leaf.node as usize;
    let parent = skeleton.parent.get(node).copied().unwrap_or(u32::MAX) as usize;
    let pivot = skeleton
        .position
        .get(parent)
        .copied()
        .unwrap_or(leaf.position);
    let phase = skeleton
        .phase
        .get(node)
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let start = mesh.vertices.len() as u32;
    for (index, position) in positions.into_iter().enumerate() {
        mesh.vertices.push(Vertex {
            position,
            normal,
            uv: [[0.0, 1.0, 1.0, 0.0][index], [0.0, 0.0, 1.0, 1.0][index]],
            tangent: [across[0], across[1], across[2], 1.0],
            wind: WindData {
                pivot,
                phase,
                trunk: (position[1] / skeleton.height().max(0.001)).clamp(0.0, 1.0),
                branch: 0.0,
                leaf: 1.0,
                edge: if index < 2 { 0.0 } else { 1.0 },
            },
        });
    }
    mesh.indices
        .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
}
