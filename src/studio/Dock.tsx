import { useEffect, useMemo, useState } from 'react';
import { ChevronDown, ChevronRight, CircleAlert, Trash2 } from 'lucide-react';
import type { Diagnostic, Document, Node } from '../engine/generated';
import { host } from '../engine/host';
import { diagnosticsByNode, fromDegrees, nodeName, outlineRows, show, toDegrees } from '../../engine/web/panels/graph.js';
import { operators } from './engine-api';
import type { ParamSpec } from './engine-api';

type Tab = 'outline' | 'inspect' | 'history' | 'problems';
type Problem = { code: string; message: string; node?: string; related?: string[]; param?: string };

/** What the engine has reported: document issues, the failure of the current evaluation, and evaluation warnings. */
export function useProblems(): Problem[] {
  const state = host.doc?.state();
  const e = host.evaluation?.state;
  const list: Problem[] = [...((state?.issues ?? []) as Diagnostic[]).map(d => d as unknown as Problem)];
  if (e?.lastFailure && e.failureCurrent) list.push(e.lastFailure);
  for (const w of e?.warnings ?? []) list.push(w);
  return list;
}

const UNITS: Record<string, { suffix: string; to: (v: number) => number; from: (v: number) => number }> = {
  length: { suffix: 'm', to: v => v, from: v => v },
  angle: { suffix: '°', to: toDegrees, from: fromDegrees },
  ratio: { suffix: '', to: v => v, from: v => v },
  none: { suffix: '', to: v => v, from: v => v },
};

/** A number in display units (metres, degrees); one edit is one command. */
function NumField({ value, unit = 'none', whole, onCommit }: { value: number; unit?: string; whole: boolean; onCommit: (v: number) => void }) {
  const u = UNITS[unit] ?? UNITS.none;
  const shown = show(u.to(value));
  const [text, setText] = useState(shown);
  useEffect(() => setText(shown), [shown]);
  const commit = () => {
    const typed = Number(text);
    if (text.trim() === '' || !Number.isFinite(typed)) { setText(shown); return; }
    const next = whole ? Math.round(typed) : u.from(typed);
    if (next !== value) onCommit(next);
  };
  return (
    <span className="field">
      <input className="num" type="number" step={whole ? 1 : 'any'} value={text} onChange={e => setText(e.target.value)} onBlur={commit}
        onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); }} />
      {u.suffix && <span className="unit">{u.suffix}</span>}
    </span>
  );
}

function ParamControl({ spec, value, onCommit }: { spec: ParamSpec; value: any; onCommit: (v: unknown) => void }) {
  const { ty } = spec;
  switch (ty.type) {
    case 'num': case 'int': return <NumField value={Number(value)} unit={ty.unit} whole={ty.type === 'int'} onCommit={onCommit} />;
    case 'bool': return <input className="switch" type="checkbox" role="switch" checked={!!value} onChange={e => onCommit(e.target.checked)} />;
    case 'choice': return <select value={String(value)} onChange={e => onCommit(e.target.value)}>{(ty.options ?? []).map(o => <option key={o} value={o}>{o}</option>)}</select>;
    case 'str': return <TextField value={String(value ?? '')} onCommit={onCommit} />;
    case 'vec2': case 'vec3': {
      const count = ty.type === 'vec2' ? 2 : 3;
      const parts = Array.from({ length: count }, (_, i) => Number((value as number[])?.[i] ?? 0));
      return <span className="vec">{parts.map((p, i) => <NumField key={i} value={p} unit={ty.unit} whole={false}
        onCommit={v => onCommit(parts.map((q, k) => (k === i ? v : q)))} />)}</span>;
    }
    default: return <JsonField value={value} onCommit={onCommit} />;
  }
}

function TextField({ value, onCommit }: { value: string; onCommit: (v: string) => void }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  return <input className="txt" type="text" value={text} onChange={e => setText(e.target.value)} onBlur={() => text !== value && onCommit(text)}
    onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); }} />;
}

function JsonField({ value, onCommit }: { value: unknown; onCommit: (v: unknown) => void }) {
  const shown = JSON.stringify(value);
  const [text, setText] = useState(shown);
  const [bad, setBad] = useState(false);
  useEffect(() => { setText(shown); setBad(false); }, [shown]);
  return <textarea className={`json${bad ? ' bad' : ''}`} rows={3} spellCheck={false} value={text} onChange={e => setText(e.target.value)}
    onBlur={() => { try { setBad(false); if (text !== shown) onCommit(JSON.parse(text)); } catch { setBad(true); } }} />;
}

