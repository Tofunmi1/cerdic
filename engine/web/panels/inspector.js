// The inspector: the parameters of the selected node as widgets generated from its operator spec
// (units, kinds and ranges come from the engine, none are written here). Every edit is the same
// typed command a model or a script would send; the engine refuses bad values and says why. A group,
// an instance of one and a node inside one each get the extra controls they need (groups.js).
import { diagnosticsByNode, family, nodeName } from "./graph.js";
import { exposeControl, groupEditor, instanceEditor, memberNotice } from "./groups.js";
import { el, widget } from "./widgets.js";

export function createInspector({ root, api, selection, document: documentOf, problems, onOutcome }) {
  let rejected = []; // what the engine said about the last refused edit of the shown node
  let shownNode = null;
  let lastSignature = "";

  const specs = () => new Map(api.operators().map((spec) => [spec.id, spec]));

  /// Sends one command, or a list that commits together.
  function send(command, label) {
    const outcome = api.send(Array.isArray(command) ? command : [command], label);
    rejected = outcome?.rejected?.diagnostics ?? [];
    onOutcome(outcome);
  }

  /// The name field, flags and delete button every node has, around what is specific to its kind.
  function nodeEditor(doc, node, spec, mine, specific) {
    const out = [];
    const title = el("input", "title");
    title.value = node.label ?? "";
    title.placeholder = family(node.op);
    title.title = "Name";
    title.addEventListener("change", () => send({ set_label: { node: node.id, label: title.value.trim() || null } }, "Rename"));
    const kind = node.instance ? "instance" : node.op;
    out.push(title, el("div", "ophelp", `${kind}${spec && !node.instance ? ` · ${spec.doc}` : ""} · ${node.id}`));
    if (node.group) out.push(memberNotice({ doc, node, selection }));
    out.push(...specific);

    const flags = el("div", "irow");
    for (const flag of ["muted", "bypass"]) {
      const label = el("label", "flag");
      const box = el("input");
      box.type = "checkbox";
      box.checked = !!node.flags?.[flag];
      box.addEventListener("change", () => send({ set_flags: { node: node.id, [flag]: box.checked } }, flag === "muted" ? "Mute" : "Bypass"));
      label.append(box, ` ${flag}`);
      flags.append(label);
    }
    const remove = el("button", "danger", "Delete");
    remove.addEventListener("click", () => send({ remove_node: { node: node.id, cascade: false } }, "Delete node"));
    flags.append(remove);
    out.push(flags);
    for (const issue of mine.filter((d) => !d.param)) out.push(el("div", "issue", `${issue.code}: ${issue.message}`));
    return out;
  }

  /// The inputs and parameters an ordinary node's operator spec describes.
  function operatorRows(doc, node, spec, mine) {
    const out = [];
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
        send({ set_param: { node: node.id, param: param.name, value: next } }, `Set ${param.name}`));
      scrub?.(caption);
      row.append(caption, control);
      const expose = node.group ? exposeControl({ doc, node, param: param.name, send }) : null;
      if (expose) row.append(expose);
      out.push(row);
      for (const issue of mine.filter((d) => d.param === param.name)) out.push(el("div", "issue", `${issue.code}: ${issue.message}`));
    }
    return out;
  }

  function render() {
    const doc = documentOf();
    const group = doc.groups?.find((candidate) => candidate.id === selection.node);
    const node = doc.nodes.find((candidate) => candidate.id === selection.node);
    const shown = group?.id ?? node?.id ?? null;
    if (shown !== shownNode) { rejected = []; shownNode = shown; }
    // What the engine said about this node, plus the reasons it refused the last edit made here.
    const mine = shown ? [...(diagnosticsByNode(problems()).get(shown) ?? []), ...rejected] : [];
    // Rebuilding replaces the fields, so do it only when something shown has changed: a field being
    // typed into must not lose focus because an evaluation finished. Groups and the nodes inside them
    // are part of what an instance or a member shows.
    const signature = JSON.stringify([node ?? null, group ?? null, doc.groups ?? [], node?.instance || group ? doc.nodes.filter((n) => n.group) : null, mine]);
    if (signature === lastSignature) return;
    lastSignature = signature;
    if (group) {
      root.replaceChildren(...groupEditor({ doc, group, send, selection, problems: mine }));
      return;
    }
    if (!node) {
      root.replaceChildren(el("div", "empty", "Select a node in the outliner, or click a part in the viewport."));
      return;
    }
    const spec = specs().get(node.op);
    const specific = node.instance
      ? instanceEditor({ doc, node, specs: specs(), send, selection, problems: mine })
      : operatorRows(doc, node, spec, mine);
    root.replaceChildren(...nodeEditor(doc, node, spec, mine, specific));
  }

  return { render };
}
