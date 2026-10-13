// The outliner: every shown part with the nodes that build it underneath. Clicking a row selects the
// node (the engine lights up what it builds); a red dot marks a node a diagnostic names.
import { diagnosticsByNode, outlineRows } from "./graph.js";

export function createOutliner({ root, selection, document: documentOf, problems }) {
  const collapsed = new Set();
  let lastSignature = "";
  const el = (tag, className, text) => {
    const node = globalThis.document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };

  function render() {
    const doc = documentOf();
    const bad = diagnosticsByNode(problems());
    const rows = outlineRows(doc, collapsed);
    const signature = JSON.stringify([rows, selection.node, [...bad.entries()]]);
    if (signature === lastSignature) return;
    lastSignature = signature;
    if (!rows.length) {
      root.replaceChildren(el("div", "empty", "Nothing here yet: add a shape or send some text."));
      return;
    }
    root.replaceChildren(
      ...rows.map((row) => {
        if (row.header) return el("div", "ohead", row.header);
        // A row is a node, or the header of a group (whose id is the group's).
        const id = row.group ?? row.id;
        const item = el("div", row.group ? "orow ogroup" : "orow");
        item.dataset.id = id;
        item.tabIndex = 0;
        item.setAttribute("role", "treeitem");
        item.style.paddingLeft = `${4 + (row.depth ?? 0) * 14}px`;
        item.classList.toggle("selected", selection.node === id);
        item.classList.toggle("muted", !!row.muted);
        item.classList.toggle("unused", !!row.unused);
        const expandable = row.group ? true : row.children;
        const caret = el("span", "caret", expandable ? (row.collapsed ? "▸" : "▾") : "");
        if (expandable) {
          caret.addEventListener("click", (event) => {
            event.stopPropagation();
            if (collapsed.has(id)) collapsed.delete(id);
            else collapsed.add(id);
            render();
          });
        }
        item.append(caret, el("span", "oname", row.reference ? `↗ ${row.name}` : row.name));
        if (row.group) {
          item.append(el("span", "obadge", `v${row.version}`), el("span", "oop", `${row.instances} use${row.instances === 1 ? "" : "s"}`));
        } else {
          if (row.output) item.append(el("span", "obadge part", row.output));
          if (row.muted) item.append(el("span", "obadge", "muted"));
          if (row.bypass) item.append(el("span", "obadge", "bypass"));
          if (row.outdated) item.append(el("span", "obadge warn", "outdated"));
          item.append(el("span", "oop", row.instanceOf ?? row.op));
        }
        if (bad.has(id)) {
          const dot = el("span", "odot", "●");
          dot.title = bad.get(id).map((d) => `${d.code}: ${d.message}`).join("\n");
          item.append(dot);
        }
        const choose = () => selection.select(selection.node === id ? null : id);
        item.addEventListener("click", choose);
        item.addEventListener("keydown", (event) => {
          if (event.key === "Enter" || event.key === " ") { event.preventDefault(); choose(); }
        });
        return item;
      }),
    );
  }

  return { render };
}
