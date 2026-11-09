//! Action map (F6 of `plans/game-foundations-plan.md`): an `input` line may name the keys and gamepad controls that drive it
//! (`keys=`, `pad=`) and a `label=` for prompts, and the game exports the table so the page can build its bindings from the game instead of
//! hard-coding them. Written before the body; do not weaken.
use ve_game::GameDef;

fn parse(inputs: &str) -> Result<GameDef, String> {
    GameDef::parse(&format!("game name=bind\nkind hero size=0.4 tags=hero\n{inputs}\n"))
        .map_err(|errors| errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(" | "))
}

#[test]
fn keys_pad_and_label_are_kept_and_exported() {
    let def = parse("input move axis=x,z target=tag:hero keys=KeyD,KeyA,KeyW,KeyS pad=stick:left label=Move\ninput jump key=press keys=Space,KeyC pad=button0 label=Jump").unwrap();
    let table: serde_json::Value = serde_json::from_str(&def.bindings_json()).unwrap();
    let rows = table.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "move");
    assert_eq!(rows[0]["kind"], "axis");
    assert_eq!(rows[0]["axes"], serde_json::json!(["x", "z"]));
    assert_eq!(rows[0]["keys"], serde_json::json!(["KeyD", "KeyA", "KeyW", "KeyS"]));
    assert_eq!(rows[0]["pad"], serde_json::json!(["stick:left"]));
    assert_eq!(rows[0]["label"], "Move");
    assert_eq!(rows[1]["kind"], "press");
    assert_eq!(rows[1]["keys"], serde_json::json!(["Space", "KeyC"]));
    assert_eq!(rows[1]["pad"], serde_json::json!(["button0"]));
}

#[test]
fn inputs_without_bindings_export_empty_lists_so_old_games_keep_their_defaults() {
    let def = parse("input dodge key=press").unwrap();
    let table: serde_json::Value = serde_json::from_str(&def.bindings_json()).unwrap();
    assert_eq!(table[0]["keys"], serde_json::json!([]));
    assert_eq!(table[0]["pad"], serde_json::json!([]));
    assert_eq!(table[0]["label"], "dodge", "the label defaults to the name");
}

#[test]
fn bindings_survive_a_text_round_trip() {
    let def = parse("input move axis=x,z keys=KeyD,KeyA,KeyW,KeyS pad=stick:left label=Move").unwrap();
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def.bindings_json(), again.bindings_json());
}

#[test]
fn an_axis_needs_a_key_for_each_direction_of_each_axis() {
    let problem = parse("input move axis=x,z keys=KeyD,KeyA").unwrap_err();
    assert!(problem.contains("keys") && problem.contains('4'), "{problem}");
    assert!(parse("input steer axis=x keys=KeyD,KeyA").is_ok(), "two keys for one axis");
}

#[test]
fn bad_pad_controls_and_key_names_are_refused() {
    assert!(parse("input jump key=press pad=trigger9").unwrap_err().contains("pad"));
    assert!(parse("input move axis=x,z pad=stick:middle").unwrap_err().contains("pad"));
    assert!(parse("input jump key=press pad=stick:left").unwrap_err().contains("pad"), "a stick drives an axis pair, not a press");
    assert!(parse("input jump key=press keys=Sp ace").is_err());
    assert!(parse("input jump key=press keys=").unwrap_err().contains("keys"));
    assert!(parse("input move axis=x,z pad=axis1+").unwrap_err().contains("pad"), "a single stick direction is for a one-axis input");
    assert!(parse("input steer axis=x pad=axis0+,axis0-").is_ok());
}
