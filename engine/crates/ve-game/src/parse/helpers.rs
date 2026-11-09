use super::GameDef;
use crate::expr::Expr;
use std::collections::BTreeMap;
pub(super) fn valid_event(event: &str) -> bool {
    matches!(
        event,
        "start"
            | "tick"
            | "touch"
            | "destroyed"
            | "fell"
            | "hurt"
            | "killed"
            | "hit"
            | "impact"
            | "fired"
            | "empty"
            | "reloaded"
            | "grab"
            | "release"
            | "use"
            | "sight"
            | "lost"
            | "catch"
            | "stun"
            | "recover"
            | "swing"
            | "out"
            | "pocket"
    ) || ["var:", "emit:", "timer:", "input:", "host:", "state:", "level:", "ui:", "ability:", "refused:"]
        .iter()
        .any(|prefix| event.starts_with(prefix) && event.len() > prefix.len())
}
pub(crate) fn validate_selector(selector: &str) -> Result<(), String> {
    if matches!(selector, "a" | "b" | "other")
        || selector
            .strip_prefix("tag:")
            .is_some_and(|name| !name.is_empty())
        || selector
            .strip_prefix("kind:")
            .is_some_and(|name| !name.is_empty())
    {
        Ok(())
    } else {
        Err(format!("bad selector '{selector}'"))
    }
}
pub(super) fn selector_exists(def: &GameDef, selector: &str) -> bool {
    selector.strip_prefix("kind:").map_or_else(
        || {
            selector.strip_prefix("tag:").is_some_and(|tag| {
                def.kinds
                    .iter()
                    .any(|kind| kind.tags.iter().any(|value| value == tag))
            })
        },
        |name| def.kinds.iter().any(|kind| kind.name == name),
    )
}
pub(super) fn event_raised(def: &GameDef, event: &str) -> bool {
    if matches!(
        event,
        "start"
            | "tick"
            | "touch"
            | "fell"
            | "hurt"
            | "killed"
            | "hit"
            | "impact"
            | "fired"
            | "empty"
            | "reloaded"
            | "grab"
            | "release"
            | "use"
            | "sight"
            | "lost"
            | "catch"
            | "stun"
            | "recover"
            | "swing"
            | "out"
            | "pocket"
    ) || event.starts_with("host:")
    {
        return true;
    }
    if let Some(name) = event.strip_prefix("input:") {
        return def
            .inputs
            .iter()
            .any(|input| input.key && input.name == name);
    }
    if let Some(name) = event.strip_prefix("state:") {
        return def.states.iter().any(|state| state.name == name);
    }
    if let Some(name) = event.strip_prefix("level:") {
        return def.levels.iter().any(|level| level.name == name);
    }
    if let Some(name) = event.strip_prefix("ability:").or_else(|| event.strip_prefix("refused:")) {
        return def.abilities.iter().any(|ability| ability.name == name);
    }
    if let Some(id) = event.strip_prefix("ui:") {
        return def.ui.iter().any(|item| item.kind == "button" && item.id == id);
    }
    if let Some(name) = event.strip_prefix("var:") {
        return def
            .rules
            .iter()
            .flat_map(|rule| &rule.actions)
            .any(|action| action_changes(action, name));
    }
    if let Some(name) = event.strip_prefix("emit:") {
        return def
            .rules
            .iter()
            .flat_map(|rule| &rule.actions)
            .any(|action| action == &format!("emit:{name}"));
    }
    if let Some(name) = event.strip_prefix("timer:") {
        return def
            .rules
            .iter()
            .flat_map(|rule| &rule.actions)
            .any(|action| action.starts_with(&format!("timer:{name}:")));
    }
    if event == "destroyed" {
        return def
            .rules
            .iter()
            .flat_map(|rule| &rule.actions)
            .any(|action| action.starts_with("destroy:"));
    }
    false
}
pub(crate) fn validate_action(action: &str) -> Result<(), String> {
    if matches!(action, "win" | "lose" | "restart" | "checkpoint" | "respawn") {
        return Ok(());
    }
    // flow: `go:STATE`, `restart:STATE`, `level:LEVEL` (the names are checked against the declarations once everything is read), and the
    // optional selector of `checkpoint:SELECTOR` and `respawn:SELECTOR`
    if let Some(name) = action.strip_prefix("go:").or_else(|| action.strip_prefix("restart:")).or_else(|| action.strip_prefix("level:")) {
        return nonempty(name, "this action needs a name");
    }
    if let Some(selector) = action.strip_prefix("checkpoint:").or_else(|| action.strip_prefix("respawn:")) {
        return validate_selector(selector);
    }
    // party: `target:next|prev`, `dash:SELECTOR:SPEED:SECONDS`, `iframes:SELECTOR:SECONDS`, `blast:CENTRE:RADIUS:AMOUNT[@FOES]`
    if matches!(action, "target:next" | "target:prev") {
        return Ok(());
    }
    if let Some(body) = action.strip_prefix("dash:") {
        let shape = "dash needs a selector, a speed and seconds, such as dash:tag:hero:12:0.25";
        let (rest, seconds) = body.rsplit_once(':').ok_or(shape)?;
        let (selector, speed) = rest.rsplit_once(':').ok_or(shape)?;
        number(seconds).and(number(speed)).map_err(|_| shape.to_owned())?;
        return validate_selector(selector);
    }
    if let Some(body) = action.strip_prefix("iframes:") {
        let shape = "iframes needs a selector and seconds, such as iframes:tag:hero:0.4";
        let (selector, seconds) = body.rsplit_once(':').ok_or(shape)?;
        number(seconds).map_err(|_| shape.to_owned())?;
        return validate_selector(selector);
    }
    if let Some(body) = action.strip_prefix("blast:") {
        let shape = "blast needs a centre, a radius and an amount, such as blast:a:5:30@tag:enemy";
        let (spec, foes) = body.split_once('@').map_or((body, None), |(spec, foes)| (spec, Some(foes)));
        let (rest, amount) = spec.rsplit_once(':').ok_or(shape)?;
        let (centre, radius) = rest.rsplit_once(':').ok_or(shape)?;
        number(amount).and(number(radius)).map_err(|_| shape.to_owned())?;
        validate_selector(centre)?;
        return foes.map_or(Ok(()), validate_selector);
    }
    // combat: `hurt:SELECTOR:AMOUNT[@SOURCE]`, `heal:SELECTOR:AMOUNT`, `fire:WEAPON[@SELECTOR]`, `reload:WEAPON[@SELECTOR]`
    if let Some(body) = action.strip_prefix("hurt:").or_else(|| action.strip_prefix("heal:")) {
        let (spec, source) = body.split_once('@').map_or((body, None), |(spec, source)| (spec, Some(source)));
        let Some((selector, _)) = spec.rsplit_once(':').filter(|(_, amount)| number(amount).is_ok()) else {
            return Err("hurt and heal need an amount (a number) after the selector, such as hurt:tag:enemy:10".into());
        };
        validate_selector(selector)?;
        return source.map_or(Ok(()), validate_selector);
    }
    if let Some(body) = action.strip_prefix("fire:").or_else(|| action.strip_prefix("reload:")) {
        let (name, selector) = body.split_once('@').map_or((body, None), |(name, selector)| (name, Some(selector)));
        nonempty(name, "fire and reload need a weapon name")?;
        return selector.map_or(Ok(()), validate_selector);
    }
    if let Some(body) = action.strip_prefix("emit:") {
        return nonempty(body, "emit needs a name");
    }
    if let Some(body) = action.strip_prefix("timer:") {
        let Some((name, seconds)) = body.split_once(':') else {
            return Err("timer needs a name and seconds".into());
        };
        nonempty(name, "timer needs a name")?;
        return number(seconds).map(|_| ());
    }
    if let Some(body) = action.strip_prefix("effect:") {
        if body.starts_with("anim_set:") || body.starts_with("anim_flag:") {
            let (spec, selector) = body.rsplit_once('@').unwrap_or((body, ""));
            let Some((kind, remainder)) = spec.split_once(':') else {
                return Err("effect needs a kind and name".into());
            };
            let Some((name, expression)) = remainder.split_once(':') else {
                return Err("animation parameter effect needs a name and expression".into());
            };
            nonempty(name, "animation parameter effect needs a name")?;
            if name.contains('@') {
                return Err("animation parameter name cannot contain '@'".into());
            }
            nonempty(expression, "animation parameter effect needs an expression")?;
            Expr::parse(expression)?;
            if spec.contains('@') {
                return Err("animation parameter name cannot contain '@'".into());
            }
            if !selector.is_empty() {
                validate_selector(selector)?;
            } else if body.rsplit_once('@').is_some() {
                return Err("bad selector ''".into());
            }
            let _ = kind;
            return Ok(());
        }
        let Some((kind, rest)) = body.split_once(':') else {
            return Err("effect needs a kind and name".into());
        };
        if kind == "camera" {
            // `camera:NAME` or `camera:NAME:SECONDS`
            let (name, seconds) = rest.rsplit_once(':').map_or((rest, None), |(name, seconds)| (name, Some(seconds)));
            nonempty(name, "a camera effect needs a camera name")?;
            return seconds.map_or(Ok(()), |seconds| number(seconds).map(|_| ()));
        }
        if !matches!(
            kind,
            "sfx" | "particles" | "camera_shake" | "message" | "anim"
        ) {
            return Err("unknown effect kind".into());
        }
        let has_target = rest.contains('@');
        let (name, target) = rest.split_once('@').unwrap_or((rest, ""));
        nonempty(name, "effect needs a name")?;
        if has_target {
            validate_selector(target)?;
        }
        return Ok(());
    }
    if let Some(body) = action.strip_prefix("spawn:") {
        let Some((kind, position)) = body.split_once('@') else {
            return Err("spawn needs kind@x,y,z".into());
        };
        nonempty(kind, "spawn needs a kind")?;
        if position.split(',').count() != 3 {
            return Err("spawn position needs x,y,z".into());
        }
        return position
            .split(',')
            .try_for_each(|value| number(value).map(|_| ()));
    }
    if let Some(body) = action
        .strip_prefix("move:")
        .or_else(|| action.strip_prefix("setvel:"))
    {
        let Some((selector, vector)) = body.rsplit_once(':') else {
            return Err("movement action needs selector and vector".into());
        };
        validate_selector(selector)?;
        if vector.split(',').count() != 3 {
            return Err("movement vector needs x,y,z".into());
        }
        return vector
            .split(',')
            .try_for_each(|value| number(value).map(|_| ()));
    }
    validate_simple_action(action)
}

