// Model-facing helpers for the MCP tools (ADR 0061): the state delta every edit answers with, labels standing in for ids in commands,
// and the scene overview. Pure functions over the evaluation host's data, so they are cheap to call and easy to test.
import type { Command } from './generated';
import type { EvaluationHost } from './evaluation';
import type { MeshPart } from './worker-types';

/** Provisional Quest-class budget the overview and deltas judge against; see `plans/game-world-system.md` section 3. */
export const QUEST_BUDGET = { triangles: 300_000, parts: 400, memory_bytes: 256 * 1024 * 1024 };

export type Totals = { triangles: number; parts: number; memory_bytes: number };

/** How many copies a part draws: an instance set carries one 4 x 4 matrix (16 floats) per copy, anything else is drawn once. */
export const copiesOf = (part: MeshPart) => (part.transforms?.length ? Math.floor(part.transforms.length / 16) : 1);

/** What is on screen now, as plain numbers. Triangles are *drawn* triangles: an instance set counts its mesh once per copy. */
export function totalsOf(evaluation: EvaluationHost): Totals {
  const parts = evaluation.parts;
  return {
    triangles: parts.reduce((sum, part) => sum + (part.indices.length / 3) * copiesOf(part), 0),
    parts: parts.length,
    memory_bytes: evaluation.state.stats?.memory_bytes ?? 0,
  };
}

const grouped = (n: number) => Math.round(n).toLocaleString('en-US');
const metres = (n: number) => (n >= 100 ? n.toFixed(0) : n >= 10 ? n.toFixed(1) : n.toFixed(2));

/** How the numbers compare with the provisional Quest budget: the worst share used and a word for it. */
export function budgetStatus(totals: Totals) {
  const shares = {
    triangles: totals.triangles / QUEST_BUDGET.triangles,
    parts: totals.parts / QUEST_BUDGET.parts,
    memory: totals.memory_bytes / QUEST_BUDGET.memory_bytes,
  };
  const worst = Math.max(shares.triangles, shares.parts, shares.memory);
  const status = worst > 1 ? 'over' : worst > 0.7 ? 'near' : 'within';
  return { profile: 'quest', provisional: true, limits: QUEST_BUDGET, used_share: { triangles: round2(shares.triangles), parts: round2(shares.parts), memory: round2(shares.memory) }, status };
}
const round2 = (n: number) => Math.round(n * 100) / 100;

/** A suggested next step, from the state an edit left behind. */
function nextStep(failure: unknown, after: Totals, budget: ReturnType<typeof budgetStatus>): string {
  if (failure) return 'Fix the failure above (its suggestion says how), or undo to go back.';
  if (after.parts === 0) return 'Nothing is on screen: give a node an output (output name = node) so it becomes a part.';
  if (budget.status === 'over') return 'The scene is over the provisional Quest budget: lower detail, segments or counts before adding more.';
  return 'Look at it with snapshot (views: ["front", "side", "top", "iso"] gives one contact sheet), then measure or check what matters.';
}

/** The compact answer to an edit: what changed on screen and in cost, whether it hurt, and one next step. */
export function stateDelta(before: Totals, after: Totals, info: { revision: number; failure: unknown; warnings: Array<{ code: string; node: string; message: string }>; elapsed_ms?: number }) {
  const budget = budgetStatus(after);
  const change = after.triangles - before.triangles;
  const sign = change >= 0 ? '+' : '-';
  const summary = `${after.parts} part${after.parts === 1 ? '' : 's'}, ${grouped(after.triangles)} triangles (${sign}${grouped(Math.abs(change))}), ${budget.status} the provisional Quest budget${info.warnings.length ? `, ${info.warnings.length} warning${info.warnings.length === 1 ? '' : 's'}` : ''}${info.failure ? ', the edit failed' : ''}`;
  return {
    revision: info.revision,
    triangles: { before: before.triangles, after: after.triangles },
    parts: { before: before.parts, after: after.parts },
    memory_bytes: { before: before.memory_bytes, after: after.memory_bytes },
    diagnostics: { count: info.warnings.length, items: info.warnings.slice(0, 5) },
    ...(info.elapsed_ms === undefined ? {} : { evaluation_ms: info.elapsed_ms }),
    budget: budget.status,
    summary,
    next: nextStep(info.failure, after, budget),
  };
}

