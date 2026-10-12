// The inspector: the parameters of the selected node as widgets generated from its operator spec
// (units, kinds and ranges come from the engine, none are written here). Every edit is the same
// typed command a model or a script would send; the engine refuses bad values and says why.
import { diagnosticsByNode, family, fromDegrees, nodeName, show, toDegrees } from "./graph.js";

/// How a number is shown: lengths in metres, angles in degrees, one pixel of dragging moves `drag`.
const UNITS = {
  length: { suffix: "m", to: (v) => v, from: (v) => v, drag: 0.001 },
  angle: { suffix: "°", to: toDegrees, from: fromDegrees, drag: 1 },
  ratio: { suffix: "", to: (v) => v, from: (v) => v, drag: 0.01 },
  none: { suffix: "", to: (v) => v, from: (v) => v, drag: 0.01 },
};

export function createInspector({ root, api, selection, document: documentOf, problems, onOutcome }) {
  let rejected = []; // what the engine said about the last refused edit of the shown node
  let shownNode = null;
  let lastSignature = "";

  const el = (tag, className, text) => {
    const node = globalThis.document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };
  const specs = () => new Map(api.operators().map((spec) => [spec.id, spec]));

  function send(node, command, label) {
    const outcome = api.send([command], label);
    rejected = outcome?.rejected?.diagnostics ?? [];
    onOutcome(outcome);
  }

  /// A number field in display units. Dragging the caption scrubs it; one edit is one command.
  function numberField(value, unit, whole, commit) {
    const u = UNITS[unit] ?? UNITS.none;
    const input = el("input", "num");
    input.type = "number";
    input.step = whole ? "1" : "any";
    input.value = show(u.to(value));
    input.addEventListener("change", () => {
      const typed = Number(input.value);
      if (input.value.trim() === "" || !Number.isFinite(typed)) { input.value = show(u.to(value)); return; }
      commit(whole ? Math.round(typed) : u.from(typed));
    });
    const wrap = el("span", "field");
    wrap.append(input);
    if (u.suffix) wrap.append(el("span", "unit", u.suffix));
    const scrub = (caption) => {
      caption.classList.add("scrub");
      caption.addEventListener("pointerdown", (down) => {
        const start = Number(input.value);
        const per = whole ? 0.1 : u.drag;
        caption.setPointerCapture(down.pointerId);
        const move = (event) => { input.value = show(start + (event.clientX - down.clientX) * per); };
        const up = () => {
          caption.removeEventListener("pointermove", move);
          caption.removeEventListener("pointerup", up);
          if (Number(input.value) !== start) input.dispatchEvent(new Event("change"));
        };
        caption.addEventListener("pointermove", move);
        caption.addEventListener("pointerup", up);
      });
    };
    return { wrap, input, scrub };
  }

  function widget(param, value, commit) {
    const ty = param.ty;
    const unit = ty.unit ?? "none";
    switch (ty.type) {
      case "num": case "int": {
        const field = numberField(value, unit, ty.type === "int", commit);
        return { control: field.wrap, scrub: field.scrub };
      }
      case "bool": {
        const box = el("input");
        box.type = "checkbox";
        box.checked = !!value;
        box.addEventListener("change", () => commit(box.checked));
        return { control: box };
      }
      case "choice": {
        const select = el("select");
        for (const option of ty.options) select.append(new Option(option, option, false, option === value));
        select.addEventListener("change", () => commit(select.value));
        return { control: select };
      }
      case "str": {
        const input = el("input");
        input.type = "text";
        input.value = value ?? "";
        input.addEventListener("change", () => commit(input.value));
        return { control: input };
      }
      case "vec2": case "vec3": {
        const group = el("span", "vec");
        const count = ty.type === "vec2" ? 2 : 3;
        const parts = Array.from({ length: count }, (_, i) => numberField(value[i], unit, false, (v) => {
          commit(parts.map((part, k) => (k === i ? v : (UNITS[unit] ?? UNITS.none).from(Number(part.input.value)))));
        }));
        group.append(...parts.map((part) => part.wrap));
        return { control: group };
      }
      default: { // points2: a list of [x, y]; edited as JSON
        const area = el("textarea", "points");
        area.value = JSON.stringify(value);
        area.rows = 3;
        area.spellcheck = false;
        area.addEventListener("change", () => {
          try { commit(JSON.parse(area.value)); }
          catch { area.classList.add("invalid"); }
        });
        return { control: area };
      }
    }
  }

  function render() {
    const doc = documentOf();
    const node = doc.nodes.find((candidate) => candidate.id === selection.node);
    if (node?.id !== shownNode) { rejected = []; shownNode = node?.id ?? null; }
    // What the engine said about this node, plus the reasons it refused the last edit made here.
    const mine = node ? [...(diagnosticsByNode(problems()).get(node.id) ?? []), ...rejected] : [];
    // Rebuilding replaces the fields, so do it only when something shown has changed: a field being
    // typed into must not lose focus because an evaluation finished.
    const signature = JSON.stringify([node ?? null, mine]);
    if (signature === lastSignature) return;
    lastSignature = signature;
    if (!node) {
      root.replaceChildren(el("div", "empty", "Select a node in the outliner, or click a part in the viewport."));
      return;
    }
    const spec = specs().get(node.op);
    const out = [];

    const title = el("input", "title");
    title.value = node.label ?? "";
    title.placeholder = family(node.op);
    title.title = "Name";
    title.addEventListener("change", () => send(node, { set_label: { node: node.id, label: title.value.trim() || null } }, "Rename"));
    out.push(title, el("div", "ophelp", `${node.op}${spec ? ` · ${spec.doc}` : ""} · ${node.id}`));

    for (const input of spec?.inputs ?? []) {
      const links = node.inputs[input.name] ?? [];
      const line = el("div", "irow");
      line.append(el("span", "cap", input.name));
      const names = el("span", "links");
      if (!links.length) names.append(el("span", "none", "nothing connected"));
      for (const link of links) {
        const source = doc.nodes.find((candidate) => candidate.id === link.node);
        const button = el("button", "link", source ? nodeName(source) : link.node);
        button.addEventListener("click", () => selection.select(link.node));
        names.append(button);
      }
      line.append(names);
      out.push(line);
    }

    for (const param of spec?.params ?? []) {
      const value = node.params[param.name] ?? param.default;
      const row = el("div", "irow");
      const caption = el("label", "cap", param.name);
      caption.title = param.doc;
      const { control, scrub } = widget(param, value, (next) =>
        send(node, { set_param: { node: node.id, param: param.name, value: next } }, `Set ${param.name}`));
      scrub?.(caption);
      row.append(caption, control);
      out.push(row);
      for (const issue of mine.filter((d) => d.param === param.name)) out.push(el("div", "issue", `${issue.code}: ${issue.message}`));
    }

    const flags = el("div", "irow");
    for (const flag of ["muted", "bypass"]) {
      const label = el("label", "flag");
      const box = el("input");
      box.type = "checkbox";
      box.checked = !!node.flags?.[flag];
      box.addEventListener("change", () => send(node, { set_flags: { node: node.id, [flag]: box.checked } }, flag === "muted" ? "Mute" : "Bypass"));
      label.append(box, ` ${flag}`);
      flags.append(label);
    }
    const remove = el("button", "danger", "Delete");
    remove.addEventListener("click", () => send(node, { remove_node: { node: node.id, cascade: false } }, "Delete node"));
    flags.append(remove);
    out.push(flags);

    for (const issue of mine.filter((d) => !d.param)) out.push(el("div", "issue", `${issue.code}: ${issue.message}`));
    root.replaceChildren(...out);
  }

  return { render };
}
