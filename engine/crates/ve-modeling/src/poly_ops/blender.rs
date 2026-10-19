//! The polygon nodes that run Blender's bmesh operators: inset, extrude, solidify, poke, wireframe,
//! subdivide, dissolve and symmetrize.

use ve_geometry::{MeshOp, PokeCenter, PolyMesh, Topology};
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::errors::poly_fail;
use super::params::{flag, number, text, vector};
use super::selection::{Elements, Selection};
use crate::Fail;

/// The families this module runs.
pub(super) fn handles(family: &str) -> bool {
    matches!(
        family,
        "poly.inset_region"
            | "poly.extrude_region"
            | "poly.solidify"
            | "poly.poke"
            | "poly.wireframe"
            | "poly.subdivide_edges"
            | "poly.dissolve_limit"
            | "poly.symmetrize"
            | "poly.unwrap_uv"
            | "poly.pack_uv"
            | "poly.decimate"
            | "poly.unsubdivide"
            | "poly.triangulate"
            | "poly.remove_doubles"
            | "poly.bisect"
            | "poly.spin"
            | "poly.mirror"
            | "poly.loop_cut"
    )
}

/// Whether the family needs no selection at all.
pub(super) fn ignores_selection(family: &str) -> bool {
    matches!(
        family,
        "poly.dissolve_limit"
            | "poly.symmetrize"
            | "poly.decimate"
            | "poly.unsubdivide"
            | "poly.triangulate"
            | "poly.remove_doubles"
            | "poly.bisect"
            | "poly.mirror"
            | "poly.loop_cut"
            | "poly.pack_uv"
    )
}

/// Whether the family takes a selection but runs on the whole mesh without one.
pub(super) fn selection_optional(family: &str) -> bool {
    matches!(
        family,
        "poly.solidify" | "poly.poke" | "poly.wireframe" | "poly.unwrap_uv" | "poly.spin"
    )
}

pub(super) fn apply(
    node: &Node,
    family: &str,
    poly: &PolyMesh,
    selection: Option<&Selection>,
    context: &ExecutionContext,
) -> Result<PolyMesh, Fail> {
    let (faces, edges) = chosen(node, family, selection)?;
    let vertices = chosen_vertices(node, family, selection)?;
    if family == "poly.loop_cut" {
        check_loop_cut(node, poly, context)?;
    }
    if family == "poly.unwrap_uv" {
        return unwrap(node, poly, &edges, context);
    }
    if family == "poly.pack_uv" {
        if !poly.attributes().contains_key("u") || !poly.attributes().contains_key("v") {
            return Err(super::errors::refuse(
                node,
                "poly.uv_missing",
                "corner UVs are missing; run poly.unwrap_uv first".to_owned(),
            ));
        }
        let options = ve_kernel::UvPackOptions {
            rotate: flag(node, "rotate")?,
            keep_density: flag(node, "keep_density")?,
            shape: match text(node, "shape")? {
                "convex" => ve_kernel::UvPackShape::Convex,
                _ => ve_kernel::UvPackShape::Aabb,
            },
            margin: number(node, "margin")? as f32,
        };
        return ve_geometry::pack_uv(poly, &options, context)
            .map_err(|error| poly_fail(node, &error, Some(poly)));
    }
    if family == "poly.bisect" {
        let normal = vector(node, "normal")?;
        if normal.iter().all(|v| *v == 0.0) {
            return Err(super::errors::refuse(
                node,
                "poly.bisect_normal_zero",
                "normal must be non-zero".to_owned(),
            ));
        }
        let point = vector(node, "point")?;
        let keep = text(node, "keep")?;
        let dot = |p: [f32; 3]| {
            (f64::from(p[0]) - point[0]) * normal[0]
                + (f64::from(p[1]) - point[1]) * normal[1]
                + (f64::from(p[2]) - point[2]) * normal[2]
        };
        let has_kept = poly.positions().iter().any(|p| match keep {
            "above" => dot(*p) >= 0.0,
            "below" => dot(*p) <= 0.0,
            _ => true,
        });
        if !has_kept {
            return Err(super::errors::refuse(
                node,
                "poly.bisect_empty",
                "plane removes the entire mesh".to_owned(),
            ));
        }
    }
    if family == "poly.spin" && vector(node, "axis")?.iter().all(|value| *value == 0.0) {
        return Err(super::errors::refuse(
            node,
            "poly.spin_axis_zero",
            "axis must be non-zero".to_owned(),
        ));
    }
    if family == "poly.mirror" && vector(node, "normal")?.iter().all(|value| *value == 0.0) {
        return Err(super::errors::refuse(
            node,
            "poly.mirror_normal_zero",
            "normal must be non-zero".to_owned(),
        ));
    }
    let op = operator(node, family)?;
    ve_geometry::blender_edit(poly, &faces, &edges, &vertices, &op, context)
        .map_err(|error| poly_fail(node, &error, Some(poly)))
}

