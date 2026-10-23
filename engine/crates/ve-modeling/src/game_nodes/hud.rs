use super::{GameFragment, Section, item, number_or, text};
use ve_graph::Node;

pub(super) fn make_hud(node: &Node) -> GameFragment {
    let hud_text = text(node, "text")
        .unwrap_or("")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let anchor = text(node, "anchor").unwrap_or("gaze");
    let size = number_or(node, "size", 0.05);
    item(
        node,
        Section::Hud,
        format!("hud text=\"{hud_text}\" anchor={anchor} size={size}"),
        None,
    )
}
