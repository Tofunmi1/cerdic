//! Refusals outside the polygon operators tell the caller what to change: budget, field, group,
//! rig, animation, curve and profile refusals each carry a suggestion naming the fix.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helpers unwrap fixtures that are known to be valid and panic when a program does not refuse"
)]

use ve_graph::{
    CommandBatch, Diagnostic, Outcome, Session, TransactionId, builtin_registry, parse_text,
};
use ve_modeling::{Evaluator, Quality};

/// The first refusal a program produces, with its code and suggestion. A program refused while
/// reading or applying is read there; one that applies is evaluated at Preview quality.
fn refusal(text: &str) -> (String, Option<String>) {
    let batch = match parse_text(&builtin_registry(), text) {
        Ok(batch) => batch,
        Err(diagnostics) => return first(diagnostics),
    };
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_suggestions_more").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { diagnostics } = outcome {
        return first(diagnostics);
    }
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let error = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .err()
        .unwrap_or_else(|| panic!("the program did not refuse:\n{text}"));
    (error.code.clone(), error.suggestion.clone())
}

fn first(diagnostics: Vec<Diagnostic>) -> (String, Option<String>) {
    let error = diagnostics.into_iter().next().unwrap();
    (error.code, error.suggestion)
}

/// Asserts the program refuses with `code` and a suggestion naming `keyword`.
fn assert_refusal(text: &str, code: &str, keyword: &str) {
    let (got_code, suggestion) = refusal(text);
    assert_eq!(got_code, code, "unexpected code for:\n{text}");
    let suggestion = suggestion.unwrap_or_else(|| panic!("no suggestion for {code} on:\n{text}"));
    assert!(
        suggestion.to_lowercase().contains(&keyword.to_lowercase()),
        "suggestion {suggestion:?} does not name {keyword:?} for {code} on:\n{text}"
    );
}

#[test]
fn a_solid_over_its_budget_names_the_mesh_detail() {
    assert_refusal(
        "t = solid.torus@1(major=0.5, minor=0.2, segments=512, tube_segments=256)\noutput t\n",
        "budget.triangles",
        "segments",
    );
}

#[test]
fn a_field_mesh_over_its_cell_budget_names_edge_length() {
    assert_refusal(
        "f = field.sphere@1(radius=0.5)\n\
         m = field.to_solid@1(f, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.002)\n\
         output m\n",
        "budget.field_cells",
        "edge_length",
    );
}

#[test]
fn an_unknown_parameter_lists_the_known_ones() {
    assert_refusal(
        "b = solid.box@1(size=[1, 1, 1], wat=1)\noutput b\n",
        "parse.unknown_param",
        "size",
    );
}

#[test]
fn a_missing_input_names_the_port_to_link() {
    assert_refusal(
        "outer = profile.circle@1(radius=0.2, segments=16)\n\
         w = check.min_wall@1(outer, min=0.01)\n",
        "graph.missing_input",
        "inner",
    );
}

#[test]
fn a_wrong_input_type_names_what_the_port_accepts() {
    assert_refusal(
        "p = profile.circle@1(radius=0.2, segments=16)\n\
         u = solid.union@1(p, p)\n\
         output u\n",
        "graph.port_type",
        "solid",
    );
}

#[test]
fn a_group_input_with_nothing_linked_names_it() {
    assert_refusal(
        "group bracket\n\
         \x20 mount = group.input@1(name=\"mount\")\n\
         \x20 plate = solid.box@1(size=[0.1, 0.1, 0.1])\n\
         \x20 joined = solid.union@1(mount, plate)\n\
         \x20 result joined\n\
         end\n\
         b = bracket()\n\
         output b\n",
        "graph.missing_input",
        "mount",
    );
}

#[test]
fn too_many_bones_names_the_bones_parameter() {
    let bones: Vec<String> = (0..33).map(|i| format!("b{i}")).collect();
    assert_refusal(
        &format!(
            "a = solid.box@1(size=[1, 1, 1])\n\
             b = rig.bone@1(a, head=[0, 0, 0], tail=[0, 1, 0])\n\
             s = rig.skin@1(b, bones=\"{}\")\n\
             output s\n",
            bones.join("; ")
        ),
        "param.range",
        "bones",
    );
}