type NodeLabel = { id: string; label?: string | null };

/**
 * Lets a command name an existing node by its label instead of its id. A reference that is an id, or an alias of a node the same batch
 * adds, is left alone; a label that names exactly one node becomes its id; a label that names several is reported with their ids so
 * the model can pick, and nothing is applied. Only the `node` of a command, of its `from` and of its `to` are looked at: parameter values
 * are never touched.
 */
export function resolveLabels(commands: Command[], nodes: NodeLabel[]): { commands: Command[] } | { error: string; ambiguous: Record<string, string[]> } {
  const ids = new Set(nodes.map(node => node.id));
  const byLabel = new Map<string, string[]>();
  for (const node of nodes) if (node.label) byLabel.set(node.label, [...(byLabel.get(node.label) ?? []), node.id]);
  const aliases = new Set<string>();
  for (const command of commands) if ('add_node' in command && command.add_node.alias) aliases.add(command.add_node.alias);
  const ambiguous: Record<string, string[]> = {};
  const fix = (ref: unknown): unknown => {
    if (typeof ref !== 'string' || ids.has(ref) || aliases.has(ref)) return ref;
    const hits = byLabel.get(ref);
    if (!hits) return ref;
    if (hits.length === 1) return hits[0];
    ambiguous[ref] = hits;
    return ref;
  };
  const resolved = commands.map(command => {
    const entries = Object.entries(command as Record<string, Record<string, unknown>>);
    if (entries.length !== 1) return command;
    const [name, body] = entries[0];
    if (!body || typeof body !== 'object') return command;
    const next: Record<string, unknown> = { ...body };
    if ('node' in next) next.node = fix(next.node);
    for (const side of ['from', 'to']) {
      const end = next[side];
      if (end && typeof end === 'object' && 'node' in end) next[side] = { ...end, node: fix((end as { node: unknown }).node) };
    }
    return { [name]: next } as unknown as Command;
  });
  const labels = Object.keys(ambiguous);
  if (labels.length > 0) {
    const detail = labels.map(label => `"${label}" names ${ambiguous[label].length} nodes (${ambiguous[label].join(', ')})`).join('; ');
    return { error: `ambiguous label: ${detail}. Use one of the ids, or give the nodes distinct labels with set_label.`, ambiguous };
  }
  return { commands: resolved };
}

/** The axis-aligned box of one part, covering every copy when the part is an instance set. */
export const boundsOfPart = (part: MeshPart) => {
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < part.positions.length; i += 3) for (let axis = 0; axis < 3; axis++) {
    const v = part.positions[i + axis];
    if (v < min[axis]) min[axis] = v;
    if (v > max[axis]) max[axis] = v;
  }
  const matrices = part.transforms;
  if (!matrices?.length || !Number.isFinite(min[0])) return { min, max };
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (let at = 0; at + 15 < matrices.length; at += 16) {
    for (let corner = 0; corner < 8; corner++) {
      const point = [corner & 1 ? max[0] : min[0], corner & 2 ? max[1] : min[1], corner & 4 ? max[2] : min[2]];
      for (let axis = 0; axis < 3; axis++) {
        const v = matrices[at + axis] * point[0] + matrices[at + 4 + axis] * point[1] + matrices[at + 8 + axis] * point[2] + matrices[at + 12 + axis];
        if (v < lo[axis]) lo[axis] = v;
        if (v > hi[axis]) hi[axis] = v;
      }
    }
  }
  return { min: lo, max: hi };
};

