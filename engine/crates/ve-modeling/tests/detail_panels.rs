//! `detail.panels`: a grid of raised plates, some thick, some vented, laid on one face of a solid: the panel lines and greebles that make a
//! hull, a bulkhead or a machine read as a built thing. A separate solid that sits on the face, so it can be joined, textured or kept apart.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(13);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_panels").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

/// A wall 4 wide, 3 high and 0.2 thick at the origin, and a panels node on it.
fn panels(params: &str) -> Result<Evaluation, String> {
    evaluate(&format!(
        "wall = solid.box@1(size=[4, 3, 0.2])\np = detail.panels@1(wall{params})\noutput p = p\n"
    ))
}

fn bounds(evaluation: &Evaluation) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &evaluation.parts[0].positions {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(p[axis]);
            hi[axis] = hi[axis].max(p[axis]);
        }
    }
    (lo, hi)
}

fn triangles(evaluation: &Evaluation) -> usize {
    evaluation.parts[0].indices.len() / 3
}

const E: f32 = 1e-4;

#[test]
fn plates_cover_the_face_and_sit_on_it() {
    let ev = panels(", face=\"+z\", cols=4, rows=3, gap=0.02, depth=0.04").unwrap();
    let (lo, hi) = bounds(&ev);
    // The +z face of the wall is the plane z = 0.1, spanning x in -2..2 and y in -1.5..1.5.
    assert!(
        lo[0] >= -2.0 - E && hi[0] <= 2.0 + E,
        "x {} .. {}",
        lo[0],
        hi[0]
    );
    assert!(
        lo[1] >= -1.5 - E && hi[1] <= 1.5 + E,
        "y {} .. {}",
        lo[1],
        hi[1]
    );
    assert!(
        (lo[2] - 0.1).abs() < E,
        "the plates start on the face, not {} away",
        lo[2] - 0.1
    );
    assert!(hi[2] > 0.1 + 0.04 - E, "plates are at least `depth` thick");
    assert!(
        hi[2] <= 0.1 + 3.0 * 0.04 + E,
        "no plate is thicker than three times `depth`"
    );
    assert!(ev.parts[0].volume > 0.0);
}

#[test]
fn the_gap_between_plates_is_empty() {
    let ev =
        panels(", face=\"+z\", cols=4, rows=3, gap=0.1, depth=0.04, raised=0, vents=0").unwrap();
    for p in &ev.parts[0].positions {
        for seam in [-1.0_f32, 0.0, 1.0] {
            assert!(
                (p[0] - seam).abs() > 0.05 - E,
                "a vertex at x={} lies in the seam at {seam}",
                p[0]
            );
        }
        for seam in [-0.5_f32, 0.5] {
            assert!(
                (p[1] - seam).abs() > 0.05 - E,
                "a vertex at y={} lies in the seam at {seam}",
                p[1]
            );
        }
    }
}

#[test]
fn flat_plates_are_all_depth_thick_and_raised_ones_stand_out() {
    let flat = panels(", cols=6, rows=4, depth=0.05, raised=0, vents=0").unwrap();
    let (_, hi) = bounds(&flat);
    assert!(
        (hi[2] - (0.1 + 0.05)).abs() < E,
        "flat plates top out at depth: {}",
        hi[2] - 0.1
    );
    let tall = panels(", cols=6, rows=4, depth=0.05, raised=1, vents=0").unwrap();
    let (lo, hi) = bounds(&tall);
    assert!(
        hi[2] >= 0.1 + 1.5 * 0.05 - E,
        "raised plates are at least 1.5 times depth: {}",
        hi[2] - 0.1
    );
    assert!(hi[2] <= 0.1 + 3.0 * 0.05 + E);
    assert!((lo[2] - 0.1).abs() < E);
}

#[test]
fn vents_add_slats() {
    let plain = panels(", cols=4, rows=3, raised=0, vents=0").unwrap();
    let vented = panels(", cols=4, rows=3, raised=0, vents=1").unwrap();
    assert!(
        triangles(&vented) > triangles(&plain),
        "{} vs {}",
        triangles(&vented),
        triangles(&plain)
    );
}

