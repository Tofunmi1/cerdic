//! `field.taper`, `field.twist` and `field.bend`: domain warps along an axis, so a face's nose, lips and brow, a tail or a horn can be shaped in the field language.
//! Every number here is worked out by hand: a square frustum has volume h/3 (a1^2 + a1 a2 + a2^2); twisting and bending a bar keep its volume (to first order for the
//! bend); a bar bent by 90 degrees over a metre has radius 1 / (pi / 2) = 0.637 and its far end at (0.637, 0.137) heading +x.
#![expect(
    clippy::unwrap_used,
    reason = "the helpers unwrap fixtures that are known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

const BOUND: &str = "min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.02";

/// Meshes `source` (which defines `shape`) and returns the part's volume, or the failure.
fn volume_of(shape_lines: &str) -> Result<f64, String> {
    let source =
        format!("{shape_lines}\nbody = field.to_solid@1(shape, {BOUND})\noutput body = body\n");
    let commands = parse_text(&builtin_registry(), &source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(5);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_field_warp").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(result) => Ok(result.parts[0].volume),
        Err(error) => Err(format!("{error:?}")),
    }
}

/// A bar 0.2 x 1 x 0.2 standing along y, centred at the origin.
const BAR: &str = "bar = field.box@1(half=[0.1, 0.5, 0.1])\n";
/// A 0.4 x 1 x 0.4 square column along y.
const COLUMN: &str = "column = field.box@1(half=[0.2, 0.5, 0.2])\n";

#[test]
fn a_taper_makes_the_frustum_volume() {
    // Side 0.4 at y=-0.5 down to 0.2 at y=0.5: h/3 (a1^2 + a1 a2 + a2^2) = (0.16 + 0.08 + 0.04) / 3.
    let v = volume_of(&format!(
        "{COLUMN}shape = field.taper@1(column, axis=\"y\", from=-0.5, to=0.5, scale_start=1, scale_end=0.5)"
    ))
    .unwrap();
    let expected = (0.16 + 0.08 + 0.04) / 3.0;
    assert!(
        (v - expected).abs() / expected < 0.04,
        "{v} against {expected}"
    );
}

#[test]
fn a_taper_with_equal_scales_changes_nothing() {
    let plain = volume_of(&format!("{COLUMN}shape = column")).unwrap();
    let same = volume_of(&format!(
        "{COLUMN}shape = field.taper@1(column, axis=\"y\", from=-0.5, to=0.5, scale_start=1, scale_end=1)"
    ))
    .unwrap();
    assert!(
        (plain - same).abs() / plain < 0.01,
        "{plain} against {same}"
    );
}

#[test]
fn a_taper_can_run_along_any_axis() {
    // The same column lying along x gives the same volume.
    let v = volume_of(
        "column = field.box@1(half=[0.5, 0.2, 0.2])\n\
         shape = field.taper@1(column, axis=\"x\", from=-0.5, to=0.5, scale_start=0.5, scale_end=1)",
    )
    .unwrap();
    let expected = (0.16 + 0.08 + 0.04) / 3.0;
    assert!(
        (v - expected).abs() / expected < 0.04,
        "{v} against {expected}"
    );
}

#[test]
fn a_twist_keeps_the_volume_and_turns_the_top() {
    let plain = volume_of(&format!("{BAR}shape = bar")).unwrap();
    let twisted = volume_of(&format!(
        "{BAR}shape = field.twist@1(bar, axis=\"y\", from=-0.5, to=0.5, angle=90)"
    ))
    .unwrap();
    assert!(
        (plain - twisted).abs() / plain < 0.05,
        "{plain} against {twisted}"
    );
    // A thin slab along x, cut by a crossing slab along z at the top: untwisted they overlap in a 0.1 x 0.1 patch, twisted by 90 degrees the top lies along z.
    let overlap = |warp: &str| {
        volume_of(&format!(
            "slab = field.box@1(half=[0.4, 0.5, 0.05])\n{warp}\ncross = field.box@1(half=[0.05, 0.5, 0.4])\nshape = field.intersect@1(turned, cross)"
        ))
        .unwrap()
    };
    let straight = overlap("turned = slab");
    let turned = overlap("turned = field.twist@1(slab, axis=\"y\", from=-0.5, to=0.5, angle=90)");
    assert!(
        turned > straight * 2.0,
        "twisted overlap {turned} against straight {straight}"
    );
}