/** The `scene_overview` tool: parts by name with their cost and size, the extent of the whole scene, and the budget, in a few lines. */
export function sceneOverview(evaluation: EvaluationHost, params: Record<string, unknown>) {
  const totals = totalsOf(evaluation);
  const listed = evaluation.parts.map(part => {
    const { min, max } = boundsOfPart(part);
    const copies = copiesOf(part), mesh = Math.round(part.indices.length / 3);
    return { name: part.name || part.node, node: part.node, triangles: mesh * copies, ...(copies > 1 ? { copies, mesh_triangles: mesh } : {}), size: max.map((v, i) => round3(v - min[i])), center: max.map((v, i) => round3((v + min[i]) / 2)) };
  }).sort((a, b) => b.triangles - a.triangles);
  const cap = params.detail === 'full' ? listed.length : 12;
  const bounds = evaluation.bounds;
  const size = bounds ? bounds.max.map((v, i) => round3(v - bounds.min[i])) : [0, 0, 0];
  const budget = budgetStatus(totals);
  const warnings = evaluation.state.warnings;
  const summary = totals.parts === 0
    ? 'The scene is empty.'
    : `${totals.parts} part${totals.parts === 1 ? '' : 's'}, ${grouped(totals.triangles)} triangles, ${size.map(metres).join(' x ')} m, ${budget.status} the provisional Quest budget.`;
  return {
    summary,
    totals,
    parts: listed.slice(0, cap),
    ...(listed.length > cap ? { more_parts: listed.length - cap, hint: 'pass detail: "full" to list every part' } : {}),
    bounds: bounds ? { min: bounds.min.map(round3), max: bounds.max.map(round3), size } : { min: [0, 0, 0], max: [0, 0, 0], size },
    evaluation: { elapsed_ms: evaluation.state.stats?.elapsed_ms ?? null, cache_hits: evaluation.state.stats?.cache_hits ?? null, failure: evaluation.state.failureCurrent ? evaluation.state.lastFailure : null },
    warnings: warnings.slice(0, 5),
    budget,
  };
}
const round3 = (n: number) => Math.round(n * 1000) / 1000;

type DocNode = import('./generated').Node;
type DocLike = { nodes: DocNode[]; outputs: Array<{ name: string; from: { node: string } }> };

/**
 * `inspect` (G1): the value inspector and trace. Without `node` it lists every node in run order with what it made. With `node` (a label or id) it answers "why did this give this": the node's
 * operator and parameters, what fed it, what it produced, what it changed from its main input, and the chain of nodes that led there (nearest first, at most `depth`, default 8) with the
 * same facts, so a wrong size or an empty selection can be followed upstream to the node that caused it. Everything is by label.
 */