pub(super) fn validate_simple_action(action: &str) -> Result<(), String> {
    if let Some(body) = action.strip_prefix("stun:") {
        let (selector, _) = body
            .rsplit_once(':')
            .ok_or("stun needs selector and seconds")?;
        return validate_selector(selector);
    }
    if let Some(body) = action.strip_prefix("setprop:") {
        let (remainder, value) = body
            .rsplit_once(':')
            .ok_or("setprop needs selector, name, and value")?;
        let (selector, property) = remainder
            .rsplit_once(':')
            .ok_or("setprop needs selector, name, and value")?;
        validate_selector(selector)?;
        nonempty(property, "setprop needs a property name")?;
        return number(value).map(|_| ());
    }
    if let Some(body) = action.strip_prefix("add:") {
        if body.split(':').count() != 2 {
            return Err("add needs a variable and an amount".into());
        }
        return Ok(());
    }
    if action
        .strip_prefix("set:")
        .is_some_and(|body| body.split(':').count() == 2)
    {
        return Ok(());
    }
    if let Some(body) = action.strip_prefix("destroy:") {
        return validate_selector(body);
    }
    Err(format!("unknown action '{action}'"))
}
pub(crate) fn nonempty(value: &str, message: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(message.into())
    } else {
        Ok(())
    }
}
pub(super) fn action_changes(action: &str, name: &str) -> bool {
    action
        .strip_prefix("add:")
        .or_else(|| action.strip_prefix("set:"))
        .is_some_and(|body| body.split(':').next() == Some(name))
}
pub(crate) fn expr_names(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Name(name) => vec![name.clone()],
        Expr::Unary(_, inner) => expr_names(inner),
        Expr::Binary(left, _, right) => expr_names(left)
            .into_iter()
            .chain(expr_names(right))
            .collect(),
        Expr::Number(_) => Vec::new(),
    }
}
pub(super) fn parse_fields(s: &str) -> Result<BTreeMap<String, String>, String> {
    let mut m = BTreeMap::new();
    let mut key_value = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut fields = Vec::new();
    for ch in s.chars() {
        if escaped {
            key_value.push(ch);
            escaped = false;
            continue;
        }
        if quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
            continue;
        }
        if ch.is_whitespace() && !quoted {
            if !key_value.is_empty() {
                fields.push(std::mem::take(&mut key_value));
            }
        } else {
            key_value.push(ch);
        }
    }
    if quoted {
        return Err("unterminated quoted value".into());
    }
    if !key_value.is_empty() {
        fields.push(key_value);
    }
    for x in fields {
        if let Some((k, v)) = x.split_once('=') {
            m.insert(k.into(), v.into());
        } else {
            return Err(format!("missing '=' in '{x}'"));
        }
    }
    Ok(m)
}
pub(super) fn required<'a>(m: &'a BTreeMap<String, String>, k: &str) -> Result<&'a str, String> {
    m.get(k)
        .map(String::as_str)
        .ok_or_else(|| format!("missing {k}=..."))
}
pub(crate) fn number(s: &str) -> Result<f64, String> {
    s.parse().map_err(|_| format!("'{s}' is not a number"))
}
pub(super) fn getnum(m: &BTreeMap<String, String>, k: &str, default: f64) -> Result<f64, String> {
    m.get(k).map_or(Ok(default), |s| number(s))
}