#[test]
fn the_seed_decides_and_the_same_seed_repeats() {
    let a = panels(", cols=6, rows=4, raised=0.5, vents=0.3, seed=1").unwrap();
    let b = panels(", cols=6, rows=4, raised=0.5, vents=0.3, seed=1").unwrap();
    let c = panels(", cols=6, rows=4, raised=0.5, vents=0.3, seed=2").unwrap();
    assert_eq!(a.parts[0].positions, b.parts[0].positions);
    assert_ne!(a.parts[0].positions, c.parts[0].positions);
}

#[test]
fn every_face_of_the_box_works() {
    let faces = [
        ("+x", 0, 2.0, 1.0_f32),
        ("-x", 0, -2.0, -1.0),
        ("+y", 1, 1.5, 1.0),
        ("-y", 1, -1.5, -1.0),
        ("+z", 2, 0.1, 1.0),
        ("-z", 2, -0.1, -1.0),
    ];
    for (face, axis, plane, sign) in faces {
        let ev = panels(&format!(", face=\"{face}\", cols=3, rows=3, depth=0.04")).unwrap();
        let (lo, hi) = bounds(&ev);
        let (near, far) = if sign > 0.0 {
            (lo[axis], hi[axis])
        } else {
            (hi[axis], lo[axis])
        };
        assert!(
            (near - plane).abs() < E,
            "{face}: plates start at {near}, the face is at {plane}"
        );
        assert!(
            (far - plane) * sign > 0.04 - E,
            "{face}: plates stand out of the face"
        );
        assert!(
            (far - plane) * sign <= 3.0 * 0.04 + E,
            "{face}: no plate thicker than 3 depth"
        );
    }
}

#[test]
fn it_stays_within_a_triangle_budget() {
    let ev = panels(", cols=12, rows=8, raised=0.5, vents=0.5").unwrap();
    assert!(triangles(&ev) < 60_000, "{} triangles", triangles(&ev));
}

#[test]
fn bad_parameters_are_refused() {
    assert!(panels(", cols=0").is_err());
    assert!(panels(", rows=0").is_err());
    assert!(panels(", cols=33").is_err(), "more than 32 columns");
    assert!(panels(", rows=33").is_err(), "more than 32 rows");
    assert!(panels(", face=\"up\"").is_err());
    assert!(
        panels(", cols=4, gap=1.5").is_err(),
        "a gap wider than a plate leaves no plate"
    );
    assert!(panels(", raised=1.5").is_err());
    assert!(panels(", vents=-0.1").is_err());
    assert!(panels(", depth=0").is_err());
}

#[test]
fn whole_number_parameters_written_as_integers_work() {
    // The saved document writes 0 and 1 as integers: raised, vents and gap at those values must not be refused.
    assert!(panels(", raised=1, vents=0, depth=1, gap=0").is_ok());
}

#[test]
fn it_joins_the_wall_and_takes_a_surface() {
    let source = "wall = solid.box@1(size=[4, 3, 0.2])\n\
                  p = detail.panels@1(wall, cols=4, rows=3)\n\
                  tex = material.preset@1(p, name=\"brushed_steel\", tile=0.5, size=\"256\")\n\
                  joined = solid.union@1(wall, p)\n\
                  output tex = tex\noutput joined = joined\n";
    let ev = evaluate(source).unwrap();
    let tex = ev.parts.iter().find(|p| p.name == "tex").unwrap();
    assert!(tex.material.as_ref().unwrap().texture_program.is_some());
    let joined = ev.parts.iter().find(|p| p.name == "joined").unwrap();
    assert!(
        joined.volume > 4.0 * 3.0 * 0.2,
        "joined volume {}",
        joined.volume
    );
}

#[test]
fn attached_to_its_wall_for_the_floating_parts_check() {
    let source = "wall = solid.box@1(size=[4, 3, 0.2])\n\
                  p = detail.panels@1(wall, cols=4, rows=3)\n\
                  chk = check.attached@1(wall, p)\n\
                  output wall = wall\noutput p = p\n";
    let ev = evaluate(source).unwrap();
    assert!(
        !ev.warnings.iter().any(|w| w.code == "attach.floating"),
        "panels float off their wall: {:?}",
        ev.warnings
    );
}
