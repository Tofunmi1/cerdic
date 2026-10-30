//! Small worker module; the rendering module never needs to run modeling operations.
#[cfg(any(target_arch = "wasm32", test))]
mod attachments;
#[cfg(any(target_arch = "wasm32", test))]
mod json;
#[cfg(any(target_arch = "wasm32", test))]
mod text;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(test)]
mod tests {
    use super::json::{eval_game_json, game_playtest_json};
    use serde_json::Value;

    #[test]
    fn playtest_json_uses_stable_status_and_report_fields() {
        let value: Value = serde_json::from_str(&game_playtest_json(
            "game name=empty\nvar score=0",
            "idle",
            5,
            2,
        ))
        .expect("valid JSON");
        assert_eq!(value["status"], "playing");
        assert_eq!(value["steps"], 2);
        assert!(value["hash"].is_number());
        assert_eq!(value["vars"]["score"], 0.0);
    }

    #[test]
    fn invalid_game_json_contains_parse_diagnostics() {
        let value: Value = serde_json::from_str(&game_playtest_json("not a game", "idle", 0, 1))
            .expect("valid JSON");
        assert!(value["error"].is_array());
        assert!(value["error"][0]["message"].is_string());
    }

    #[test]
    fn game_json_policy_parser_rejects_malformed_seek_policy() {
        assert_eq!(
            game_playtest_json("game name=x", "seek::target", 0, 1),
            r#"{"error":"policy must be idle, random or seek:<hero tag>:<target tag,...>"}"#
        );
    }

    #[test]
    fn absent_game_json_has_the_complete_export_shape() {
        let value: Value = serde_json::from_str(&eval_game_json(None)).expect("valid JSON");
        assert_eq!(value["present"], false);
        assert_eq!(value["text"], "");
        assert_eq!(value["summary"], "");
        assert_eq!(value["diagnostics"], serde_json::json!([]));
    }

    #[test]
    fn present_game_json_contains_text_summary_and_diagnostics() {
        let game = ve_game::GameDef::parse("game name=demo\nvar score=0\n").expect("valid fixture");
        let value: Value = serde_json::from_str(&eval_game_json(Some(&game))).expect("valid JSON");
        assert_eq!(value["present"], true);
        assert!(
            value["text"]
                .as_str()
                .is_some_and(|text| text.contains("game name=demo"))
        );
        assert!(
            value["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("demo"))
        );
        assert!(value["diagnostics"].is_array());
    }
}
