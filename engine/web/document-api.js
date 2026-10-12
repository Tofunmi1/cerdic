// The page's handle on the authoring document. The engine owns the document; this wrapper turns its
// JSON-text functions into plain objects and builds batches from the current revision, so callers
// only list commands. Nothing here holds document state.

export function createDocApi(wasm) {
  const json = (text) => JSON.parse(text);
  let counter = 0;
  const key = () => `t_${Date.now().toString(36)}${(counter++).toString(36)}`;

  const api = {
    open: (name = "default", seed = 1) => json(wasm.doc_open(name, seed)),
    reset: (name = "default", seed = 1) => json(wasm.doc_reset(name, seed)),
    state: () => json(wasm.doc_state()),
    operators: () => json(wasm.doc_operators()),
    undo: () => json(wasm.doc_undo()),
    redo: () => json(wasm.doc_redo()),
    save: () => json(wasm.doc_save()),
    // A raw batch (object or text); the engine checks and applies it atomically.
    apply: (batch) => json(wasm.doc_apply(typeof batch === "string" ? batch : JSON.stringify(batch))),
    // Commands against the current revision with a fresh transaction key.
    send(commands, label) {
      const state = api.state();
      return api.apply({
        doc: state.id,
        base_revision: state.revision,
        transaction: key(),
        label,
        commands,
      });
    },
  };
  return api;
}
