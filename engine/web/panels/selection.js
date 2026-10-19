// The selected node, shared between the panels and the viewport. Selecting in a panel tells the
// engine which parts of the model to light up (the ones the node builds); clicking a part in the
// viewport comes back here as a selection. Selection is view state: never saved, never a command.
import { featureCandidates, partsFor } from "./graph.js";

export function createSelection({ runtime, state }) {
  let node = null;
  let instance = null;
  let socket = null;
  let lost = null;
  let featureMode = false; // a viewport click picks the authored feature under the part, not the part
  let ambiguity = null; // `{ part, at, candidates }` when a feature pick could mean several nodes
  let seen = -1; // the engine's generation we last looked at
  const listeners = new Set();
  const emit = () => listeners.forEach((listener) => listener());

  const look = () => JSON.parse(runtime.selection_get());
  const push = (preserveInstance = false) => {
    const parts = partsFor(state().document,node);
    const current = look();
    if (preserveInstance && current.node===node && JSON.stringify([...current.parts].sort())===JSON.stringify([...parts].sort())) {
      seen=current.generation; return;
    }
    if (node) runtime.selection_set(node, JSON.stringify(parts));
    else runtime.selection_clear();
    seen = look().generation;
  };

  // A click on a part in the viewport changes the engine's selection behind our back.
  const timer = setInterval(() => {
    const engine = look();
    if (engine.generation === seen) return;
    seen = engine.generation;
    if (engine.node !== node || engine.instance !== instance) {
      node = engine.node; instance=engine.instance ?? null; socket=null; lost=null; ambiguity=null;
      if (featureMode && node && instance === null) resolveFeature();
      emit();
    }
  }, 100);

  /// Narrows a clicked part to the feature that builds it (see `featureCandidates`); the part stays
  /// selected, and says why, when the click could mean several operands.
  function resolveFeature() {
    const found = featureCandidates(state().document, node);
    if (found.node && found.node !== node) { node = found.node; push(); }
    else if (found.ambiguous) ambiguity = { part: node, at: found.at, candidates: found.ambiguous };
  }

  return {
    get node() { return node; },
    get instance() { return instance; },
    get socket() { return socket; },
    get lost() { return lost; },
    get ambiguity() { return ambiguity; },
    get featureMode() { return featureMode; },
    setFeatureMode(on) { featureMode = Boolean(on); ambiguity = null; emit(); },
    get instanceCount() {
      const sets=JSON.parse(runtime.eval_projection_status()).instance_sets ?? {};
      return partsFor(state().document,node).reduce((sum,id)=>sum+(sets[id] ?? 0),0);
    },
    select(id) { node = id ?? null; instance=null; socket=null; lost=null; ambiguity=null; push(); emit(); },
    selectSocket(part,name) { socket=socket?.part===part&&socket?.name===name ? null : {part,name}; emit(); },
    /// After the document changed: forget a node that no longer exists, and re-aim at the parts it builds.
    refresh() {
      const doc = state().document;
      const exists = doc.nodes.some((candidate) => candidate.id === node) || doc.groups?.some((g) => g.id === node);
      if (node && !exists) { lost = node; node = null; instance = null; socket = null; }
      if (ambiguity && !ambiguity.candidates.every((id) => doc.nodes.some((n) => n.id === id))) ambiguity = null;
      push(true);
    },
    subscribe(listener) { listeners.add(listener); return () => listeners.delete(listener); },
    dispose() { clearInterval(timer); },
  };
}
