// The selected node, shared between the panels and the viewport. Selecting in a panel tells the
// engine which parts of the model to light up (the ones the node builds); clicking a part in the
// viewport comes back here as a selection. Selection is view state: never saved, never a command.
import { partsFor } from "./graph.js";

export function createSelection({ runtime, state }) {
  let node = null;
  let seen = -1; // the engine's generation we last looked at
  const listeners = new Set();
  const emit = () => listeners.forEach((listener) => listener());

  const look = () => JSON.parse(runtime.selection_get());
  const push = () => {
    if (node) runtime.selection_set(node, JSON.stringify(partsFor(state().document, node)));
    else runtime.selection_clear();
    seen = look().generation;
  };

  // A click on a part in the viewport changes the engine's selection behind our back.
  const timer = setInterval(() => {
    const engine = look();
    if (engine.generation === seen) return;
    seen = engine.generation;
    if (engine.node !== node) { node = engine.node; emit(); }
  }, 100);

  return {
    get node() { return node; },
    select(id) { node = id ?? null; push(); emit(); },
    /// After the document changed: forget a node that no longer exists, and re-aim at the parts it builds.
    refresh() {
      if (node && !state().document.nodes.some((candidate) => candidate.id === node)) node = null;
      push();
    },
    subscribe(listener) { listeners.add(listener); return () => listeners.delete(listener); },
    dispose() { clearInterval(timer); },
  };
}