fn check_loop_cut(node: &Node, poly: &PolyMesh, context: &ExecutionContext) -> Result<(), Fail> {
    if poly.face_sizes().iter().any(|size| *size != 4) {
        return Err(super::errors::refuse(
            node,
            "poly.loop_cut_non_quad",
            "the ring must run through quads only; merge triangles with poly.merge_coplanar first"
                .to_owned(),
        ));
    }
    let at = vector(node, "at")?;
    let topology =
        Topology::build(poly, context).map_err(|error| poly_fail(node, &error, Some(poly)))?;
    let nearest = topology
        .edges()
        .keys()
        .map(|&(a, b)| {
            let p = poly.positions()[a as usize];
            let q = poly.positions()[b as usize];
            (0..3)
                .map(|axis| {
                    let delta = (f64::from(p[axis]) + f64::from(q[axis])) * 0.5 - at[axis];
                    delta * delta
                })
                .sum::<f64>()
        })
        .fold(f64::INFINITY, f64::min);
    let diagonal = (0..3)
        .map(|axis| {
            let (low, high) = poly
                .positions()
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                    (lo.min(p[axis]), hi.max(p[axis]))
                });
            f64::from(high - low).powi(2)
        })
        .sum::<f64>();
    if nearest > diagonal {
        return Err(super::errors::refuse(
            node,
            "poly.loop_cut_at_far",
            "at is farther from every mesh edge than the bounding diagonal".to_owned(),
        ));
    }
    Ok(())
}

/// `poly.unwrap_uv`: the selected edges are seams.
fn unwrap(
    node: &Node,
    poly: &PolyMesh,
    seams: &[(u32, u32)],
    context: &ExecutionContext,
) -> Result<PolyMesh, Fail> {
    let options = ve_geometry::UvUnwrap {
        method: match text(node, "method")? {
            "angle" => ve_geometry::UvMethod::AngleBased,
            _ => ve_geometry::UvMethod::Conformal,
        },
        pack: flag(node, "pack")?,
        rotate: flag(node, "rotate")?,
        margin: number(node, "margin")?,
    };
    // With no seams selected, cut automatically: sharp edges, and the paths that open closed or tube-shaped pieces.
    let automatic;
    let seams = if seams.is_empty() && flag(node, "auto_seams")? {
        automatic = ve_geometry::auto_seams(poly, number(node, "seam_angle")?, context)
            .map_err(|error| poly_fail(node, &error, Some(poly)))?;
        &automatic[..]
    } else {
        seams
    };
    ve_geometry::unwrap_uv(poly, seams, &options, context)
        .map_err(|error| poly_fail(node, &error, Some(poly)))
}

type Chosen = (Vec<usize>, Vec<(u32, u32)>);

fn chosen_vertices(
    node: &Node,
    family: &str,
    selection: Option<&Selection>,
) -> Result<Vec<u32>, Fail> {
    if family != "poly.remove_doubles" {
        return Ok(Vec::new());
    }
    match selection.map(|selection| &selection.elements) {
        None => Ok(Vec::new()),
        Some(Elements::Vertices(items)) => Ok(items.clone()),
        Some(other) => Err(Box::new(
            ve_graph::Diagnostic::error(
                "poly.selection_kind",
                format!(
                    "{}: needs a selection of vertices but got {}s",
                    node.op,
                    other.kind()
                ),
            )
            .at_node(&node.id),
        )),
    }
}

/// The faces or edges the family works on, from its selection.
fn chosen(node: &Node, family: &str, selection: Option<&Selection>) -> Result<Chosen, Fail> {
    let wrong = |wanted: &str, got: &str| -> Fail {
        Box::new(
            ve_graph::Diagnostic::error(
                "poly.selection_kind",
                format!(
                    "{}: needs a selection of {wanted}s but got {got}s (set element on poly.select)",
                    node.op
                ),
            )
            .at_node(&node.id),
        )
    };
    let Some(selection) = selection else {
        return Ok((Vec::new(), Vec::new()));
    };
    if family == "poly.remove_doubles" && matches!(&selection.elements, Elements::Vertices(_)) {
        return Ok((Vec::new(), Vec::new()));
    }
    match (&selection.elements, family) {
        (Elements::Edges(items), "poly.subdivide_edges" | "poly.unwrap_uv" | "poly.spin") => {
            Ok((Vec::new(), items.clone()))
        }
        (other, "poly.subdivide_edges" | "poly.unwrap_uv") => Err(wrong("edge", other.kind())),
        (Elements::Faces(items), _) => Ok((items.clone(), Vec::new())),
        (other, _) => Err(wrong("face", other.kind())),
    }
}

