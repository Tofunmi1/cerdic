//! The `layer` and `event` statements of the state machine text format.
//!
//! ```text
//! layer NAME clip=CLIP [weight=PARAM|NUMBER] [additive] [mask=PART,PART] [speed=S] [once] [fade=SECONDS]
//! event CLIP NAME at=SECONDS
//! ```
use crate::layers::{ClipEvent, Extras, Layer, LayerWeight, MAX_EVENTS, MAX_OVERLAYS, plain_name};

fn float(text: &str, what: &str) -> Result<f32, String> {
    text.parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("bad {what} {text:?}; use a finite number"))
}

/// Parses one `layer` statement (the words after the keyword) into `extras`.
pub(crate) fn parse_layer(words: &[&str], extras: &mut Extras) -> Result<(), String> {
    let Some((name, options)) = words.split_first() else {
        return Err("write `layer NAME clip=CLIP ...`".to_owned());
    };
    if !plain_name(name) {
        return Err(format!("bad layer name {name:?}; use letters, digits, _, - or ."));
    }
    if extras.layers.iter().any(|layer| layer.name == *name) {
        return Err(format!("repeated layer {name:?}; layer names are unique"));
    }
    if extras.layers.len() >= MAX_OVERLAYS {
        return Err(format!("more than {MAX_OVERLAYS} layers"));
    }
    let mut layer = Layer {
        name: (*name).to_owned(),
        clip: String::new(),
        weight: LayerWeight::Fixed(1.0),
        additive: false,
        mask: Vec::new(),
        speed: 1.0,
        looping: true,
        fade: 0.2,
    };
    let mut has_clip = false;
    for option in options {
        if let Some(value) = option.strip_prefix("clip=") {
            if value.is_empty() || value.chars().any(char::is_whitespace) {
                return Err("layer clip= needs a clip name".to_owned());
            }
            layer.clip = if value == "\"\"" { String::new() } else { value.to_owned() };
            has_clip = true;
        } else if let Some(value) = option.strip_prefix("weight=") {
            layer.weight = match value.parse::<f32>() {
                Ok(number) if number.is_finite() && (0.0..=1.0).contains(&number) => LayerWeight::Fixed(number),
                Ok(_) => return Err("layer weight must be a number from 0 to 1, or a parameter name".to_owned()),
                Err(_) if plain_name(value) => LayerWeight::Parameter(value.to_owned()),
                Err(_) => return Err(format!("bad layer weight {value:?}; write a number from 0 to 1 or a parameter name")),
            };
        } else if let Some(value) = option.strip_prefix("mask=") {
            layer.mask = value.split(',').filter(|part| !part.is_empty()).map(str::to_owned).collect();
            if layer.mask.is_empty() {
                return Err("layer mask= needs part names separated by commas".to_owned());
            }
        } else if let Some(value) = option.strip_prefix("speed=") {
            layer.speed = float(value, "layer speed")?;
            if layer.speed <= 0.0 {
                return Err("layer speed must be above 0".to_owned());
            }
        } else if let Some(value) = option.strip_prefix("fade=") {
            layer.fade = float(value, "layer fade")?;
            if layer.fade < 0.0 {
                return Err("layer fade cannot be negative".to_owned());
            }
        } else if *option == "additive" {
            layer.additive = true;
        } else if *option == "once" {
            layer.looping = false;
        } else if *option == "loop" {
            layer.looping = true;
        } else {
            return Err(format!("unknown layer option {option:?}; use clip=, weight=, mask=, speed=, fade=, additive or once"));
        }
    }
    if !has_clip {
        return Err("layer plays nothing; add `clip=NAME`".to_owned());
    }
    extras.layers.push(layer);
    Ok(())
}

/// Parses one `event` statement (the words after the keyword) into `extras`.
pub(crate) fn parse_event(words: &[&str], extras: &mut Extras) -> Result<(), String> {
    let [clip, name, at] = words else {
        return Err("write `event CLIP NAME at=SECONDS`".to_owned());
    };
    let Some(time) = at.strip_prefix("at=") else {
        return Err("an event needs its time: `event CLIP NAME at=SECONDS`".to_owned());
    };
    if !plain_name(name) {
        return Err(format!("bad event name {name:?}; use letters, digits, _, - or ."));
    }
    let time = float(time, "event time (at=)")?;
    if time < 0.0 {
        return Err("event time (at=) cannot be negative".to_owned());
    }
    if extras.events.len() >= MAX_EVENTS {
        return Err(format!("more than {MAX_EVENTS} events"));
    }
    extras.events.push(ClipEvent {
        clip: if *clip == "\"\"" { String::new() } else { (*clip).to_owned() },
        name: (*name).to_owned(),
        time,
    });
    Ok(())
}

fn clip_token(clip: &str) -> String {
    if clip.is_empty() { "\"\"".to_owned() } else { clip.to_owned() }
}

/// The text of `extras`, one statement per line, to follow the states and transitions.
pub(crate) fn write(extras: &Extras, out: &mut String) {
    use std::fmt::Write;
    // Writing to a String cannot fail.
    for event in &extras.events {
        let _ = writeln!(out, "event {} {} at={}", clip_token(&event.clip), event.name, event.time);
    }
    for layer in &extras.layers {
        let _ = write!(out, "layer {} clip={}", layer.name, clip_token(&layer.clip));
        match &layer.weight {
            LayerWeight::Fixed(w) => {
                let _ = write!(out, " weight={w}");
            }
            LayerWeight::Parameter(name) => {
                let _ = write!(out, " weight={name}");
            }
        }
        if layer.additive {
            out.push_str(" additive");
        }
        if !layer.mask.is_empty() {
            let _ = write!(out, " mask={}", layer.mask.join(","));
        }
        if layer.speed.to_bits() != 1.0_f32.to_bits() {
            let _ = write!(out, " speed={}", layer.speed);
        }
        if !layer.looping {
            out.push_str(" once");
        }
        if layer.fade.to_bits() != 0.2_f32.to_bits() {
            let _ = write!(out, " fade={}", layer.fade);
        }
        out.push('\n');
    }
}
