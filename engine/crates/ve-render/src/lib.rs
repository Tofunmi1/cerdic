//! Render core for the engine.
//!
//! Built on the vendored Bevy render crates (`third_party/bevy`). M1 only establishes that the
//! trimmed crate set builds for the browser; the schedule, driver and pipelines arrive in M4.

use bevy_app::{App, Plugin};

/// Registers the engine's render systems. Empty until milestone M4.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeRenderPlugin;

impl Plugin for VeRenderPlugin {
    fn build(&self, _app: &mut App) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds_into_an_app() {
        let mut app = App::new();
        app.add_plugins(VeRenderPlugin);
    }
}
