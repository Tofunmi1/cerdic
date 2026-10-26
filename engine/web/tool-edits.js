// Page-side tools compile to the same atomic command/evaluation path as apply_text.
import { validateToolArguments } from './tool-arguments.js';

function uniqueNode(document, label) {
  const hits = document.nodes.filter(node => node.label === label);
  if (hits.length === 1) return hits[0];
  const candidates = (hits.length ? hits : document.nodes).map(node => `${node.label ?? '(unlabelled)'} (${node.id})`);
  throw new Error(`${hits.length ? 'Ambiguous' : 'Unknown'} label ${JSON.stringify(label)}; candidates: ${candidates.join(', ') || '(none)'}`);
}

export async function editDocument(params, services) {
  const invalid = validateToolArguments('edit', params);
  if (invalid) throw new Error(invalid);
  const original = services.doc.state().document;
  const originalId = original.id, originalRevision = original.revision;
  const document = structuredClone(original);
  const commands = [];
  // Working snapshots retain valid engine IDs for parsing; commands use aliases after replacement.
  const replacements = new Map();
  const ref = id => replacements.get(id) ?? id;
  for (const [index, op] of params.ops.entries()) {
    const node = uniqueNode(document, op.node);
    const target = ref(node.id);
    if (op.op === 'set') {
      for (const [param, value] of Object.entries(op.params)) {
        commands.push({ [node.instance ? 'set_instance_param' : 'set_param']: { node: target, param, value } });
        (node.instance ? node.instance.overrides : node.params)[param] = value;
      }
    } else if (op.op === 'rename') {
      const taken = document.nodes.filter(other => other.id !== node.id && other.label === op.to);
      if (taken.length) throw new Error(`Label ${JSON.stringify(op.to)} is already taken; candidates: ${taken.map(n => n.id).join(', ')}`);
      commands.push({ set_label: { node: target, label: op.to } });
      node.label = op.to;
    } else if (op.op === 'delete') {
      // Removing a visible leaf also removes its named outputs, without deleting its users.
      for (const output of document.outputs.filter(output => output.from.node === node.id)) commands.push({ set_output: { name: output.name, from: null } });
      document.outputs = document.outputs.filter(output => output.from.node !== node.id);
      commands.push({ remove_node: { node: target, cascade: false } });
      document.nodes = document.nodes.filter(other => other.id !== node.id);
    } else {
      const base = structuredClone(document);
      base.nodes = base.nodes.filter(other => other.id !== node.id);
      if (node.group) base.nodes = base.nodes.filter(other => other.group === node.group).map(other => ({ ...other, group: null }));
      base.outputs = base.outputs.filter(output => output.from.node !== node.id);
      const read = await services.parse(base, op.text);
      if (!read.commands) return { applied: false, diagnostics: read.diagnostics ?? (read.error ? [read.error] : []) };
      const adds = read.commands.filter(command => command.add_node || command.add_instance);
      const added = adds[0]?.add_node ?? adds[0]?.add_instance;
      if (adds.length !== 1 || added.label !== node.label || read.commands.some(command => !command.add_node && !command.add_instance && !command.connect && !command.set_flags && !command.set_instance_param)) {
        throw new Error(`replace needs one node statement named ${JSON.stringify(node.label)}; use apply_text for a larger program`);
      }
      const alias = `$edit_replace_${index}`;
      const oldAlias = added.alias;
      const newInputs = {};
      let flags = { ...node.flags };
      const overrides = {};
      for (const command of read.commands) {
        if (command.add_node || command.add_instance) {
          commands.push(command.add_node ? { add_node: { ...added, alias, group: node.group ?? null } } : { add_instance: { ...added, alias, into: node.group ?? null } });
          commands.push({ set_flags: { node: alias, ...flags } });
        } else if (command.connect) {
          const { from, to } = command.connect;
          if (to.node !== oldAlias) throw new Error('replace may only connect inputs of the replacement node');
          commands.push({ connect: { from: { ...from, node: ref(from.node) }, to: { ...to, node: alias } } });
          const links = newInputs[to.port] ??= [];
          links.splice(to.index ?? links.length, 0, from);
        } else if (command.set_flags) {
          if (command.set_flags.node !== oldAlias) throw new Error('replace may only set flags of the replacement node');
          for (const [key, value] of Object.entries(command.set_flags)) if (key !== 'node' && value !== null) flags[key] = value;
          commands.push({ set_flags: { node: alias, ...flags } });
        } else {
          const param = command.set_instance_param;
          if (param.node !== oldAlias) throw new Error('replace may only set parameters of the replacement instance');
          commands.push({ set_instance_param: { ...param, node: alias } });
          overrides[param.param] = param.value;
        }
      }
      for (const user of document.nodes) for (const [port, links] of Object.entries(user.inputs)) {
        for (const [i, from] of links.entries()) if (from.node === node.id) {
          const to = { node: ref(user.id), port, index: i };
          // Disconnect before connect: a variadic input inserts rather than replaces.
          commands.push({ disconnect: { to } }, { connect: { from: { ...from, node: alias }, to } });
        }
      }
      for (const output of document.outputs.filter(output => output.from.node === node.id)) commands.push({ set_output: { name: output.name, from: { ...output.from, node: alias } } });
      for (const group of document.groups ?? []) {
        if (group.output === node.id) commands.push({ set_group_output: { group: group.id, node: alias } });
        for (const param of group.params) if (param.node === node.id) commands.push(
          { unexpose_param: { group: group.id, name: param.name } },
          { expose_param: { group: group.id, name: param.name, node: alias, param: param.param } },
        );
      }
      commands.push({ remove_node: { node: target, cascade: false } });
      replacements.set(node.id, alias);
      node.op = added.op ?? 'group.use@1';
      node.params = added.params ?? {};
      node.inputs = newInputs;
      node.flags = flags;
      if (adds[0].add_instance) node.instance = { group: added.group, version: document.groups.find(group => group.id === added.group).version, overrides };
      else delete node.instance;
    }
  }
  const latest = services.doc.state().document;
  if (latest.id !== originalId || latest.revision !== originalRevision) throw new Error('The document changed while preparing edit; retry against its current revision');
  return services.mutate(commands, params.label ?? 'assistant: edit', params.dry_run === true);
}

export async function diffDocument(params, services) {
  const invalid = validateToolArguments('diff', params);
  if (invalid) throw new Error(invalid);
  const current = services.doc.state().document;
  const since = params.since ?? Math.max(0, current.revision - 1);
  const previous = services.doc.atRevision(since);
  if (previous.error) throw new Error(previous.error);
  const after = services.doc.atRevision(current.revision);
  if (after.error) throw new Error(after.error);
  const lines = [];
  const oldLines = previous.lines, newLines = after.lines;
  for (const [id, line] of Object.entries(oldLines)) {
    if (!(id in newLines)) lines.push(`- ${line}`);
    else if (newLines[id] !== line) lines.push(`~ ${newLines[id]}`);
  }
  for (const [id, line] of Object.entries(newLines)) if (!(id in oldLines)) lines.push(`+ ${line}`);
  const remaining = Math.max(0, lines.length - 80);
  return { since, revision: current.revision, text: [...lines.slice(0, 80), ...(remaining ? [`... ${remaining} more node lines`] : [])].join('\n'), remaining };
}