export function inspectNodes(nodes: import('./evaluator-text').NodeTrace[], params: { node?: unknown; depth?: unknown; limit?: unknown; kind?: unknown }) {
  const nameOf = (n: import('./evaluator-text').NodeTrace) => n.label ?? n.id;
  const brief = (n: import('./evaluator-text').NodeTrace) => ({ node: nameOf(n), operator: n.operator, kind: n.kind, summary: n.summary, ...(n.effect.length ? { changed: n.effect } : {}), ...(n.warnings.length ? { warnings: n.warnings } : {}) });
  const one = typeof params.node === 'string' ? params.node.trim() : '';
  if (!one) {
    const kind = typeof params.kind === 'string' ? params.kind : '';
    const limit = Math.max(1, Math.min(200, Number(params.limit) || 60));
    const shown = nodes.filter(n => !kind || n.kind === kind);
    return { total: nodes.length, shown: Math.min(limit, shown.length), nodes: shown.slice(0, limit).map(n => ({ node: nameOf(n), operator: n.operator, kind: n.kind, summary: n.summary })), ...(shown.length > limit ? { omitted: shown.length - limit } : {}) };
  }
  const target = nodes.find(n => n.id === one || n.label === one);
  if (!target) {
    const near = nodes.filter(n => (n.label ?? '').includes(one) || n.id.includes(one) || one.includes(n.label ?? '\u0000')).slice(0, 5).map(nameOf);
    return { error: { code: 'inspect.unknown_node', message: `no node is called "${one}"`, suggestion: near.length ? `did you mean ${near.join(', ')}?` : 'use inspect without a node to list them' } };
  }
  const depth = Math.max(1, Math.min(40, Number(params.depth) || 8));
  const byName = new Map(nodes.map(n => [nameOf(n), n] as const));
  // Upstream in breadth-first order, nearest first, each node once.
  const chain: typeof nodes = [];
  const seen = new Set<string>([nameOf(target)]);
  let frontier = [target];
  while (frontier.length && chain.length < depth) {
    const next: typeof nodes = [];
    for (const n of frontier) for (const link of n.inputs) {
      const up = byName.get(link.from);
      if (up && !seen.has(nameOf(up)) && chain.length < depth) { seen.add(nameOf(up)); chain.push(up); next.push(up); }
    }
    frontier = next;
  }
  const params2 = Object.entries(target.params).map(([k, v]) => `${k}=${JSON.stringify(v)}`).join(', ');
  const why = `${nameOf(target)} = ${target.operator}(${[...target.inputs.map(i => i.from), params2].filter(Boolean).join(', ')}) made ${target.summary}.`
    + (target.effect.length ? ` It changed: ${target.effect.join('; ')}.` : '')
    + (target.warnings.length ? ` Warnings: ${target.warnings.join('; ')}.` : '');
  return { node: nameOf(target), operator: target.operator, params: target.params, inputs: target.inputs, kind: target.kind, summary: target.summary, facts: target.facts, changed: target.effect, warnings: target.warnings, why, upstream: chain.map(brief) };
}

/**
 * `find` (ADR 0061): nodes by words (in label, operator or parameter names and values), by operator family, or one node by label or id.
 * Answers are short lines plus, for one node, what feeds it and what uses it, all by label so the model can keep talking in names.
 */
export function findNodes(document: DocLike, params: { query?: unknown; family?: unknown; node?: unknown; limit?: unknown }, lines: Record<string, string>) {
  const nameOf = (id: string) => document.nodes.find(n => n.id === id)?.label ?? id;
  const outputOf = (id: string) => document.outputs.find(o => o.from.node === id)?.name ?? null;
  const usedBy = (id: string) => document.nodes.filter(n => Object.values(n.inputs).some(links => links.some(link => link.node === id))).map(n => ({ id: n.id, label: n.label ?? n.id }));
  const describe = (n: DocNode, full: boolean) => {
    const label = n.label ?? n.id;
    const line = lines[n.id];
    const base = { id: n.id, label, op: n.op, line };
    if (!full) return base;
    return {
      ...base,
      params: n.params,
      inputs: Object.entries(n.inputs).flatMap(([port, links]) => links.map(link => ({ port, id: link.node, label: nameOf(link.node) }))),
      used_by: usedBy(n.id),
      output: outputOf(n.id),
    };
  };
  const limit = Math.max(1, Math.min(100, Number(params.limit) || 30));
  const one = typeof params.node === 'string' ? params.node.trim() : '';
  let found = document.nodes.filter(n => !n.group);
  if (one) found = found.filter(n => n.id === one || n.label === one);
  if (typeof params.family === 'string' && params.family) found = found.filter(n => n.op.startsWith(`${params.family}.`));
  const words = typeof params.query === 'string' ? params.query.toLowerCase().split(/\s+/).filter(Boolean) : [];
  if (words.length) {
    found = found.filter(n => {
      const hay = `${n.label ?? ''} ${n.id} ${n.op} ${Object.entries(n.params).map(([k, v]) => `${k} ${JSON.stringify(v)}`).join(' ')}`.toLowerCase();
      return words.every(w => hay.includes(w));
    });
  }
  const matches = found.slice(0, limit).map(n => describe(n, Boolean(one)));
  return {
    matches,
    ...(found.length > limit ? { more: found.length - limit, hint: 'narrow with query, family or node' } : {}),
    ...(found.length === 0 ? { hint: 'nothing matched; try fewer words, a family such as solid or inst, or a label from get_text' } : {}),
  };
}