/// The operator and its parameters for a node.
fn operator(node: &Node, family: &str) -> Result<MeshOp, Fail> {
    if matches!(
        family,
        "poly.bisect" | "poly.spin" | "poly.mirror" | "poly.loop_cut"
    ) {
        return advanced_operator(node, family);
    }
    Ok(match family {
        "poly.inset_region" => MeshOp::Inset {
            thickness: number(node, "thickness")?,
            depth: number(node, "depth")?,
            even: flag(node, "even")?,
            relative: flag(node, "relative")?,
            boundary: flag(node, "boundary")?,
            outset: flag(node, "outset")?,
        },
        "poly.extrude_region" => MeshOp::Extrude {
            offset: vector(node, "offset")?,
        },
        "poly.solidify" => MeshOp::Solidify {
            thickness: number(node, "thickness")?,
        },
        "poly.poke" => MeshOp::Poke {
            offset: number(node, "offset")?,
            center: match text(node, "center")? {
                "weighted" => PokeCenter::Weighted,
                "bounds" => PokeCenter::Bounds,
                _ => PokeCenter::Median,
            },
            relative: flag(node, "relative")?,
        },
        "poly.wireframe" => MeshOp::Wireframe {
            thickness: number(node, "thickness")?,
            offset: number(node, "offset")?,
            even: flag(node, "even")?,
            relative: flag(node, "relative")?,
            boundary: flag(node, "boundary")?,
            replace: flag(node, "replace")?,
        },
        "poly.subdivide_edges" => MeshOp::Subdivide {
            cuts: super::params::count(node, "cuts")?,
            smooth: number(node, "smooth")?,
            grid_fill: flag(node, "grid_fill")?,
        },
        "poly.dissolve_limit" => MeshOp::Dissolve {
            angle: number(node, "angle")?,
            boundaries: flag(node, "boundaries")?,
        },
        "poly.decimate" => MeshOp::Decimate {
            ratio: number(node, "ratio")?,
            symmetry: match text(node, "symmetry")? {
                "x" => Some(0),
                "y" => Some(1),
                "z" => Some(2),
                _ => None,
            },
            triangulate: flag(node, "triangulate")?,
        },
        "poly.unsubdivide" => MeshOp::Unsubdivide {
            iterations: super::params::count(node, "iterations")?,
        },
        "poly.triangulate" => MeshOp::Triangulate,
        "poly.remove_doubles" => MeshOp::RemoveDoubles {
            distance: number(node, "distance")?,
        },
        _ => MeshOp::Symmetrize {
            axis: match text(node, "axis")? {
                "y" => 1,
                "z" => 2,
                _ => 0,
            },
            keep_positive: text(node, "keep")? == "positive",
            distance: number(node, "distance")?,
        },
    })
}

fn advanced_operator(node: &Node, family: &str) -> Result<MeshOp, Fail> {
    Ok(match family {
        "poly.bisect" => {
            let normal = vector(node, "normal")?;
            let keep = text(node, "keep")?;
            MeshOp::Bisect {
                point: vector(node, "point")?,
                normal,
                distance: number(node, "distance")?,
                clear_inner: keep == "above",
                clear_outer: keep == "below",
                fill: flag(node, "fill")?,
            }
        }
        "poly.spin" => MeshOp::Spin {
            center: vector(node, "center")?,
            axis: vector(node, "axis")?,
            angle: number(node, "angle")?,
            steps: super::params::count(node, "steps")?,
            merge: flag(node, "merge")?,
            distance: number(node, "distance")?,
        },
        "poly.mirror" => MeshOp::Mirror {
            point: vector(node, "point")?,
            normal: vector(node, "normal")?,
            merge: flag(node, "merge")?,
            distance: number(node, "distance")?,
        },
        _ => return loop_cut_operator(node),
    })
}

fn loop_cut_operator(node: &Node) -> Result<MeshOp, Fail> {
    Ok(MeshOp::LoopCut {
        at: vector(node, "at")?,
        cuts: super::params::count(node, "cuts")?,
        smooth: number(node, "smooth")?,
    })
}
