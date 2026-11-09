use super::helpers::{
    getnum, number, parse_fields, required, valid_event, validate_action, validate_selector,
};
use super::{Diagnostic, Expr, GameDef, Goal, InputDef, KindDef, Rule, SpawnDef, VariableDef};
use std::collections::BTreeMap;

pub(super) fn parse_lines(text: &str, definition: &mut GameDef) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        if let Err(message) = parse_line(raw, line, definition) {
            let suggestion = if let Some(suggestion) = super::rolling::suggestion(raw, &message) {
                suggestion
            } else if raw.trim_start().starts_with("hud ") {
                format!(
                    "check HUD text and variables: {}",
                    definition
                        .vars
                        .iter()
                        .map(|v| v.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else if raw.trim_start().starts_with("spawn ") && message.contains("heading") {
                "set heading to a finite number of degrees, for example heading=90".to_owned()
            } else if raw.trim_start().starts_with("agent ") {
                "declare the kind first; use patrol=0,0;1,1 speed=1 with finite positive speed, chase_speed, sight and lose".to_owned()
            } else if raw.trim_start().starts_with("nav ") {
                "use nav parts=floor cell=0.25 radius=0.3 height=1.8 step=0.4 slope=45; cell and height must be positive, radius and step nonnegative, slope between 0 and 90".to_owned()
            } else {
                "check the statement name and required fields".to_owned()
            };
            errors.push(Diagnostic::new(
                line,
                super::rolling::error_column(raw, &message)
                    .or_else(|| super::agent::error_column(raw, &message))
                    .or_else(|| super::controller::error_column(raw, &message))
                    .or_else(|| crate::camera::error_column(raw, &message))
                    .unwrap_or_else(|| token_column(raw)),
                message,
                suggestion,
            ));
        }
    }
    errors
}

fn parse_line(raw: &str, line: usize, definition: &mut GameDef) -> Result<(), String> {
    let source = strip_comment(raw).trim();
    if source.is_empty() {
        return Ok(());
    }
    let words = tokenize(source)?;
    let mut tokens = words.iter().map(String::as_str);
    let command = tokens.next().unwrap_or("");
    let positional = if matches!(
        command,
        "kind" | "spawn" | "goal" | "input" | "agent" | "surface" | "controller" | "camera" | "state" | "level" | "ui" | "health" | "projectile" | "weapon" | "brain" | "ability"
    ) {
        tokens.next()
    } else {
        None
    };
    if command == "var" {
        for token in tokens {
            let Some((name, value)) = token.split_once('=') else {
                return Err(format!("missing '=' in '{token}'"));
            };
            definition.vars.push(VariableDef {
                name: name.to_owned(),
                value: number(value)?,
            });
        }
        return Ok(());
    }
    let rest = tokens.collect::<Vec<_>>().join(" ");
    let fields = parse_fields(&rest)?;
    parse_statement(definition, command, positional, &fields, line)
}

fn parse_statement(
    definition: &mut GameDef,
    command: &str,
    positional: Option<&str>,
    fields: &BTreeMap<String, String>,
    line: usize,
) -> Result<(), String> {
    match command {
        "game" => {
            definition.name = required(fields, "name")?.into();
            Ok(())
        }
        "ground" => {
            definition.ground = required(fields, "y")?
                .parse::<f64>()
                .map_err(|_| "invalid ground height".to_owned())?;
            if !definition.ground.is_finite() {
                return Err("invalid ground height".into());
            }
            Ok(())
        }
        "kind" => parse_kind(definition, positional, fields),
        "surface" => super::rolling::parse_surface(definition, positional, fields),
        "terrain" => super::rolling::parse_terrain(definition, fields),
        "agent" => super::agent::parse_agent(definition, positional, fields),
        "controller" => super::controller::parse_controller(definition, positional, fields),
        "camera" => crate::camera::parse_camera(definition, positional, fields),
        "ui" => crate::ui::tree::parse_ui(definition, positional, fields, line),
        "health" => super::combat::parse_health(definition, positional, fields),
        "projectile" => super::combat::parse_projectile(definition, positional, fields),
        "weapon" => super::combat::parse_weapon(definition, positional, fields),
        "brain" => super::party::parse_brain(definition, positional, fields),
        "targeting" => super::party::parse_targeting(definition, fields),
        "ability" => super::party::parse_ability(definition, positional, fields),
        "state" => super::flow::parse_state(definition, positional, fields),
        "level" => super::flow::parse_level(definition, positional, fields),
        "nav" => super::agent::parse_nav(definition, fields),
        "spawn" => parse_spawn(definition, positional, fields),
        "input" => parse_input(definition, positional, fields),
        "rule" => parse_rule(definition, fields, line),
        "goal" => parse_goal(definition, positional, fields),
        "hud" => super::hud::parse_hud(definition, fields),
        "script" => super::script::parse_script(definition, fields),
        _ => Err(format!("unknown statement '{command}'")),
    }
}

fn strip_comment(raw: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in raw.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
        }
        if ch == '#' && !quoted {
            return &raw[..index];
        }
    }
    raw
}

