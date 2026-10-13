// The inspector for groups: the group itself (its name, exposed parameters, result), an instance of it
// (which parameters it overrides, what it is linked to, whether it is behind), and the notice on a node
// inside a group that editing it edits the definition. Every edit is a typed command; none is local.
import { exposedParams, instanceInfo, nodeName } from "./graph.js";
import { el, widget } from "./widgets.js";

const link = (text, onClick) => {
  const button = el("button", "link", text);
  button.addEventListener("click", onClick);
  return button;
};

/// An edit of a group's interface (exposing a parameter, stopping to, choosing another result) leaves
/// every instance behind the group's new version, and evaluation refuses an instance that is behind.
/// So the edit is sent together with the upgrade of each instance: one transaction, one undo step.
function changeInterface(doc, group, command, label, send) {
  const upgrades = doc.nodes
    .filter((n) => n.instance?.group === group.id)
    .map((n) => ({ upgrade_group: { node: n.id } }));
  send([command, ...upgrades], upgrades.length ? `${label} (upgrading ${upgrades.length})` : label);
}

/// The operator spec's parameter that an exposed parameter stands for.
const specParam = (specs, member, name) => specs.get(member.op)?.params.find((p) => p.name === name);

/// The editor for a group: what it is called, what instances may override, its result, and its uses.
export function groupEditor({ doc, group, send, selection, problems }) {
  const out = [];
  const title = el("input", "title");
  title.value = group.name;
  title.title = "Group name (letters, digits and _)";
  title.addEventListener("change", () => send({ set_group_name: { group: group.id, name: title.value.trim() } }, "Rename group"));
  const uses = doc.nodes.filter((n) => n.instance?.group === group.id);
  out.push(title, el("div", "ophelp", `group · version ${group.version} · ${uses.length} use${uses.length === 1 ? "" : "s"} · ${group.id}`));

  const result = doc.nodes.find((n) => n.id === group.output);
  const resultRow = el("div", "irow");
  resultRow.append(el("span", "cap", "result"), result ? link(nodeName(result), () => selection.select(result.id)) : el("span", "none", "missing"));
  out.push(resultRow);

  out.push(el("div", "ohead", "Instances may override"));
  const exposed = exposedParams(doc, group);
  if (!exposed.length) out.push(el("div", "empty", "No parameter is exposed. Select a node inside the group and expose one."));
  for (const p of exposed) {
    const row = el("div", "irow");
    const remove = el("button", "danger", "×");
    remove.title = "Stop exposing this parameter (changes the group's version)";
    remove.addEventListener("click", () => changeInterface(doc, group, { unexpose_param: { group: group.id, name: p.name } }, `Unexpose ${p.name}`, send));
    row.append(el("span", "cap", p.name), link(`${nodeName(p.node)}.${p.param}`, () => selection.select(p.node.id)), remove);
    out.push(row);
  }

  const buttons = el("div", "irow");
  const add = el("button", "", "Add instance");
  add.addEventListener("click", () => send({ add_instance: { group: group.id, label: `${group.name}_${uses.length + 1}` } }, `Add ${group.name}`));
  const del = el("button", "danger", "Delete group");
  del.addEventListener("click", () => send({ remove_group: { group: group.id } }, "Delete group"));
  buttons.append(add, del);
  out.push(buttons);
  for (const issue of problems) out.push(el("div", "issue", `${issue.code}: ${issue.message}`));
  return out;
}

