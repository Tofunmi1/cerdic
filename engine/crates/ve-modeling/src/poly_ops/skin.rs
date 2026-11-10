//! `poly.skin`: a skeleton of named joints with radii becomes one closed surface (Blender's Skin modifier through the kernel).

use std::collections::HashMap;

use ve_geometry::{AttrData, Attribute, Domain, PolyMesh};
use ve_graph::{Diagnostic, Node};
use ve_kernel::{ExecutionContext, KernelError, SkinJoint, SkinOptions, skin_graph};

use super::{convert::value_from_poly, errors::poly_fail, params::{flag, number, text}};
use crate::{Fail, cache::Value};

fn refuse(node: &Node, code: &str, message: String, suggestion: &str) -> Fail {
    Box::new(Diagnostic::error(code, message).suggest(suggestion).at_node(&node.id))
}

/// `name x y z rx rz [root]` entries separated by `;`, in order.
fn parse_joints(node: &Node, source: &str) -> Result<(Vec<String>, Vec<SkinJoint>), Fail> {
    let mut names = Vec::new();
    let mut joints = Vec::new();
    for (index, entry) in source.split(';').map(str::trim).filter(|e| !e.is_empty()).enumerate() {
        let words: Vec<&str> = entry.split_whitespace().collect();
        let fix = "write each joint as `name x y z rx rz`, with the word `root` after the numbers on one joint of each piece";
        if words.len() < 6 || words.len() > 7 || (words.len() == 7 && words[6] != "root") {
            return Err(refuse(node, "skin.joint_syntax", format!("joint {index} (`{entry}`) is not `name x y z rx rz [root]`"), fix));
        }
        let mut numbers = [0.0_f32; 5];
        for (slot, word) in numbers.iter_mut().zip(&words[1..6]) {
            *slot = word.parse::<f32>().map_err(|_| {
                refuse(node, "skin.joint_syntax", format!("joint `{}` has `{word}` where a number is expected", words[0]), fix)
            })?;
        }
        if names.iter().any(|n| n == words[0]) {
            return Err(refuse(node, "skin.joint_duplicate", format!("two joints are named `{}`", words[0]), "give every joint its own name"));
        }
        names.push(words[0].to_owned());
        joints.push(SkinJoint { position: [numbers[0], numbers[1], numbers[2]], radius: [numbers[3], numbers[4]], root: words.len() == 7, loose: false });
    }
    if joints.len() < 2 {
        return Err(refuse(node, "skin.too_few_joints", "a skeleton needs at least two joints".to_owned(), "add joints and a bone between them"));
    }
    Ok((names, joints))
}

fn parse_bones(node: &Node, source: &str, names: &[String]) -> Result<Vec<[u32; 2]>, Fail> {
    let lookup: HashMap<&str, u32> = names.iter().enumerate().map(|(i, n)| (n.as_str(), u32::try_from(i).unwrap_or(u32::MAX))).collect();
    let mut bones = Vec::new();
    for entry in source.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let Some((a, b)) = entry.split_once('-') else {
            return Err(refuse(node, "skin.bone_syntax", format!("bone `{entry}` is not `a-b`"), "write each bone as two joint names joined by `-`"));
        };
        let find = |name: &str| {
            lookup.get(name.trim()).copied().ok_or_else(|| {
                refuse(node, "skin.bone_unknown_joint", format!("bone `{entry}` names `{}`, which is not a joint", name.trim()), "use a name from `joints`")
            })
        };
        bones.push([find(a)?, find(b)?]);
    }
    if bones.is_empty() {
        return Err(refuse(node, "skin.no_bones", "a skeleton needs at least one bone".to_owned(), "write bones as `a-b; b-c`"));
    }
    Ok(bones)
}

/// Every connected piece of the skeleton needs exactly one root joint to orient its bone frames; the first piece without one is named.
fn check_roots(node: &Node, names: &[String], joints: &[SkinJoint], bones: &[[u32; 2]]) -> Result<(), Fail> {
    let mut parent: Vec<usize> = (0..joints.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for [a, b] in bones {
        let (ra, rb) = (find(&mut parent, *a as usize), find(&mut parent, *b as usize));
        parent[ra] = rb;
    }
    let mut roots: HashMap<usize, u32> = HashMap::new();
    let mut touched: HashMap<usize, usize> = HashMap::new();
    let used: std::collections::BTreeSet<usize> = bones.iter().flat_map(|[a, b]| [*a as usize, *b as usize]).collect();
    for joint in used {
        let piece = find(&mut parent, joint);
        touched.entry(piece).or_insert(joint);
        if joints[joint].root {
            *roots.entry(piece).or_default() += 1;
        }
    }
    for (piece, first) in &touched {
        match roots.get(piece).copied().unwrap_or(0) {
            0 => {
                return Err(refuse(
                    node,
                    "skin.no_root",
                    format!("the piece of the skeleton containing `{}` has no root joint", names[*first]),
                    "put the word `root` after the numbers of one joint in every connected piece (the pelvis, for a body)",
                ));
            }
            1 => {}
            _ => {
                return Err(refuse(
                    node,
                    "skin.many_roots",
                    format!("the piece of the skeleton containing `{}` has more than one root joint", names[*first]),
                    "keep `root` on one joint per connected piece",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn run(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let (names, joints) = parse_joints(node, text(node, "joints")?)?;
    let bones = parse_bones(node, text(node, "bones")?, &names)?;
    check_roots(node, &names, &joints, &bones)?;
    let symmetry = text(node, "symmetry")?;
    let axes = (u32::from(symmetry.contains('x'))) | (u32::from(symmetry.contains('y')) << 1) | (u32::from(symmetry.contains('z')) << 2);
    let options = SkinOptions {
        #[expect(clippy::cast_possible_truncation, reason = "a ratio between 0 and 1")]
        branch_smoothing: number(node, "branch_smoothing")? as f32,
        symmetry_axes: axes,
        subdivide: flag(node, "subdivide")?,
    };
    let skin = skin_graph(&joints, &bones, &options).map_err(|error: KernelError| {
        refuse(node, "kernel.invalid_input", format!("poly.skin: {error}"), "check radii are zero or more, every bone joins two different joints, and the skeleton has at most 4,096 joints")
    })?;
    let mut mesh = PolyMesh::new(skin.positions.clone(), skin.face_sizes.clone(), skin.corners.clone()).map_err(|e| poly_fail(node, &e, None))?;
    let owner: Vec<i32> = skin.vertex_origin.iter().map(|o| i32::try_from(*o).unwrap_or(-1)).collect();
    mesh.set_attribute("joint", Attribute { domain: Domain::Point, data: AttrData::I32(owner) }).map_err(|e| poly_fail(node, &e, None))?;
    // The quads of a skin are slightly bent, so they are cut along their curved diagonal rather than refused as non-planar.
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    value_from_poly(node, mesh, None, context)
}