fn tokenize(source: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut started = false;
    for ch in source.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
            continue;
        }
        if quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
            word.push(ch);
            started = true;
            continue;
        }
        if ch.is_whitespace() && !quoted {
            if started {
                words.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(ch);
            started = true;
        }
    }
    if quoted {
        return Err("unterminated quoted value".into());
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

fn parse_kind(
    definition: &mut GameDef,
    positional: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    let name = positional.ok_or("a kind needs a name")?.to_owned();
    for (old, new) in [("strike", "swing"), ("cup", "pocket")] {
        if fields.contains_key(old) {
            return Err(format!("unknown kind key '{old}'; use {new}"));
        }
    }
    let solid = fields
        .get("solid")
        .map(|value| {
            let parts = value
                .split(',')
                .map(super::helpers::number)
                .collect::<Result<Vec<_>, _>>()?;
            if parts.len() != 3
                || parts
                    .iter()
                    .any(|extent| !extent.is_finite() || *extent <= 0.0)
            {
                return Err("solid needs exactly three positive finite extents".to_owned());
            }
            Ok([parts[0], parts[1], parts[2]])
        })
        .transpose()
        .map_err(|_| "invalid solid extents".to_owned())?;
    let tags = fields
        .get("tags")
        .map(|value| value.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    definition.kinds.push(KindDef {
        name,
        size: getnum(fields, "size", 0.5)?,
        speed: getnum(fields, "speed", 0.0)?,
        tags,
        part: fields.get("part").cloned(),
        solid,
        grab: fields.get("grab").is_some_and(|value| value == "1"),
        roll: super::rolling::flag(fields, "roll")?,
        swing: super::rolling::flag(fields, "swing")?,
        pocket: super::rolling::flag(fields, "pocket")?,
        reach: super::rolling::bounded(fields, "reach", 0.0, 0.0, f64::MAX)?,
        loft: super::rolling::bounded(fields, "loft", 0.0, 0.0, 80.0)?,
        power: super::rolling::positive(fields, "power", 1.0, 5.0)?,
        fall: match fields.get("fall").map_or("0", String::as_str) {
            "0" => false,
            "1" => true,
            _ => return Err("invalid fall value".into()),
        },
        bounce: {
            let value = getnum(fields, "bounce", 0.3)?;
            if !(0.0..=0.95).contains(&value) {
                return Err("bounce must be from 0 to 0.95".into());
            }
            value
        },
        props: BTreeMap::new(),
    });
    Ok(())
}

fn parse_spawn(
    definition: &mut GameDef,
    positional: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    let kind = positional.ok_or("missing kind name")?.to_owned();
    if !definition.kinds.iter().any(|item| item.name == kind) {
        return Err(format!("unknown kind '{kind}'"));
    }
    let xyz: Vec<f64> = required(fields, "at")?
        .split(',')
        .map(number)
        .collect::<Result<_, _>>()?;
    if xyz.len() != 3 {
        return Err("position needs x,y,z".into());
    }
    let count = fields.get("count").map_or(Ok(1), |value| {
        value.parse().map_err(|_| "count must be an integer")
    })?;
    let heading = match fields.get("heading") {
        Some(value) => {
            let degrees =
                number(value).map_err(|_| "heading must be a finite number of degrees")?;
            if !degrees.is_finite() {
                return Err("heading must be a finite number of degrees".into());
            }
            degrees.to_radians()
        }
        None => 0.0,
    };
    definition.spawns.push(SpawnDef {
        kind,
        position: [xyz[0], xyz[1], xyz[2]],
        heading,
        count,
        spread: getnum(fields, "spread", 0.0)?,
        level: fields.get("level").cloned(),
    });
    Ok(())
}

fn parse_input(
    definition: &mut GameDef,
    positional: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    let name = positional.ok_or("input needs a name")?.to_owned();
    let axes: Vec<String> = fields
        .get("axis")
        .map(|value| value.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    let target = fields.get("target").cloned();
    if let Some(selector) = &target {
        validate_selector(selector)?;
    }
    let follow_value = fields.get("follow").map(String::as_str);
    let follow = follow_value == Some("viewer");
    let hand_follow = match follow_value {
        None | Some("" | "viewer") => None,
        Some("left_hand") => Some(crate::HandSide::Left),
        Some("right_hand") => Some(crate::HandSide::Right),
        Some(_) => return Err("unknown follow source".into()),
    };
    let key = fields.get("key").is_some_and(|value| value == "press");
    let bindings = super::bindings::parse_bindings(axes.len(), fields)?;
    definition.inputs.push(InputDef {
        name,
        axes,
        target,
        key,
        follow,
        hand_follow,
        bindings,
    });
    Ok(())
}

fn parse_rule(
    definition: &mut GameDef,
    fields: &BTreeMap<String, String>,
    line: usize,
) -> Result<(), String> {
    let event = required(fields, "on")?.to_owned();
    if !valid_event(&event) {
        return Err(format!("unknown event '{event}'"));
    }
    let when = fields.get("when").cloned();
    let expr = when.as_deref().map(Expr::parse).transpose()?;
    let actions: Vec<String> = fields
        .get("do")
        .map(|value| {
            value
                .split(';')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    for action in &actions {
        validate_action(action)?;
    }
    for selector in [fields.get("a"), fields.get("b")].into_iter().flatten() {
        validate_selector(selector)?;
    }
    let every = fields
        .get("every")
        .map(|value| {
            value
                .parse::<f64>()
                .map(seconds_to_steps)
                .map_err(|_| "every must be a number")
        })
        .transpose()?;
    definition.rules.push(Rule {
        event,
        when,
        actions,
        a: fields.get("a").cloned(),
        b: fields.get("b").cloned(),
        every,
        state: fields.get("state").cloned(),
        expr,
        line,
    });
    Ok(())
}

fn parse_goal(
    definition: &mut GameDef,
    positional: Option<&str>,
    fields: &BTreeMap<String, String>,
) -> Result<(), String> {
    let win = match positional.ok_or("a goal needs when=")? {
        "win" => true,
        "lose" => false,
        _ => return Err("goal must say win or lose".into()),
    };
    let when = fields.get("when").ok_or("a goal needs when=")?.to_owned();
    let expr = Expr::parse(&when)?;
    definition.goals.push(Goal { win, when, expr });
    Ok(())
}

fn token_column(raw: &str) -> usize {
    raw.chars()
        .position(|character| !character.is_whitespace())
        .map_or(1, |index| index + 1)
}
fn seconds_to_steps(seconds: f64) -> u64 {
    let steps = (seconds * 60.0).round().max(1.0);
    steps.to_string().parse().unwrap_or(1)
}