/// The part of an instance's inspector that is about its group: which group, whether it is behind, what
/// it is linked to, and each exposed parameter with its override. Name, flags and delete are the
/// inspector's own.
export function instanceEditor({ doc, node, specs, send, selection, problems }) {
  const info = instanceInfo(doc, node);
  if (!info) return [el("div", "issue", "The group of this instance is missing.")];
  const out = [];
  const head = el("div", "irow");
  head.append(el("span", "cap", "group"), link(info.group.name, () => selection.select(info.group.id)), el("span", "unit", `v${info.version}`));
  out.push(head);
  if (info.outdated) {
    const dropped = [...info.droppedOverrides.map((n) => `override ${n}`), ...info.droppedLinks.map((n) => `link ${n}`)];
    const banner = el("div", "banner", `Made against version ${info.version}; the group is now version ${info.group.version}. `
      + (dropped.length ? `Upgrading drops: ${dropped.join(", ")}.` : "Upgrading loses nothing."));
    const upgrade = el("button", "", "Upgrade");
    upgrade.addEventListener("click", () => send({ upgrade_group: { node: node.id } }, "Upgrade group"));
    banner.append(" ", upgrade);
    out.push(banner);
  }
  for (const name of info.inputs) {
    const row = el("div", "irow");
    row.append(el("span", "cap", name));
    const names = el("span", "links");
    const links = node.inputs?.[name] ?? [];
    if (!links.length) names.append(el("span", "none", "nothing connected"));
    for (const l of links) {
      const source = doc.nodes.find((n) => n.id === l.node);
      names.append(link(source ? nodeName(source) : l.node, () => selection.select(l.node)));
    }
    row.append(names);
    out.push(row);
  }
  for (const p of exposedParams(doc, info.group)) {
    const param = specParam(specs, p.node, p.param);
    if (!param) continue;
    const overridden = Object.hasOwn(node.instance.overrides ?? {}, p.name);
    const value = overridden ? node.instance.overrides[p.name] : p.node.params[p.param] ?? param.default;
    const row = el("div", overridden ? "irow overridden" : "irow");
    const caption = el("label", "cap", p.name);
    caption.title = `${nodeName(p.node)}.${p.param}: ${param.doc}`;
    const { control, scrub } = widget(param, value, (next) =>
      send({ set_instance_param: { node: node.id, param: p.name, value: next } }, `Set ${p.name}`));
    if (info.outdated) for (const input of control.querySelectorAll?.("input,select,textarea") ?? [control]) input.disabled = true;
    else scrub?.(caption);
    row.append(caption, control);
    if (overridden && !info.outdated) {
      const reset = el("button", "link", "reset");
      reset.title = "Follow the group's definition again";
      reset.addEventListener("click", () => send({ clear_instance_param: { node: node.id, param: p.name } }, `Reset ${p.name}`));
      row.append(reset);
    }
    out.push(row);
    for (const issue of problems.filter((d) => d.param === p.name)) out.push(el("div", "issue", `${issue.code}: ${issue.message}`));
  }
  return out;
}

/// For a node inside a group: a notice that edits change every instance.
export function memberNotice({ doc, node, selection }) {
  const group = doc.groups?.find((g) => g.id === node.group);
  const banner = el("div", "banner", "Part of the group ");
  banner.append(link(group?.name ?? node.group, () => selection.select(node.group)), ": editing it edits every instance.");
  return banner;
}

/// For a parameter of a node inside a group: a control to expose it to instances or stop doing so.
export function exposeControl({ doc, node, param, send }) {
  const group = doc.groups?.find((g) => g.id === node.group);
  if (!group) return null;
  const exposed = group.params.find((p) => p.node === node.id && p.param === param);
  if (exposed) {
    const button = el("button", "link", `exposed as ${exposed.name} ×`);
    button.title = "Stop exposing (changes the group's version)";
    button.addEventListener("click", () => changeInterface(doc, group, { unexpose_param: { group: group.id, name: exposed.name } }, `Unexpose ${exposed.name}`, send));
    return button;
  }
  const taken = new Set(group.params.map((p) => p.name));
  let name = param;
  for (let i = 2; taken.has(name); i++) name = `${param}_${i}`;
  const button = el("button", "link", "expose");
  button.title = "Let instances override this parameter (changes the group's version)";
  button.addEventListener("click", () => changeInterface(doc, group, { expose_param: { group: group.id, name, node: node.id, param } }, `Expose ${name}`, send));
  return button;
}
