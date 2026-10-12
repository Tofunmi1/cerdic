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
        const item = el("div", "orow");
        item.dataset.id = row.id;
        item.tabIndex = 0;
        item.setAttribute("role", "treeitem");
        item.style.paddingLeft = `${4 + row.depth * 14}px`;
        item.classList.toggle("selected", selection.node === row.id);
        item.classList.toggle("muted", row.muted);
        const caret = el("span", "caret", row.children ? (row.collapsed ? "▸" : "▾") : "");
        if (row.children) {
          caret.addEventListener("click", (event) => {
            event.stopPropagation();
            if (collapsed.has(row.id)) collapsed.delete(row.id);
            else collapsed.add(row.id);
            render();
          });
        }
        item.append(caret, el("span", "oname", row.reference ? `↗ ${row.name}` : row.name));
        if (row.output) item.append(el("span", "obadge part", row.output));
        if (row.muted) item.append(el("span", "obadge", "muted"));
        if (row.bypass) item.append(el("span", "obadge", "bypass"));
        item.append(el("span", "oop", row.op));
        if (bad.has(row.id)) {
          const dot = el("span", "odot", "●");
          dot.title = bad.get(row.id).map((d) => `${d.code}: ${d.message}`).join("\n");
          item.append(dot);
        }
        const choose = () => selection.select(selection.node === row.id ? null : row.id);
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
