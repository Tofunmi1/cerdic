//! Deterministic prompt assembly with explicit section and character budgets.
use crate::Vocabulary;

/// Prompt content split into fixed semantic sections.
#[derive(Clone, Debug, Default)]
pub struct PromptSections {
    /// Character persona.
    pub persona: String,
    /// Behavioral rules.
    pub rules: String,
    /// Allowed world facts.
    pub world: Vec<String>,
    /// Current goals.
    pub goals: Vec<String>,
    /// Current state.
    pub state: Vec<String>,
    /// Retrieved memories in caller-provided order.
    pub memories: Vec<String>,
    /// Recent speaker and utterance pairs.
    pub conversation: Vec<(String, String)>,
    /// Current situation.
    pub situation: String,
}
/// Maximum assembled prompt character count.
#[derive(Clone, Copy, Debug)]
pub struct PromptBudget {
    /// Character limit; essential sections may exceed it.
    pub max_chars: usize,
}
/// Assembled prompt text and section size metadata.
#[derive(Clone, Debug, Default)]
pub struct Prompt {
    /// Prompt text.
    pub text: String,
    /// Section name and rendered character count.
    pub parts: Vec<(String, usize)>,
    /// Content removed to fit the budget.
    pub truncated: Vec<String>,
    /// Whether essentials alone exceed the budget.
    pub over_budget: bool,
}

#[derive(Clone)]
struct Section {
    name: &'static str,
    header: &'static str,
    rows: Vec<String>,
}
impl Section {
    fn body(&self) -> String {
        match self.name {
            "world" | "goals" | "state" => self
                .rows
                .iter()
                .map(|r| format!("- {r}"))
                .collect::<Vec<_>>()
                .join("\n"),
            "memories" => self
                .rows
                .iter()
                .enumerate()
                .map(|(i, r)| format!("{}. {r}", i + 1))
                .collect::<Vec<_>>()
                .join("\n"),
            "conversation" => self.rows.join("\n"),
            _ => self.rows.first().cloned().unwrap_or_default(),
        }
    }
    fn render(&self) -> String {
        format!("{}\n{}", self.header, self.body())
    }
}
/// Builds fixed-order prompt sections and trims low-priority rows to the character budget.
#[must_use]
pub fn assemble_prompt(
    sections: &PromptSections,
    vocab: &Vocabulary,
    budget: &PromptBudget,
) -> Prompt {
    let mut list = render_sections(sections, vocab);
    let removed = drop_to_budget(&mut list, budget.max_chars);
    let text = render_all(&list);
    let over_budget = text.chars().count() > budget.max_chars;
    let parts = section_parts(&list);
    let truncated = truncation_summary(removed);
    Prompt {
        text,
        parts,
        truncated,
        over_budget,
    }
}

fn render_sections(sections: &PromptSections, vocab: &Vocabulary) -> Vec<Section> {
    let mut list = Vec::new();
    add_single(&mut list, "persona", "## Persona", &sections.persona, true);
    add_single(&mut list, "rules", "## Rules", &sections.rules, true);
    add_rows(
        &mut list,
        "world",
        "## World",
        sections.world.iter().map(String::as_str),
        false,
    );
    add_rows(
        &mut list,
        "goals",
        "## Goals",
        sections.goals.iter().map(String::as_str),
        false,
    );
    add_rows(
        &mut list,
        "state",
        "## State",
        sections.state.iter().map(String::as_str),
        false,
    );
    add_rows(
        &mut list,
        "memories",
        "## Memories",
        sections.memories.iter().map(String::as_str),
        false,
    );
    add_rows(
        &mut list,
        "conversation",
        "## Conversation",
        sections
            .conversation
            .iter()
            .map(|(a, b)| format!("{a}: {b}"))
            .collect::<Vec<_>>()
            .iter()
            .map(String::as_str),
        false,
    );
    add_single(
        &mut list,
        "situation",
        "## Situation",
        &sections.situation,
        true,
    );
    add_single(
        &mut list,
        "reply_format",
        "## Reply format",
        &format!(
            "Reply with ONE JSON object and nothing else, shaped exactly like this example: {{\"intents\":[{{\"name\":\"say\",\"args\":{{\"text\":\"Hello there!\"}}}}]}}\nEach entry of \"intents\" is an object with a \"name\" (one of the intents below) and an \"args\" object that uses exactly that intent's argument names. Use at most eight intents. Only the listed intents exist. When the player speaks to you, include a say intent with your answer.\nIntents:\n{}\nWhen to use them: use remember(text, importance) for gifts, promises, threats, secrets you learn, or anything that changes how you feel about the player (importance 0.2 trivial, 0.5 notable, 0.9 life-changing). Do not remember small talk. Use adjust_relationship when the player is kind (positive numbers) or rude (negative), by small amounts such as 0.1 to 0.3. Vary your wording: do not repeat a sentence or sales pitch you already said in this conversation unless the player asks for it, and answer what the player actually said before anything else.",
            vocab.describe()
        ),
        true,
    );
    list
}

fn drop_to_budget(list: &mut [Section], max_chars: usize) -> Vec<&'static str> {
    let mut removed = Vec::new();
    for (name, from_end) in [
        ("conversation", false),
        ("memories", true),
        ("world", true),
        ("goals", true),
        ("state", true),
    ] {
        while render_all(list).chars().count() > max_chars {
            let Some(section) = list.iter_mut().find(|s| s.name == name) else {
                break;
            };
            if section.rows.is_empty() || (name == "conversation" && section.rows.len() == 1) {
                break;
            }
            if from_end {
                section.rows.pop();
            } else {
                section.rows.remove(0);
            }
            removed.push(name);
        }
    }
    removed
}

fn section_parts(list: &[Section]) -> Vec<(String, usize)> {
    list.iter()
        .filter(|s| !s.rows.is_empty())
        .map(|s| (s.name.into(), s.body().chars().count()))
        .collect()
}

fn truncation_summary(removed: Vec<&str>) -> Vec<String> {
    removed.iter().fold(Vec::<String>::new(), |mut acc, n| {
        if let Some(last) = acc.last_mut().filter(|v| v.starts_with(&format!("{n}:"))) {
            let count = last
                .split_whitespace()
                .last()
                .unwrap_or("0")
                .parse::<usize>()
                .unwrap_or(0);
            *last = format!("{n}: dropped {}", count + 1);
        } else {
            acc.push(format!("{n}: dropped 1"));
        }
        acc
    })
}
fn add_single(
    list: &mut Vec<Section>,
    name: &'static str,
    header: &'static str,
    value: &str,
    _essential: bool,
) {
    if !value.is_empty() {
        list.push(Section {
            name,
            header,
            rows: vec![value.into()],
        });
    }
}
fn add_rows<'a>(
    list: &mut Vec<Section>,
    name: &'static str,
    header: &'static str,
    rows: impl Iterator<Item = &'a str>,
    _essential: bool,
) {
    let rows = rows.map(str::to_owned).collect::<Vec<_>>();
    if !rows.is_empty() {
        list.push(Section { name, header, rows });
    }
}
fn render_all(list: &[Section]) -> String {
    list.iter()
        .filter(|s| !s.rows.is_empty())
        .map(Section::render)
        .collect::<Vec<_>>()
        .join("\n\n")
}