function Outline({ doc, selected, onSelect, problems }: { doc: Document; selected: string | null; onSelect: (id: string | null) => void; problems: Problem[] }) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const bad = useMemo(() => diagnosticsByNode(problems) as Map<string, Problem[]>, [problems]);
  const rows = useMemo(() => outlineRows(doc, collapsed) as any[], [doc, collapsed]);
  const toggle = (id: string) => setCollapsed(previous => { const next = new Set(previous); if (next.has(id)) next.delete(id); else next.add(id); return next; });
  if (!rows.length) return <div className="empty">Nothing here yet. Ask the assistant to build something.</div>;
  return (
    <div className="outline" role="tree">
      {rows.map((row, i) => {
        if (row.header) return <div key={`h${i}`} className="ohead">{row.header}</div>;
        const id: string = row.group ?? row.id;
        const expandable = row.group ? true : row.children;
        return (
          <div key={`${id}-${i}`} className={`orow${selected === id ? ' selected' : ''}${row.muted ? ' muted' : ''}${row.unused ? ' unused' : ''}`}
            style={{ paddingLeft: 6 + (row.depth ?? 0) * 14 }} role="treeitem" tabIndex={0} aria-selected={selected === id}
            onClick={() => onSelect(selected === id ? null : id)}
            onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSelect(selected === id ? null : id); } }}>
            <span className="caret" onClick={e => { if (expandable) { e.stopPropagation(); toggle(id); } }}>
              {expandable ? (row.collapsed ? <ChevronRight size={13} /> : <ChevronDown size={13} />) : null}
            </span>
            <span className="oname">{row.reference ? `↗ ${row.name}` : row.name}</span>
            {row.group ? <><span className="badge">v{row.version}</span><span className="oop">{row.instances} use{row.instances === 1 ? '' : 's'}</span></> : <>
              {row.output && <span className="badge part">{row.output}</span>}
              {row.muted && <span className="badge">muted</span>}
              {row.bypass && <span className="badge">bypass</span>}
              {row.outdated && <span className="badge warn">outdated</span>}
              <span className="oop">{row.instanceOf ?? row.op}</span></>}
            {bad.has(id) && <span className="odot" title={bad.get(id)!.map(d => `${d.code}: ${d.message}`).join('\n')}>●</span>}
          </div>
        );
      })}
    </div>
  );
}

function Inspector({ doc, selected, onSelect, problems }: { doc: Document; selected: string | null; onSelect: (id: string | null) => void; problems: Problem[] }) {
  const [rejected, setRejected] = useState<Problem[]>([]);
  useEffect(() => setRejected([]), [selected]);
  const node = doc.nodes.find(n => n.id === selected) as Node | undefined;
  const group = doc.groups?.find(g => g.id === selected);
  if (!selected) return <div className="empty">Select a node in the outline, or click a part in the view.</div>;
  if (group) return <div className="empty">{group.name} is a group (version {group.version}). Group editing is not in the studio yet.</div>;
  if (!node) return <div className="empty">The selected node no longer exists.</div>;
  const spec = operators().get(node.op);
  const mine = [...((diagnosticsByNode(problems) as Map<string, Problem[]>).get(node.id) ?? []), ...rejected];
  const send = (command: unknown, label: string) => {
    const outcome = host.doc?.send([command as never], label) as { rejected?: { diagnostics: Problem[] } } | undefined;
    setRejected(outcome?.rejected?.diagnostics ?? []);
  };
  return (
    <div className="inspect">
      <TextField value={node.label ?? ''} onCommit={v => send({ set_label: { node: node.id, label: v.trim() || null } }, 'Rename')} />
      <div className="ophelp">{node.instance ? 'instance' : node.op}{spec && !node.instance ? ` · ${spec.doc}` : ''} · {node.id}</div>
      {node.instance && <div className="ophelp">This is an instance of a group; its overrides are not editable in the studio yet.</div>}
      {!node.instance && spec?.inputs.map(input => {
        const links = node.inputs[input.name] ?? [];
        return (
          <div className="irow" key={input.name}>
            <span className="cap">{input.name}</span>
            <span className="links">
              {links.length === 0 && <span className="none">nothing connected</span>}
              {links.map(link => {
                const source = doc.nodes.find(c => c.id === link.node);
                return <button key={link.node} className="link" type="button" onClick={() => onSelect(link.node)}>{source ? nodeName(source) : link.node}</button>;
              })}
            </span>
          </div>
        );
      })}
      {!node.instance && spec?.params.map(param => (
        <div key={param.name}>
          <div className="irow">
            <label className="cap" title={param.doc}>{param.name}</label>
            <ParamControl spec={param} value={node.params[param.name] ?? param.default}
              onCommit={value => send({ set_param: { node: node.id, param: param.name, value } }, `Set ${param.name}`)} />
          </div>
          {mine.filter(d => d.param === param.name).map((d, i) => <div key={i} className="issue">{d.code}: {d.message}</div>)}
        </div>
      ))}
      <div className="irow flags">
        {(['muted', 'bypass'] as const).map(flag => (
          <label key={flag} className="flag"><input type="checkbox" checked={!!(node.flags as Record<string, boolean> | undefined)?.[flag]}
            onChange={e => send({ set_flags: { node: node.id, [flag]: e.target.checked } }, flag === 'muted' ? 'Mute' : 'Bypass')} /> {flag}</label>
        ))}
        <span className="grow" />
        <button className="btn danger" type="button" onClick={() => send({ remove_node: { node: node.id, cascade: false } }, 'Delete node')}><Trash2 size={13} />Delete</button>
      </div>
      {mine.filter(d => !d.param).map((d, i) => <div key={i} className="issue">{d.code}: {d.message}</div>)}
    </div>
  );
}

