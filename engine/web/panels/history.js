// The history list and the problems list. History shows every transaction oldest first with a marker
// where the document stands now; clicking one steps undo or redo to it. Problems lists what the
// engine reported (document issues, evaluation failures, refused edits); a row names its node, and
// clicking it selects the node.

export function createHistory({ root, api, onStep }) {
  let lastSignature = "";
  const el = (tag, className, text) => {
    const node = globalThis.document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };

  function render() {
    const state = api.state();
    const { undo = [], redo = [] } = state.history ?? {};
    const readOnly = state.read_only;
    const signature = JSON.stringify([undo, redo, readOnly]);
    if (signature === lastSignature) return;
    lastSignature = signature;
    if (!undo.length && !redo.length) {
      root.replaceChildren(el("div", "empty", "No edits yet."));
      return;
    }
    const rows = [];
    const step = (button, times, move) => button.addEventListener("click", () => {
      if (readOnly) return;
      for (let i = 0; i < times; i++) move();
      onStep();
    });
    undo.forEach((entry, index) => {
      const row = el("button", "hrow done", entry.label);
      row.title = `${entry.changed.length} node${entry.changed.length === 1 ? "" : "s"} changed. Click to go back to just after this edit.`;
      step(row, undo.length - 1 - index, () => api.undo());
      rows.push(row);
    });
    rows.push(el("div", "hnow", "now"));
    redo.forEach((entry, index) => {
      const row = el("button", "hrow undone", entry.label);
      row.title = "Undone. Click to redo up to and including this edit.";
      step(row, index + 1, () => api.redo());
      rows.push(row);
    });
    root.replaceChildren(...rows);
    root.scrollTop = root.scrollHeight;
  }

  return { render };
}

export function createProblems({ root, details, selection, list }) {
  let lastSignature = "";
  let count = 0;
  const el = (tag, className, text) => {
    const node = globalThis.document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };

  function render() {
    const problems = list();
    const signature = JSON.stringify(problems);
    if (signature === lastSignature) return;
    lastSignature = signature;
    // A new problem opens the list so it cannot be missed; the person may close it again.
    if (problems.length > count) details.open = true;
    count = problems.length;
    details.hidden = problems.length === 0;
    details.querySelector("summary").textContent = `Problems (${problems.length})`;
    root.replaceChildren(
      ...problems.map((problem) => {
        const row = el("button", "prow");
        row.append(el("b", "", problem.code), ` ${problem.message}`);
        const id = problem.node ?? problem.related?.[0];
        if (id) {
          row.title = `Select ${id}`;
          row.addEventListener("click", () => selection.select(id));
        } else {
          row.disabled = true;
        }
        return row;
      }),
    );
  }

  return { render };
}
