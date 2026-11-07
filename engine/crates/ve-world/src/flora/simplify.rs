//! Levels of detail for a grown plant, made by pruning the skeleton itself rather than dropping triangles of the finished mesh (which
//! leaves holes in the trunk): each level cuts the thinnest twigs, drops every second ring along the plain stretches of a branch, and
//! keeps a fraction of the leaves at a larger size so the crown keeps its coverage.
use super::{Leaf, Skeleton};

/// The skeleton at a level of detail: level 0 is the plant as grown, each level after it about half the weight of the one before.
#[must_use]
pub fn simplify(skeleton: &Skeleton, level: u32) -> Skeleton {
    if level == 0 || skeleton.node_count() < 3 {
        return skeleton.clone();
    }
    let count = skeleton.node_count();
    let root_radius = skeleton.radius[0];
    let level_f = f32::from(u16::try_from(level.min(8)).unwrap_or(8));
    let cut = root_radius * if level <= 2 { 0.02 } else { 0.01 } * level_f.exp2();
    let mut children = vec![0_u32; count];
    for node in 1..count {
        let parent = skeleton.parent[node] as usize;
        if skeleton.radius[node] >= cut {
            children[parent] += 1;
        }
    }
    // keep a node when it is thick enough, and thin out the plain stretches (one child, same branch) by dropping every second one
    let mut keep = vec![false; count];
    keep[0] = true;
    for node in 1..count {
        let parent = skeleton.parent[node] as usize;
        let thick = skeleton.radius[node] >= cut;
        let plain = children[node] == 1 && skeleton.branch[node] == skeleton.branch[parent];
        keep[node] = thick && !(plain && node % 2 == 1);
    }
    let mut ancestor = vec![0_u32; count];
    let mut renumber = vec![u32::MAX; count];
    let mut out = Skeleton::default();
    for node in 0..count {
        ancestor[node] = if keep[node] {
            u32::try_from(node).unwrap_or(0)
        } else {
            ancestor[skeleton.parent[node] as usize]
        };
        if !keep[node] {
            continue;
        }
        renumber[node] = u32::try_from(out.node_count()).unwrap_or(u32::MAX);
        let parent = if node == 0 {
            u32::MAX
        } else {
            renumber[ancestor[skeleton.parent[node] as usize] as usize]
        };
        out.position.push(skeleton.position[node]);
        out.radius.push(skeleton.radius[node]);
        out.parent.push(parent);
        out.depth.push(skeleton.depth[node]);
        out.wind.push(skeleton.wind[node]);
        out.phase.push(skeleton.phase[node]);
        out.stiffness.push(skeleton.stiffness[node]);
        out.branch.push(skeleton.branch[node]);
    }
    let every = 1_usize << level.min(6);
    let grow = (every as f32).sqrt();
    for (index, leaf) in skeleton.leaves.iter().enumerate() {
        if index % every != 0 {
            continue;
        }
        let kept = ancestor[leaf.node as usize] as usize;
        out.leaves.push(Leaf {
            size: leaf.size * grow,
            node: renumber[kept],
            ..*leaf
        });
    }
    out
}
