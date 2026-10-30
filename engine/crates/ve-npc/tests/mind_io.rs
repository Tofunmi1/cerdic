//! Contract tests for the boundary between a language model and the game (ADR 0064, step M2a): the **intent vocabulary** a game registers,
//! the **validator** that turns a model's reply into intents (and drops everything else), and the **prompt assembler** with its fixed order
//! and its budget. The model can only ever ask for what the vocabulary lists.
#![expect(
    clippy::expect_used,
    clippy::float_cmp,
    clippy::panic,
    reason = "tests assert exact values the contract fixes and fail loudly on the wrong kind of argument"
)]

use ve_npc::{
    ArgType, ArgValue, IntentError, IntentSpec, PromptBudget, PromptSections, Vocabulary,
    assemble_prompt, parse_reply,
};

fn vocabulary() -> Vocabulary {
    let mut v = Vocabulary::with_builtins();
    v.register(
        IntentSpec::new("offer_discount", "Offer the player a discount.").arg(
            "percent",
            ArgType::Number {
                min: 0.0,
                max: 30.0,
            },
        ),
    )
    .expect("registers");
    v.register(
        IntentSpec::new("set_mood", "Change how the character feels.").arg(
            "mood",
            ArgType::Choice(vec!["calm".into(), "angry".into(), "afraid".into()]),
        ),
    )
    .expect("registers");
    v
}

fn text(intent: &ve_npc::Intent, arg: &str) -> String {
    match intent.args.get(arg).expect("arg present") {
        ArgValue::Text(s) => s.clone(),
        other => panic!("not text: {other:?}"),
    }
}

