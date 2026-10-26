//! Parser fuzz properties: arbitrary fields never panic and accepted declarations round trip.
use crate::GameDef;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_agent_and_nav_fields_are_diagnostic_or_round_trip(
        fields in "[a-zA-Z0-9+.,;=:_ -]{0,512}",
        agent in any::<bool>(),
    ) {
        let command = if agent { "agent creature" } else { "nav" };
        let source = format!("game name=fuzz\nkind creature size=0.4\n{command} {fields}\n");
        match GameDef::parse(&source) {
            Ok(definition) => {
                let text = definition.describe();
                let round_trip = GameDef::parse(&text);
                prop_assert!(round_trip.is_ok());
                prop_assert_eq!(round_trip.unwrap().describe(), text);
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
