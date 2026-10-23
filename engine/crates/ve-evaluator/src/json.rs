//! Stable JSON builders shared by the wasm exports and native tests.
use serde_json::json;

pub(super) fn eval_game_json(game: Option<&ve_game::GameDef>) -> String {
    let Some(game) = game else {
        return json!({"present": false, "text": "", "summary": "", "diagnostics": []}).to_string();
    };
    json!({
        "present": true,
        "text": game.to_text(),
        "summary": game.describe(),
        "diagnostics": game.validate().into_iter().map(|diagnostic| json!({
            "code": "game.validation",
            "message": diagnostic.message,
            "suggestion": diagnostic.suggestion
        })).collect::<Vec<_>>()
    })
    .to_string()
}

pub(super) fn game_playtest_json(text: &str, policy: &str, seed: u32, max_steps: u32) -> String {
    let definition = match ve_game::GameDef::parse(text) {
        Ok(definition) => definition,
        Err(diagnostics) => {
            return json!({"error": diagnostics.iter().map(|diagnostic| json!({
            "line": diagnostic.line,
            "column": diagnostic.column,
            "message": diagnostic.message,
            "suggestion": diagnostic.suggestion
        })).collect::<Vec<_>>() })
            .to_string();
        }
    };
    let Some(policy) = parse_policy(policy) else {
        return json!({"error":"policy must be idle, random or seek:<hero tag>:<target tag,...>"})
            .to_string();
    };
    let report =
        ve_game::playtest::run(&definition, u64::from(seed), &policy, max_steps.min(36_000));
    let status = match report.status {
        ve_game::Status::Playing => "playing",
        ve_game::Status::Won => "won",
        ve_game::Status::Lost => "lost",
    };
    json!({"status":status,"steps":report.steps,"vars":report.vars,"rule_fires":report.rule_fires,"never_fired":report.never_fired,"hash":report.hash}).to_string()
}

fn parse_policy(policy: &str) -> Option<ve_game::playtest::Policy> {
    match policy {
        "idle" => Some(ve_game::playtest::Policy::Idle),
        "random" => Some(ve_game::playtest::Policy::Random),
        _ => {
            let mut fields = policy.strip_prefix("seek:")?.split(':');
            let hero = fields.next()?;
            let target = fields.next()?;
            if hero.is_empty() || target.is_empty() || fields.next().is_some() {
                return None;
            }
            Some(ve_game::playtest::Policy::Seek {
                hero: hero.to_owned(),
                target: target.to_owned(),
            })
        }
    }
}
