use super::World;
use std::fmt::Write as _;

impl World {
    /// HUD text with each declared variable placeholder replaced by its current value.
    #[must_use]
    pub fn hud_lines(&self) -> Vec<String> {
        self.def
            .huds
            .iter()
            .map(|hud| {
                let mut output = String::new();
                let mut rest = hud.text.as_str();
                while let Some(open) = rest.find('{') {
                    output.push_str(&rest[..open]);
                    rest = &rest[open + 1..];
                    let Some(close) = rest.find('}') else {
                        output.push('{');
                        continue;
                    };
                    let name = &rest[..close];
                    if let Some(value) = self.vars.get(name) {
                        if value.fract() == 0.0 {
                            let _ = write!(output, "{value:.0}");
                        } else {
                            let _ = write!(output, "{value:.1}");
                        }
                    } else {
                        output.push('{');
                        output.push_str(name);
                        output.push('}');
                    }
                    rest = &rest[close + 1..];
                }
                output.push_str(rest);
                output
            })
            .collect()
    }
}
