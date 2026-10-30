//! Which parts of the level the creatures can walk on (ADR 0067): a `nav` line names the authored parts that are floor, stairs and ramps, and
//! the cell size and agent size to build the walkable grid with. `ve-game` only carries the declaration (`GameDef::nav`); the host builds the
//! `ve_nav::NavMesh` from those parts' triangles and gives it to the world. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::GameDef;

#[test]
fn a_nav_line_names_the_walkable_parts_and_the_agent() {
    let def = GameDef::parse(
        "game name=a\nnav parts=floor,stairs cell=0.2 radius=0.35 height=1.7 step=0.3 slope=40\n",
    )
    .unwrap();
    let nav = def.nav().expect("a nav declaration");
    assert_eq!(nav.parts, vec!["floor".to_owned(), "stairs".to_owned()]);
    assert!((nav.cell - 0.2).abs() < 1e-12 && (nav.radius - 0.35).abs() < 1e-12);
    assert!(
        (nav.height - 1.7).abs() < 1e-12
            && (nav.step - 0.3).abs() < 1e-12
            && (nav.slope - 40.0).abs() < 1e-12
    );
}

#[test]
fn only_the_parts_are_required() {
    let def = GameDef::parse("game name=a\nnav parts=floor\n").unwrap();
    let nav = def.nav().unwrap();
    assert!(
        (nav.cell - 0.25).abs() < 1e-12
            && (nav.radius - 0.3).abs() < 1e-12
            && (nav.height - 1.8).abs() < 1e-12
    );
    assert!((nav.step - 0.4).abs() < 1e-12 && (nav.slope - 45.0).abs() < 1e-12);
}

#[test]
fn a_game_without_a_nav_line_has_none() {
    assert!(GameDef::parse("game name=a\n").unwrap().nav().is_none());
}

#[test]
fn bad_nav_lines_name_the_key() {
    for (line, key) in [
        ("nav cell=0.2\n", "parts"),
        ("nav parts=floor cell=0\n", "cell"),
        ("nav parts=floor radius=-1\n", "radius"),
        ("nav parts=floor slope=95\n", "slope"),
    ] {
        let diagnostics = GameDef::parse(&format!("game name=a\n{line}")).unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.message.contains(key)),
            "{line}: {diagnostics:?}"
        );
    }
}

#[test]
fn it_round_trips() {
    let def = GameDef::parse("game name=a\nnav parts=floor,ramp cell=0.2 radius=0.35\n").unwrap();
    let text = def.describe();
    assert!(text.contains("nav parts=floor,ramp"), "{text}");
    assert_eq!(GameDef::parse(&text).unwrap().describe(), text);
}
