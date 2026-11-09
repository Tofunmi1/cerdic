//! Script declarations: JavaScript behaviours that run inside the fixed step.
use super::{GameDef, helpers::required};
use std::collections::BTreeMap;

/// How many scripts one game may declare.
pub(crate) const MAX_SCRIPTS: usize = 16;
/// The longest script source, in bytes.
pub(crate) const MAX_SOURCE: usize = 64 * 1024;

/// A JavaScript behaviour: it registers handlers with `on(event, fn)` when the game starts and answers events inside the step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptDef {
    /// Unique lowercase name (letters, digits, underscores).
    pub name: String,
    /// The source. Line ends inside a definition line are U+2028, which JavaScript reads as a line end too.
    pub source: String,
    /// The standard library this script is, when it is one (`rpg`); its source is then the library's own.
    pub library: Option<String>,
}

/// The standard libraries a game can load with `script name=rpg lib=rpg`: ordinary scripts that define a global for the scripts after
/// them. Names are what `game.script@1(lib=...)` accepts.
pub const LIBRARIES: &[&str] = &["rpg", "combat", "inventory"];

fn library_source(name: &str) -> Option<&'static str> {
    match name {
        "rpg" => Some(include_str!("../world/state/script/lib_rpg.js")),
        "combat" => Some(include_str!("../world/state/script/lib_combat.js")),
        "inventory" => Some(include_str!("../world/state/script/lib_inventory.js")),
        _ => None,
    }
}

impl ScriptDef {
    /// Builds a script from ordinary source text (new lines become U+2028 so the definition stays one line).
    #[must_use]
    pub fn new(name: &str, source: &str) -> Self {
        Self {
            name: name.to_owned(),
            source: source.replace("\r\n", "\n").replace('\n', "\u{2028}"),
            library: None,
        }
    }

    /// A standard library as a script, or `None` for a name that is not one (see [`LIBRARIES`]).
    #[must_use]
    pub fn library(name: &str, library: &str) -> Option<Self> {
        library_source(library).map(|source| Self {
            name: name.to_owned(),
            source: source.replace("\r\n", "\n").replace('\n', "\u{2028}"),
            library: Some(library.to_owned()),
        })
    }

    /// The statement that declares it, one line.
    #[must_use]
    pub fn to_text(&self) -> String {
        if let Some(library) = &self.library {
            return format!("script name={} lib={library}", self.name);
        }
        // The definition reader unescapes quoted values twice, so quotes and backslashes are written as percent codes instead.
        let source = self
            .source
            .replace('%', "%25")
            .replace('"', "%22")
            .replace('\\', "%5C");
        format!("script name={} src=\"{source}\"", self.name)
    }
}

fn decode(text: &str) -> String {
    text.replace("%22", "\"")
        .replace("%5C", "\\")
        .replace("%25", "%")
}

pub(super) fn parse_script(
    definition: &mut GameDef,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    let name = required(fields, "name")?;
    if name.is_empty()
        || !name.starts_with(|c: char| c.is_ascii_lowercase())
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!(
            "script name '{name}' must be lowercase letters, digits and underscores, starting with a letter"
        ));
    }
    if definition.scripts.len() >= MAX_SCRIPTS {
        return Err(format!("at most {MAX_SCRIPTS} scripts are allowed"));
    }
    if definition.scripts.iter().any(|script| script.name == name) {
        return Err(format!("script '{name}' is declared twice"));
    }
    if let Some(library) = fields.get("lib") {
        let script = ScriptDef::library(name, library).ok_or_else(|| {
            format!(
                "unknown library '{library}': the libraries are {}",
                LIBRARIES.join(", ")
            )
        })?;
        definition.scripts.push(script);
        return Ok(());
    }
    let source = decode(required(fields, "src")?);
    if source.trim().is_empty() {
        return Err("script source is empty".into());
    }
    if source.len() > MAX_SOURCE {
        return Err(format!("script source exceeds {MAX_SOURCE} bytes"));
    }
    definition.scripts.push(ScriptDef {
        name: name.to_owned(),
        source,
        library: None,
    });
    Ok(())
}