function History() {
  const state = host.doc?.state() as { history?: { undo: { label: string; changed: string[] }[]; redo: { label: string; changed: string[] }[] }; read_only?: boolean } | undefined;
  const { undo = [], redo = [] } = state?.history ?? {};
  const step = (times: number, move: () => unknown) => { if (state?.read_only) return; for (let i = 0; i < times; i++) move(); };
  if (!undo.length && !redo.length) return <div className="empty">No edits yet.</div>;
  return (
    <div className="history">
      {undo.map((entry, i) => (
        <button key={`u${i}`} type="button" className="hrow done" title={`${entry.changed.length} node(s) changed. Go back to just after this edit.`}
          onClick={() => step(undo.length - 1 - i, () => host.doc?.undo())}>{entry.label}</button>
      ))}
      <div className="hnow">now</div>
      {redo.map((entry, i) => (
        <button key={`r${i}`} type="button" className="hrow undone" title="Undone. Redo up to and including this edit." onClick={() => step(i + 1, () => host.doc?.redo())}>{entry.label}</button>
      ))}
    </div>
  );
}

function Problems({ problems, onSelect }: { problems: Problem[]; onSelect: (id: string) => void }) {
  if (!problems.length) return <div className="empty">No problems.</div>;
  return (
    <div className="problems">
      {problems.map((p, i) => {
        const id = p.node ?? p.related?.[0];
        return (
          <button key={i} type="button" className="prow" disabled={!id} onClick={() => id && onSelect(id)} title={id ? `Select ${id}` : undefined}>
            <CircleAlert size={14} /><span><b>{p.code}</b> {p.message}</span>
          </button>
        );
      })}
    </div>
  );
}

/** The right-hand dock: the outline of the document, the inspector for the selected node, the edit history and the problems list. */
export function Dock({ doc, selected, onSelect, problems }: { doc: Document | undefined; selected: string | null; onSelect: (id: string | null) => void; problems: Problem[] }) {
  const [tab, setTab] = useState<Tab>('outline');
  // Choosing a node (in the outline or by clicking a part) brings its parameters up.
  useEffect(() => { if (selected) setTab(t => (t === 'outline' ? 'inspect' : t)); }, [selected]);
  const tabs: [Tab, string][] = [['outline', 'Outline'], ['inspect', 'Inspector'], ['history', 'History'], ['problems', problems.length ? `Problems ${problems.length}` : 'Problems']];
  return (
    <aside className="dock glass" aria-label="Document">
      <div className="segmented wide dock-tabs" role="tablist">
        {tabs.map(([id, name]) => <button key={id} type="button" role="tab" aria-selected={tab === id} aria-pressed={tab === id} onClick={() => setTab(id)}>{name}</button>)}
      </div>
      <div className="dock-body">
        {!doc ? <div className="empty">Starting engine…</div>
          : tab === 'outline' ? <Outline doc={doc} selected={selected} onSelect={onSelect} problems={problems} />
            : tab === 'inspect' ? <Inspector doc={doc} selected={selected} onSelect={onSelect} problems={problems} />
              : tab === 'history' ? <History />
                : <Problems problems={problems} onSelect={id => { onSelect(id); setTab('inspect'); }} />}
      </div>
    </aside>
  );
}
