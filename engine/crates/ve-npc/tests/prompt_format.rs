//! The reply-format section must show a worked example of the exact JSON shape. Found with a real model (Haiku through the Claude CLI,
//! 2026-10-24): with only a list of `name(arg: type)` signatures, every reply was rejected because the model copied the signature notation
//! (`{"intents":[{"emote":"say"},{"say":"text"}]}`); with this example every reply was valid (6 of 6).

use ve_npc::{PromptBudget, PromptSections, Vocabulary, assemble_prompt, parse_reply};

#[test]
fn the_reply_format_shows_the_exact_shape_and_that_example_itself_parses() {
    let vocab = Vocabulary::with_builtins();
    let sections = PromptSections {
        persona: "You are Anna.".into(),
        rules: "Stay in character.".into(),
        world: Vec::new(),
        goals: Vec::new(),
        state: Vec::new(),
        memories: Vec::new(),
        conversation: Vec::new(),
        situation: "The player says: \"Hi\"".into(),
    };
    let prompt = assemble_prompt(&sections, &vocab, &PromptBudget { max_chars: 10_000 });
    let example = r#"{"intents":[{"name":"say","args":{"text":"Hello there!"}}]}"#;
    assert!(
        prompt.text.contains(example),
        "the example is in the prompt:\n{}",
        prompt.text
    );
    assert!(prompt.text.contains("ONE JSON object"), "{}", prompt.text);
    let parsed = parse_reply(example, &vocab);
    assert_eq!(
        parsed.intents.len(),
        1,
        "the prompt's own example is a valid reply: {:?}",
        parsed.rejected
    );
    assert!(
        prompt.text.contains("include a say intent"),
        "the model is told to answer the player"
    );
    let signature_copy = parse_reply(r#"{"intents":[{"emote":"say"},{"say":"text"}]}"#, &vocab);
    assert!(
        signature_copy.intents.is_empty(),
        "the failure mode seen with a real model is still rejected"
    );
}
