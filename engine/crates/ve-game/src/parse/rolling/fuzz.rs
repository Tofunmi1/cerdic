//! Arbitrary rolling declarations are diagnostic or canonically round-trip.
use crate::GameDef;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn rolling_fields_are_diagnostic_or_round_trip(
        fields in "[a-zA-Z0-9+.,;=:_ -]{0,512}",
        command in prop::sample::select(vec!["surface sand", "terrain", "kind ball"]),
    ) {
        match GameDef::parse(&format!("game name=fuzz\n{command} {fields}\n")) {
            Ok(definition) => {
                let text = definition.describe();
                let parsed = GameDef::parse(&text);
                prop_assert!(parsed.is_ok());
                prop_assert_eq!(parsed.unwrap().describe(), text);
            }
            Err(diagnostics) => {
                prop_assert!(!diagnostics.is_empty());
                for diagnostic in diagnostics {
                    prop_assert!(diagnostic.line > 0 && diagnostic.column > 0);
                    prop_assert!(!diagnostic.message.is_empty());
                    prop_assert!(!diagnostic.suggestion.is_empty());
                }
            }
        }
    }
}
