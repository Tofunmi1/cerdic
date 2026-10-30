//! Additional robustness tests for the language-model boundary.
use ve_npc::{
    ArgType, ArgValue, IntentSpec, PromptBudget, PromptSections, Vocabulary, assemble_prompt,
    parse_reply,
};

#[test]
fn seeded_byte_fuzz_never_panics() {
    let vocab = Vocabulary::with_builtins();
    let mut state = 0x5eed_u64;
    for _ in 0..4000 {
        let len = usize::try_from(state % 100).unwrap_or(0);
        let mut bytes = Vec::with_capacity(len);
        for _ in 0..len {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            bytes.push(u8::try_from((state >> 32) & 0xff).unwrap_or(0));
        }
        let input = String::from_utf8_lossy(&bytes);
        let _ = parse_reply(&input, &vocab);
    }
}
#[test]
fn strings_booleans_integer_numbers_and_unknown_nested_values_validate() {
    let mut v = Vocabulary::new();
    v.register(
        IntentSpec::new("probe", "probe")
            .arg("flag", ArgType::Bool)
            .arg("n", ArgType::Number { min: 0.0, max: 4.0 })
            .arg("text", ArgType::Text { max_len: 80 }),
    )
    .unwrap();
    let r = parse_reply(
        r#"{"intents":[{"name":"probe","args":{"flag":true,"n":3,"text":"escaped \" {brace}" ,"extra":{"nest":{"x":1}}}}]}"#,
        &v,
    );
    assert!(r.rejected.is_empty(), "{:?}", r.rejected);
    assert_eq!(r.intents[0].args["flag"], ArgValue::Bool(true));
    assert_eq!(r.intents[0].args["n"], ArgValue::Number(3.0));
    assert_eq!(
        r.intents[0].args["text"],
        ArgValue::Text("escaped \" {brace}".into())
    );
}
#[test]
fn duplicates_and_empty_arrays_are_fine() {
    let v = Vocabulary::with_builtins();
    let dup = parse_reply(
        r#"{"intents":[{"name":"say","args":{"text":"x"}},{"name":"say","args":{"text":"x"}}]}"#,
        &v,
    );
    assert_eq!(dup.intents.len(), 2);
    let empty = parse_reply(r#"{"intents":[]}"#, &v);
    assert!(empty.intents.is_empty() && empty.rejected.is_empty());
}
#[test]
fn describe_includes_every_argument_type() {
    let mut v = Vocabulary::new();
    v.register(
        IntentSpec::new("types", "types")
            .arg(
                "number",
                ArgType::Number {
                    min: 0.0,
                    max: 30.0,
                },
            )
            .arg("text", ArgType::Text { max_len: 5 })
            .arg(
                "choice",
                ArgType::Choice(vec!["calm".into(), "angry".into()]),
            )
            .arg("bool", ArgType::Bool),
    )
    .unwrap();
    let text = v.describe();
    for phrase in [
        "number 0..30",
        "text (max 5 chars)",
        "one of: calm, angry",
        "true or false",
    ] {
        assert!(text.contains(phrase), "{phrase}: {text}");
    }
}
#[test]
fn minimal_prompt_and_equal_full_budget_keep_everything() {
    let v = Vocabulary::new();
    let s = PromptSections {
        persona: "P".into(),
        rules: "R".into(),
        situation: "S".into(),
        ..PromptSections::default()
    };
    let full = assemble_prompt(&s, &v, &PromptBudget { max_chars: 10000 });
    let exact = assemble_prompt(
        &s,
        &v,
        &PromptBudget {
            max_chars: full.text.chars().count(),
        },
    );
    assert_eq!(exact.text, full.text);
    assert!(exact.truncated.is_empty());
    assert!(
        exact.text.contains("## Persona")
            && exact.text.contains("## Rules")
            && exact.text.contains("## Situation")
            && exact.text.contains("## Reply format")
    );
}
