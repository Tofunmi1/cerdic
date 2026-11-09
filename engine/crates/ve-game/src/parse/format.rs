use super::{InputDef, KindDef};
use std::fmt::Write as _;

pub(super) fn kind_text(kind: &KindDef) -> String {
    let mut line = format!(
        "kind {} size={} speed={}",
        kind.name,
        number_text(kind.size),
        number_text(kind.speed)
    );
    if !kind.tags.is_empty() {
        let _ = write!(line, " tags={}", kind.tags.join(","));
    }
    if let Some(part) = &kind.part {
        let _ = write!(line, " part={part}");
    }
    if let Some(solid) = kind.solid {
        let _ = write!(
            line,
            " solid={},{},{}",
            number_text(solid[0]),
            number_text(solid[1]),
            number_text(solid[2])
        );
    }
    if kind.grab {
        line.push_str(" grab=1");
    }
    for (flag, name) in [
        (kind.roll, "roll"),
        (kind.swing, "swing"),
        (kind.pocket, "pocket"),
    ] {
        if flag {
            let _ = write!(line, " {name}=1");
        }
    }
    for (value, default, name) in [
        (kind.reach, 0.0_f64, "reach"),
        (kind.loft, 0.0, "loft"),
        (kind.power, 1.0, "power"),
    ] {
        if value.to_bits() != default.to_bits() {
            let _ = write!(line, " {name}={}", number_text(value));
        }
    }
    if kind.fall {
        line.push_str(" fall=1");
        let _ = write!(line, " bounce={}", number_text(kind.bounce));
    } else if kind.bounce.to_bits() != 0.3_f64.to_bits() {
        let _ = write!(line, " bounce={}", number_text(kind.bounce));
    }
    line
}

pub(super) fn input_text(input: &InputDef) -> String {
    let mut line = format!("input {}", input.name);
    if !input.axes.is_empty() {
        let _ = write!(line, " axis={}", input.axes.join(","));
    }
    if let Some(target) = &input.target {
        let _ = write!(line, " target={target}");
    }
    if input.follow {
        line.push_str(" follow=viewer");
    }
    if let Some(side) = input.hand_follow {
        line.push_str(match side {
            crate::HandSide::Left => " follow=left_hand",
            crate::HandSide::Right => " follow=right_hand",
        });
    }
    if input.key {
        line.push_str(" key=press");
    }
    line.push_str(&super::bindings::write_bindings(&input.bindings));
    line
}

pub(super) fn number_text(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        value.to_string()
    }
}

pub(super) fn steps_text(steps: u64) -> String {
    let whole = steps / 60;
    let fractional = (steps % 60) * 1_000_000 / 60;
    if fractional == 0 {
        whole.to_string()
    } else {
        let mut fraction = format!("{fractional:06}");
        while fraction.ends_with('0') {
            fraction.pop();
        }
        format!("{whole}.{fraction}")
    }
}