fn number(intent: &ve_npc::Intent, arg: &str) -> f64 {
    match intent.args.get(arg).expect("arg present") {
        ArgValue::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

#[test]
fn builtin_intents_exist() {
    let v = Vocabulary::with_builtins();
    for name in [
        "say",
        "remember",
        "set_fact",
        "adjust_relationship",
        "emote",
    ] {
        assert!(v.get(name).is_some(), "{name}");
    }
}

#[test]
fn a_clean_reply_becomes_typed_intents() {
    let v = vocabulary();
    let reply = r#"{"intents":[{"name":"say","args":{"text":"Fresh bread, just out of the oven!"}},{"name":"remember","args":{"text":"The player praised my bread","importance":0.6}}]}"#;
    let parsed = parse_reply(reply, &v);
    assert!(parsed.rejected.is_empty(), "{:?}", parsed.rejected);
    assert_eq!(parsed.intents.len(), 2);
    assert_eq!(parsed.intents[0].name, "say");
    assert_eq!(
        text(&parsed.intents[0], "text"),
        "Fresh bread, just out of the oven!"
    );
    assert_eq!(number(&parsed.intents[1], "importance"), 0.6);
}

#[test]
fn json_wrapped_in_prose_or_fences_is_found() {
    let v = vocabulary();
    let fenced = "Sure! Here you go:\n```json\n{\"intents\":[{\"name\":\"say\",\"args\":{\"text\":\"Hello\"}}]}\n```\nAnything else?";
    let parsed = parse_reply(fenced, &v);
    assert_eq!(parsed.intents.len(), 1, "{:?}", parsed.rejected);
    let braces_in_text =
        r#"Reply: {"intents":[{"name":"say","args":{"text":"A {curly} word"}}]} done"#;
    let parsed = parse_reply(braces_in_text, &v);
    assert_eq!(text(&parsed.intents[0], "text"), "A {curly} word");
}

#[test]
fn an_unknown_intent_is_dropped_and_the_rest_kept() {
    let v = vocabulary();
    let reply = r#"{"intents":[{"name":"teleport_player","args":{"x":1}},{"name":"say","args":{"text":"Hmm."}}]}"#;
    let parsed = parse_reply(reply, &v);
    assert_eq!(parsed.intents.len(), 1);
    assert_eq!(parsed.intents[0].name, "say");
    assert_eq!(parsed.rejected.len(), 1);
    assert!(
        parsed.rejected[0].reason.contains("teleport_player"),
        "{}",
        parsed.rejected[0].reason
    );
}

#[test]
fn a_missing_or_mistyped_argument_drops_that_intent_only() {
    let v = vocabulary();
    let reply = r#"{"intents":[{"name":"offer_discount","args":{}},{"name":"offer_discount","args":{"percent":"lots"}},{"name":"offer_discount","args":{"percent":10}}]}"#;
    let parsed = parse_reply(reply, &v);
    assert_eq!(parsed.intents.len(), 1);
    assert_eq!(number(&parsed.intents[0], "percent"), 10.0);
    assert_eq!(parsed.rejected.len(), 2);
    assert!(parsed.rejected[0].reason.contains("percent"));
}

#[test]
fn numbers_are_clamped_into_their_range() {
    let v = vocabulary();
    let reply = r#"{"intents":[{"name":"offer_discount","args":{"percent":99}},{"name":"offer_discount","args":{"percent":-5}}]}"#;
    let parsed = parse_reply(reply, &v);
    assert_eq!(parsed.intents.len(), 2);
    assert_eq!(number(&parsed.intents[0], "percent"), 30.0);
    assert_eq!(number(&parsed.intents[1], "percent"), 0.0);
}

#[test]
fn a_choice_outside_the_list_is_dropped() {
    let v = vocabulary();
    let good = parse_reply(
        r#"{"intents":[{"name":"set_mood","args":{"mood":"angry"}}]}"#,
        &v,
    );
    assert_eq!(text(&good.intents[0], "mood"), "angry");
    let bad = parse_reply(
        r#"{"intents":[{"name":"set_mood","args":{"mood":"ecstatic"}}]}"#,
        &v,
    );
    assert!(bad.intents.is_empty());
    assert_eq!(bad.rejected.len(), 1);
}

#[test]
fn text_is_cut_to_its_limit_without_breaking_characters() {
    let mut v = Vocabulary::with_builtins();
    v.register(IntentSpec::new("shout", "Shout.").arg("text", ArgType::Text { max_len: 5 }))
        .expect("registers");
    let parsed = parse_reply(
        r#"{"intents":[{"name":"shout","args":{"text":"héllo wörld"}}]}"#,
        &v,
    );
    assert_eq!(
        text(&parsed.intents[0], "text"),
        "héllo",
        "five characters, not five bytes"
    );
}

#[test]
fn extra_arguments_are_ignored_and_newlines_survive() {
    let v = vocabulary();
    let reply = "{\"intents\":[{\"name\":\"say\",\"args\":{\"text\":\"Line one\\nLine two\",\"volume\":11}}]}";
    let parsed = parse_reply(reply, &v);
    assert_eq!(text(&parsed.intents[0], "text"), "Line one\nLine two");
    assert!(!parsed.intents[0].args.contains_key("volume"));
}

#[test]
fn only_the_first_eight_intents_of_a_reply_count() {
    let v = vocabulary();
    let one = r#"{"name":"say","args":{"text":"x"}}"#;
    let reply = format!("{{\"intents\":[{}]}}", [one; 10].join(","));
    let parsed = parse_reply(&reply, &v);
    assert_eq!(parsed.intents.len(), 8);
    assert_eq!(parsed.rejected.len(), 2);
}

#[test]
fn a_reply_with_no_json_gives_nothing_and_says_why() {
    let v = vocabulary();
    let parsed = parse_reply("I would like to help but I am only a language model.", &v);
    assert!(parsed.intents.is_empty());
    assert_eq!(parsed.rejected.len(), 1);
    assert!(
        parsed.rejected[0].reason.to_lowercase().contains("json"),
        "{}",
        parsed.rejected[0].reason
    );
    let broken = parse_reply(r#"{"intents":[{"name":"say","args":{"text":"cut off"#, &v);
    assert!(broken.intents.is_empty());
}

#[test]
fn the_vocabulary_refuses_bad_and_duplicate_names() {
    let mut v = Vocabulary::with_builtins();
    assert!(matches!(
        v.register(IntentSpec::new("say", "again")),
        Err(IntentError::Duplicate(_))
    ));
    assert!(matches!(
        v.register(IntentSpec::new("Bad Name", "x")),
        Err(IntentError::BadName(_))
    ));
    assert!(matches!(
        v.register(IntentSpec::new("", "x")),
        Err(IntentError::BadName(_))
    ));
}

#[test]
fn the_vocabulary_describes_itself_the_same_way_every_time() {
    let v = vocabulary();
    let a = v.describe();
    assert_eq!(a, v.describe());
    for name in ["say", "offer_discount", "set_mood"] {
        assert!(a.contains(name), "{name} missing from:\n{a}");
    }
    assert!(a.contains("percent"), "{a}");
    assert!(
        a.contains('0') && a.contains("30"),
        "the range is shown: {a}"
    );
    assert!(a.contains("angry"), "choices are shown: {a}");
}

// ---- the prompt ----

fn sections() -> PromptSections {
    PromptSections {
        persona: "You are Anna, the village baker. Warm, brisk, proud of your bread.".into(),
        rules: "Stay in character. Never mention being an AI.".into(),
        world: vec!["The village has a mill, a smithy and a well.".into()],
        goals: vec!["Sell all the bread before noon.".into()],
        state: vec!["Mood: cheerful.".into()],
        memories: vec![
            "(2 minutes ago) The player asked about bread.".into(),
            "(1 day ago) The miller owes me money.".into(),
            "(3 days ago) A wolf was seen near the hill.".into(),
        ],
        conversation: vec![
            ("player".into(), "Hello there.".into()),
            ("Anna".into(), "Good morning!".into()),
            ("player".into(), "What do you sell?".into()),
        ],
        situation: "The player stands at your counter.".into(),
    }
}

fn big() -> PromptBudget {
    PromptBudget { max_chars: 100_000 }
}

#[test]
fn sections_come_in_a_fixed_order_with_the_reply_format_last() {
    let v = vocabulary();
    let p = assemble_prompt(&sections(), &v, &big());
    let order = [
        "## Persona",
        "## Rules",
        "## World",
        "## Goals",
        "## State",
        "## Memories",
        "## Conversation",
        "## Situation",
        "## Reply format",
    ];
    let mut at = 0;
    for header in order {
        let found = p.text[at..]
            .find(header)
            .unwrap_or_else(|| panic!("{header} missing or out of order in:\n{}", p.text));
        at += found + header.len();
    }
    assert!(p.truncated.is_empty());
    assert!(
        p.text.contains("offer_discount"),
        "the reply format lists the vocabulary"
    );
}

#[test]
fn empty_sections_print_no_header() {
    let v = Vocabulary::with_builtins();
    let mut s = sections();
    s.world.clear();
    s.goals.clear();
    s.conversation.clear();
    let p = assemble_prompt(&s, &v, &big());
    assert!(
        !p.text.contains("## World")
            && !p.text.contains("## Goals")
            && !p.text.contains("## Conversation")
    );
    assert!(p.text.contains("## Persona") && p.text.contains("## Situation"));
}

#[test]
fn memories_keep_their_order_and_are_numbered() {
    let v = vocabulary();
    let p = assemble_prompt(&sections(), &v, &big());
    let one = p.text.find("1. (2 minutes ago)").expect("first numbered");
    let two = p.text.find("2. (1 day ago)").expect("second numbered");
    let three = p.text.find("3. (3 days ago)").expect("third numbered");
    assert!(one < two && two < three);
}

#[test]
fn a_tight_budget_drops_the_oldest_talk_then_the_last_memories_never_the_essentials() {
    let v = vocabulary();
    let full = assemble_prompt(&sections(), &v, &big());
    let tight = PromptBudget {
        max_chars: full.text.len() - 60,
    };
    let p = assemble_prompt(&sections(), &v, &tight);
    assert!(
        p.text.len() <= tight.max_chars,
        "{} > {}",
        p.text.len(),
        tight.max_chars
    );
    assert!(
        !p.text.contains("Hello there."),
        "the oldest conversation line goes first"
    );
    assert!(
        p.text.contains("What do you sell?"),
        "the newest line stays"
    );
    assert!(!p.truncated.is_empty());
    let very_tight = PromptBudget { max_chars: 1 };
    let p = assemble_prompt(&sections(), &v, &very_tight);
    for must in [
        "You are Anna",
        "Stay in character",
        "The player stands at your counter",
        "## Reply format",
        "say",
    ] {
        assert!(p.text.contains(must), "{must} must always be kept");
    }
    assert!(
        !p.text.contains("wolf"),
        "memories are dropped before essentials"
    );
    assert!(p.over_budget, "the essentials alone are over a budget of 1");
}

#[test]
fn the_same_input_gives_the_same_prompt() {
    let v = vocabulary();
    let a = assemble_prompt(&sections(), &v, &big());
    let b = assemble_prompt(&sections(), &v, &big());
    assert_eq!(a.text, b.text);
    assert_eq!(a.parts, b.parts);
}

#[test]
fn the_parts_report_how_big_each_section_is() {
    let v = vocabulary();
    let p = assemble_prompt(&sections(), &v, &big());
    let names: Vec<_> = p.parts.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names.first(), Some(&"persona"));
    assert_eq!(names.last(), Some(&"reply_format"));
    assert!(p.parts.iter().all(|(_, chars)| *chars > 0));
    let total: usize = p.parts.iter().map(|(_, c)| *c).sum();
    assert!(total <= p.text.len());
}