export type Finding = { id: string; severity: 'error' | 'warning' | 'info'; message: string; fix: string; node?: string };

/**
 * `check` v1 (ADR 0061): problems a model can repair, each with a stable id, the node to change and a fix in words. Covers what the
 * evaluation already knows: whether it failed, the Quest budget, warnings, and the scene parts the game's kinds name.
 */
export function checkScene(document: DocLike, evaluation: EvaluationHost, soundNames: string[] = []) {
  const findings: Finding[] = [];
  if (evaluation.state.failureCurrent && evaluation.state.lastFailure) {
    const failure = evaluation.state.lastFailure;
    findings.push({ id: failure.code, severity: 'error', message: failure.message, fix: 'fix the node named in the message, then check again', ...(failure.node ? { node: failure.node } : {}) });
  }
  const totals = totalsOf(evaluation);
  const budget = budgetStatus(totals);
  if (budget.status === 'over') {
    findings.push({ id: 'budget.quest', severity: 'error', message: `the scene is over the provisional Quest budget (${JSON.stringify(budget.used_share)} of the limits)`, fix: 'reduce triangles (lower segments, use instances instead of copies, simplify distant parts) or split the scene' });
  } else if (budget.status === 'near') {
    findings.push({ id: 'budget.quest_near', severity: 'warning', message: 'the scene uses more than 70% of the provisional Quest budget', fix: 'leave room for the game and effects; simplify before adding more' });
  }
  const labelOf = (id: string) => document.nodes.find(n => n.id === id)?.label ?? id;
  for (const warning of evaluation.state.warnings.slice(0, 10)) {
    const bake = warning.code === 'texture.bake_failed';
    findings.push({
      id: warning.code, severity: bake ? 'error' : 'warning', message: warning.message, node: labelOf(warning.node),
      fix: bake ? 'correct the texture program on that node (texture_nodes lists the node names and parameters; a program must end with albedo = <node>), then check again' : 'see the operator documentation (list_operators ids=[...]) for what this warning means',
    });
  }
  const partNames = new Set(evaluation.parts.map(part => part.name));
  for (const n of document.nodes) {
    if (n.op.startsWith('game.kind') && typeof n.params.part === 'string' && n.params.part && !partNames.has(n.params.part)) {
      const have = [...partNames].slice(0, 8);
      findings.push({
        id: 'game.part_missing', severity: 'error', node: n.label ?? n.id,
        message: `kind ${JSON.stringify(n.params.name)} is drawn by part "${n.params.part}", but no scene output has that name`,
        fix: have.length ? `set part to one of the scene outputs (${have.join(', ')}) or add \`output ${n.params.part} = <node>\`` : `add an output named ${n.params.part} (output ${n.params.part} = <node>) that draws it`,
      });
    }
  }
  if (soundNames.length) {
    for (const n of document.nodes) {
      if (!n.op.startsWith('game.rule') || typeof n.params.do !== 'string') continue;
      for (const [, name] of n.params.do.matchAll(/effect:sfx:([A-Za-z0-9_]+)/g)) {
        if (soundNames.includes(name.toLowerCase())) continue;
        findings.push({
          id: 'game.sfx_unknown', severity: 'error', node: n.label ?? n.id,
          message: `the rule plays a sound called "${name}", which does not exist, so it would be silent`,
          fix: `use one of the sounds: ${soundNames.join(', ')}`,
        });
      }
    }
  }
  const errors = findings.filter(f => f.severity === 'error').length;
  const warnings = findings.filter(f => f.severity === 'warning').length;
  return {
    ok: errors === 0,
    summary: findings.length === 0 ? 'no problems found' : `${errors} error${errors === 1 ? '' : 's'}, ${warnings} warning${warnings === 1 ? '' : 's'}`,
    findings,
  };
}