#[test]
fn an_invalid_clip_name_names_the_rule() {
    assert_refusal(
        "a = solid.box@1(size=[1, 1, 1])\n\
         t = anim.track@1(a, keys=\"0:0,0,0; 1:0,1,0\", clip=\"Bad Name\")\n\
         output t\n",
        "anim.clip_name",
        "lowercase",
    );
}

#[test]
fn a_closed_bezier_that_does_not_close_names_the_fix() {
    assert_refusal(
        "c = curve.bezier@1(points=[[0, 0, 0], [0, 0, 0.5], [0.5, 0, 0.5], [1, 0, 0]], steps=16, closed=true)\n\
         p = profile.circle@1(radius=0.05, segments=12)\n\
         s = solid.sweep@1(curve=c, profile=p)\n\
         output s\n",
        "geometry.invalid_curve",
        "closed",
    );
}

#[test]
fn a_repeated_curve_point_names_the_points() {
    assert_refusal(
        "c = curve.polyline@1(points=[[0, 0, 0], [1, 0, 0], [1, 0, 0], [2, 0, 0]])\n\
         p = profile.circle@1(radius=0.05, segments=12)\n\
         s = solid.sweep@1(curve=c, profile=p)\n\
         output s\n",
        "geometry.degenerate_curve",
        "points",
    );
}

#[test]
fn loft_sections_with_different_point_counts_name_segments() {
    assert_refusal(
        "a = profile.circle@1(radius=0.2, segments=16)\n\
         b = profile.circle@1(radius=0.1, segments=24)\n\
         l = solid.loft@1(a, b, spacing=0.2)\n\
         output l\n",
        "surface.incompatible_sections",
        "segments",
    );
}

#[test]
fn a_loft_section_with_no_loop_names_the_profile() {
    assert_refusal(
        "outer = profile.circle@1(radius=0.3, segments=24)\n\
         same = profile.circle@1(radius=0.3, segments=24)\n\
         empty = profile.difference@1(outer, same)\n\
         other = profile.circle@1(radius=0.2, segments=24)\n\
         l = solid.loft@1(empty, other, spacing=0.2)\n\
         output l\n",
        "surface.degenerate_section",
        "profile",
    );
}

#[test]
fn a_sweep_profile_that_is_not_convex_names_the_profile() {
    assert_refusal(
        "c = curve.polyline@1(points=[[0, 0, 0], [0, 0, 1]])\n\
         p = profile.polygon@1(points=[[-0.2, -0.2], [0.2, -0.2], [0.2, 0.2], [0, 0], [-0.2, 0.2]])\n\
         s = solid.sweep@1(curve=c, profile=p)\n\
         output s\n",
        "geometry.unsupported_profile",
        "profile",
    );
}

#[test]
fn a_sweep_that_curves_too_tightly_names_the_profile() {
    assert_refusal(
        "c = curve.polyline@1(points=[[0.2, 0, 0], [0, 0.2, 0], [-0.2, 0, 0]])\n\
         p = profile.circle@1(radius=0.3, segments=16)\n\
         s = solid.sweep@1(curve=c, profile=p)\n\
         output s\n",
        "curve.sweep_curvature",
        "profile",
    );
}

#[test]
fn a_scatter_that_cannot_place_names_the_parameters() {
    assert_refusal(
        "b = solid.box@1(size=[0.1, 0.1, 0.1])\n\
         s = inst.scatter@1(b, count=60, width=3, depth=3, min_spacing=2)\n\
         r = inst.realize@1(s)\n\
         output r\n",
        "scatter.unsatisfiable",
        "min_spacing",
    );
}

#[test]
fn too_many_placements_names_the_counts() {
    assert_refusal(
        "b = solid.box@1(size=[0.1, 0.1, 0.1])\n\
         s = inst.scatter@1(b, count=16384, width=20, depth=20)\n\
         a = inst.array@1(s, count_x=2, spacing=[0, 0, 0])\n\
         r = inst.realize@1(a)\n\
         output r\n",
        "budget.instances",
        "count",
    );
}

#[test]
fn a_wall_below_the_minimum_names_the_wall() {
    assert_refusal(
        "outer = profile.circle@1(radius=0.2, segments=24)\n\
         inner = profile.circle@1(radius=0.195, segments=24)\n\
         w = check.min_wall@1(outer, inner, min=0.02)\n",
        "constraint.min_wall",
        "wall",
    );
}
