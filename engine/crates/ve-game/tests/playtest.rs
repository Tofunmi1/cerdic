//! Headless playtest bots (ADR 0062 phase 4, early): scripted policies play a definition and report what happened, and a definition
//! prints back to text that parses to the same game. Written before the bodies; do not weaken.
use ve_game::playtest::{Policy, run};
use ve_game::{GameDef, Status};

const COINS: &str = "game name=coins
var score=0 lives=3 time=0
kind player size=0.5 speed=4 tags=hero
kind coin size=0.4 tags=pickup
kind spike size=0.5 tags=hazard
spawn player at=0,0,0
spawn coin at=3,0,2 count=8 spread=6
input move axis=x,z target=tag:hero
rule on=touch a=tag:hero b=tag:pickup do=add:score:1;destroy:b
rule on=touch a=tag:hero b=tag:hazard do=add:lives:-1;destroy:b
rule on=tick every=1.0 do=add:time:1
rule on=var:lives when=lives<=0 do=lose
goal win when=score>=8
goal lose when=time>=60
";

fn seek() -> Policy {
    Policy::Seek {
        hero: "hero".to_owned(),
        target: "pickup".to_owned(),
    }
}

#[test]
fn a_definition_prints_back_to_text_that_parses_to_the_same_game() {
    let def = GameDef::parse(COINS).unwrap();
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def, again);
    assert_eq!(def.to_text(), again.to_text(), "printing is stable");
}

#[test]
fn a_seeking_bot_wins_the_coin_game_and_the_report_says_how() {
    let def = GameDef::parse(COINS).unwrap();
    let report = run(&def, 7, &seek(), 3_600);
    assert_eq!(report.status, Status::Won);
    assert!(
        report.steps > 0 && report.steps < 3_600,
        "won before the step limit: {}",
        report.steps
    );
    assert_eq!(report.vars.get("score").copied(), Some(8.0));
    assert_eq!(
        report.rule_fires.len(),
        def.rules.len(),
        "one count per rule, in rule order"
    );
    assert_eq!(
        report.rule_fires[0], 8,
        "the pickup rule fired once per coin"
    );
    assert_eq!(
        report.never_fired,
        vec![1, 3],
        "nothing hurt the bot and it never died: those rules never fired ({report:?})"
    );
}

#[test]
fn an_idle_bot_loses_by_the_clock() {
    let def = GameDef::parse(COINS).unwrap();
    let report = run(&def, 7, &Policy::Idle, 3_600);
    assert_eq!(
        report.status,
        Status::Lost,
        "sixty seconds of standing still ends the game"
    );
    assert!(
        (3_500..=3_700).contains(&report.steps),
        "about 60 s at 60 Hz: {}",
        report.steps
    );
    assert_eq!(report.vars.get("score").copied(), Some(0.0));
}

#[test]
fn a_game_nobody_finishes_reports_running_at_the_limit() {
    let def = GameDef::parse(COINS).unwrap();
    let report = run(&def, 7, &Policy::Idle, 120);
    assert_eq!(report.status, Status::Playing);
    assert_eq!(report.steps, 120);
}

#[test]
fn the_same_game_seed_and_policy_give_the_same_report() {
    let def = GameDef::parse(COINS).unwrap();
    let a = run(&def, 11, &Policy::Random, 600);
    let b = run(&def, 11, &Policy::Random, 600);
    assert_eq!(a, b);
    let c = run(&def, 12, &Policy::Random, 600);
    assert_ne!(a.hash, c.hash, "another seed plays another game");
}

const WALK: &str = "game name=walk
var score=0
kind player size=0.5 tags=hero
kind coin size=0.4 tags=pickup
spawn player at=0,0,0
spawn coin at=6,0,0
spawn coin at=-3,0,4
input body follow=viewer target=tag:hero
rule on=touch a=tag:hero b=tag:pickup do=add:score:1;destroy:b
goal win when=score>=2
";

#[test]
fn a_seeking_bot_walks_a_hero_that_follows_the_viewer_the_way_a_player_would() {
    let def = GameDef::parse(WALK).unwrap();
    let report = run(&def, 3, &seek(), 3_600);
    assert_eq!(
        report.status,
        Status::Won,
        "the bot walks (as the viewer) to the nearest coin, then the next: {report:?}"
    );
    assert_eq!(report.vars.get("score").copied(), Some(2.0));
    // About 6 m, then about 9.5 m, at a walking pace of about 3 m/s: five to ten seconds, not a teleport and not a crawl.
    assert!(
        (240..=900).contains(&report.steps),
        "walking pace: {} steps",
        report.steps
    );
}

#[test]
fn an_idle_player_never_moves_a_followed_hero() {
    let def = GameDef::parse(WALK).unwrap();
    let report = run(&def, 3, &Policy::Idle, 600);
    assert_eq!(report.status, Status::Playing);
    assert_eq!(report.vars.get("score").copied(), Some(0.0));
}

#[test]
fn a_random_walker_of_a_followed_hero_is_deterministic_and_actually_moves() {
    let def = GameDef::parse(WALK).unwrap();
    let a = run(&def, 5, &Policy::Random, 900);
    let b = run(&def, 5, &Policy::Random, 900);
    assert_eq!(a, b);
    let still = run(&def, 5, &Policy::Idle, 900);
    assert_ne!(a.hash, still.hash, "a random walk changes the world");
}

#[test]
fn a_game_that_cannot_be_won_is_visible_in_the_report() {
    let text = COINS.replace("goal win when=score>=8", "goal win when=score>=99");
    let def = GameDef::parse(&text).unwrap();
    let report = run(&def, 7, &seek(), 3_600);
    assert_ne!(
        report.status,
        Status::Won,
        "only eight coins exist, so a score of 99 is unreachable"
    );
    assert_eq!(
        report.vars.get("score").copied(),
        Some(8.0),
        "the bot collected everything there was"
    );
}
