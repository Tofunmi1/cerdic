//! HUD lines bound to variables (ADR 0062 section 6): `hud text="Score {score} / 4"` statements, printed with the current variable values.
//! Written before the bodies; do not weaken.
use std::collections::BTreeMap;
use ve_game::{GameDef, Inputs, World};

const GAME: &str = "game name=h
var score=0 lives=3
kind player size=0.5 tags=hero
kind coin size=0.4 tags=pickup
spawn player at=0,0,0
spawn coin at=0,0,0
hud text=\"Score {score} / 4\" anchor=gaze
hud text=\"Lives {lives}\" anchor=left size=0.04
rule on=touch a=tag:hero b=tag:pickup do=add:score:1;destroy:b
goal win when=score>=4
";

#[test]
fn hud_statements_parse_with_quoted_text_and_defaults() {
    let def = GameDef::parse(GAME).unwrap();
    assert_eq!(def.huds.len(), 2);
    assert_eq!(def.huds[0].text, "Score {score} / 4");
    assert_eq!(def.huds[0].anchor, "gaze");
    assert!(
        (def.huds[0].size - 0.005).abs() < 1e-9,
        "default size is 0.005 m per font pixel (3.5 cm letters): {}",
        def.huds[0].size
    );
    assert_eq!(def.huds[1].anchor, "left");
    assert!((def.huds[1].size - 0.04).abs() < 1e-9);
}

#[test]
fn a_hud_survives_printing() {
    let def = GameDef::parse(GAME).unwrap();
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def, again);
    assert!(def.to_text().contains("hud text=\"Score {score} / 4\""));
}

#[test]
fn hud_lines_show_current_values_and_follow_the_game() {
    let def = GameDef::parse(GAME).unwrap();
    let mut world = World::new(&def, 1);
    assert_eq!(
        world.hud_lines(),
        vec!["Score 0 / 4".to_owned(), "Lives 3".to_owned()]
    );
    world
        .step(&Inputs {
            axes: BTreeMap::new(),
            ..Inputs::default()
        })
        .unwrap();
    assert_eq!(world.hud_lines()[0], "Score 1 / 4", "the pickup scored");
}

#[test]
fn whole_numbers_print_plainly_and_fractions_with_one_decimal() {
    let text = "game name=f\nvar speed=1.5 count=3 neg=-2\nkind a size=1\nspawn a at=0,0,0\nhud text=\"{speed} {count} {neg}\"\ngoal win when=count>=99\n";
    let world = World::new(&GameDef::parse(text).unwrap(), 1);
    assert_eq!(world.hud_lines(), vec!["1.5 3 -2".to_owned()]);
}

#[test]
fn a_placeholder_for_a_variable_that_does_not_exist_is_a_located_error() {
    let text = GAME.replace("{lives}", "{life}");
    let errors = GameDef::parse(&text).unwrap_err();
    let error = errors
        .iter()
        .find(|e| e.message.contains("life"))
        .expect("names the missing variable");
    assert_eq!(error.line, 8, "on the hud line");
    assert!(
        error.suggestion.contains("score") || error.suggestion.contains("lives"),
        "the suggestion names real variables: {}",
        error.suggestion
    );
}

#[test]
fn hud_text_is_bounded() {
    let long = format!(
        "game name=l\nvar a=0\nkind k size=1\nspawn k at=0,0,0\nhud text=\"{}\"\ngoal win when=a>=1\n",
        "x".repeat(80)
    );
    let errors = GameDef::parse(&long).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("64")),
        "{errors:?}"
    );
    let mut many = String::new();
    for i in 0..9 {
        use std::fmt::Write as _;
        let _ = writeln!(many, "hud text=\"line {i}\"");
    }
    let text = format!(
        "game name=m\nvar a=0\nkind k size=1\nspawn k at=0,0,0\n{many}goal win when=a>=1\n"
    );
    let errors = GameDef::parse(&text).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains('8')),
        "at most 8 hud lines: {errors:?}"
    );
}

#[test]
fn an_unknown_anchor_is_refused() {
    let text = GAME.replace("anchor=left", "anchor=middle_of_nowhere");
    let errors = GameDef::parse(&text).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.line == 8 && e.message.contains("anchor")),
        "{errors:?}"
    );
}