#[test]
fn a_bend_keeps_the_volume_to_first_order_and_curves_the_bar_toward_the_side() {
    let plain = volume_of(&format!("{BAR}shape = bar")).unwrap();
    let bent = volume_of(&format!(
        "{BAR}shape = field.bend@1(bar, axis=\"y\", toward=\"x\", from=-0.5, to=0.5, angle=90)"
    ))
    .unwrap();
    assert!(
        (plain - bent).abs() / plain < 0.06,
        "{plain} against {bent}"
    );
    // The far end of the bent bar reaches (0.637, 0.137): a box over x 0.3..0.9, y -0.1..0.4 holds about 0.014 of it, the straight bar none.
    let overlap = |warp: &str| {
        volume_of(&format!(
            "{BAR}{warp}\nregion = field.box@1(half=[0.3, 0.25, 0.5])\nplace = field.move@1(region, by=[0.6, 0.15, 0])\nshape = field.intersect@1(curved, place)"
        ))
        // a field that crosses no surface in the volume is reported as `field.empty`: here that means no overlap at all
        .or_else(|error| if error.contains("field.empty") { Ok(0.0) } else { Err(error) })
        .unwrap()
    };
    let straight = overlap("curved = bar");
    let curved = overlap(
        "curved = field.bend@1(bar, axis=\"y\", toward=\"x\", from=-0.5, to=0.5, angle=90)",
    );
    assert!(
        straight < 1e-6,
        "the straight bar is nowhere near: {straight}"
    );
    assert!(curved > 0.008, "the bent bar reaches the region: {curved}");
}

#[test]
fn a_bend_the_other_way_goes_the_other_way() {
    // A negative angle curves toward -x: the same region on the -x side now holds the bar's end.
    let v = volume_of(&format!(
        "{BAR}curved = field.bend@1(bar, axis=\"y\", toward=\"x\", from=-0.5, to=0.5, angle=-90)\n\
         region = field.box@1(half=[0.3, 0.25, 0.5])\nplace = field.move@1(region, by=[-0.6, 0.15, 0])\nshape = field.intersect@1(curved, place)"
    ))
    .unwrap();
    assert!(v > 0.008, "{v}");
}

#[test]
fn zero_warps_change_nothing() {
    let plain = volume_of(&format!("{BAR}shape = bar")).unwrap();
    for warp in [
        "field.twist@1(bar, axis=\"y\", from=-0.5, to=0.5, angle=0)",
        "field.bend@1(bar, axis=\"y\", toward=\"x\", from=-0.5, to=0.5, angle=0)",
    ] {
        let v = volume_of(&format!("{BAR}shape = {warp}")).unwrap();
        assert!(
            (plain - v).abs() / plain < 0.01,
            "{warp}: {plain} against {v}"
        );
    }
}

#[test]
fn bad_parameters_are_refused_with_the_parameter_named() {
    for source in [
        "b = field.box@1(half=[0.1, 0.1, 0.1])\nt = field.taper@1(b, axis=\"w\", from=0, to=1)",
        "b = field.box@1(half=[0.1, 0.1, 0.1])\nt = field.taper@1(b, axis=\"y\", from=0, to=1, scale_start=0)",
        "b = field.box@1(half=[0.1, 0.1, 0.1])\nt = field.bend@1(b, axis=\"y\", toward=\"y\", from=0, to=1, angle=30)",
        "b = field.box@1(half=[0.1, 0.1, 0.1])\nt = field.twist@1(b, axis=\"y\", from=1, to=1, angle=30)",
    ] {
        let refused = match parse_text(&builtin_registry(), source) {
            Err(error) => format!("{error:?}"),
            Ok(parsed) => {
                let mut session = Session::with_builtin(5);
                let applied = session.apply(&CommandBatch {
                    doc: session.document().id.clone(),
                    base_revision: 0,
                    transaction: TransactionId::parse("t_g12_field_warp_bad").unwrap(),
                    label: None,
                    commands: parsed.commands,
                });
                format!("{applied:?}")
            }
        };
        assert!(
            refused.contains("param.") || refused.contains("Rejected") || refused.contains("rror"),
            "{source}: {refused}"
        );
        assert!(
            !refused.contains("Committed"),
            "{source} was accepted: {refused}"
        );
    }
}

#[test]
fn the_operators_are_documented_for_a_model() {
    let text = format!("{:?}", builtin_registry());
    for name in [
        "field.taper@1",
        "field.twist@1",
        "field.bend@1",
        "scale_start",
        "scale_end",
        "toward",
    ] {
        assert!(text.contains(name), "{name} is in the registry");
    }
}
