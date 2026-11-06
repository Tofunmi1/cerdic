import { memo, useEffect, useMemo, useState } from 'react';
import { Background, BackgroundVariant, Controls, Handle, MiniMap, Position, ReactFlow, ReactFlowProvider } from '@xyflow/react';
import type { Edge, Node, NodeProps } from '@xyflow/react';
import '@xyflow/react/dist/base.css';
import { LoaderCircle } from 'lucide-react';
import type { Document } from '../engine/generated';
import { families } from './tokens';
import { familyOf } from './hooks';

type Data = {
  title: string; op: string; family: string; rows: { k: string; v: string }[]; ports: string[]; output: boolean;
  dim: boolean; selected: boolean;
};
type OpNode = Node<Data, 'op'>;

const NODE_WIDTH = 220;
const colourOf = (family: string) => (families as Record<string, string>)[family] ?? families.other;

function short(value: unknown): string {
  if (typeof value === 'number') return Number.isInteger(value) ? String(value) : String(Math.round(value * 1000) / 1000);
  if (typeof value === 'string') return value.length > 18 ? `${value.slice(0, 16)}…` : value;
  if (typeof value === 'boolean') return value ? 'on' : 'off';
  if (Array.isArray(value)) return value.length <= 4 && value.every(v => typeof v === 'number') ? `[${value.map(short).join(', ')}]` : `${value.length} items`;
  if (value && typeof value === 'object') {
    const entries = Object.values(value as Record<string, unknown>);
    return entries.length === 1 ? short(entries[0]) : '{…}';
  }
  return '';
}

/** One node of the document: coloured header by family, a socket for every input, and its first few parameters. */
const OpNodeView = memo(function OpNodeView({ data }: NodeProps<OpNode>) {
  const colour = colourOf(data.family);
  return (
    <div className="gnode" data-selected={data.selected} data-output={data.output}
      style={{ ['--fam' as string]: colour, opacity: data.dim ? 0.3 : 1 }}>
      <div className="ghead"><span className="gtitle">{data.title}</span></div>
      <div className="gop">{data.op}</div>
      {data.ports.map(port => (
        <div className="grow-line" key={port}>
          <Handle type="target" position={Position.Left} id={port} />
          <span className="k">{port}</span>
        </div>
      ))}
      {data.rows.map(row => (
        <div className="grow-line" key={row.k}><span className="k">{row.k}</span><span className="v">{row.v}</span></div>
      ))}
      <div className="gpad" />
      <Handle type="source" position={Position.Right} id="out" style={{ top: 18 }} />
    </div>
  );
});
const nodeTypes = { op: OpNodeView };

type Laid = { nodes: OpNode[]; edges: Edge[]; ancestors: Map<string, Set<string>> };

/** Lays the document out left to right (inputs on the left, results on the right) with ELK's layered algorithm. */
async function layout(doc: Document): Promise<Laid> {
  const nodes = doc.nodes.filter(node => !node.group);
  const ids = new Set(nodes.map(node => node.id));
  const outputs = new Set((doc.outputs ?? []).map(output => output.from.node));
  const edges: Edge[] = [];
  const parents = new Map<string, string[]>();
  const items = nodes.map(node => {
    const ports = Object.keys(node.inputs ?? {}).filter(port => (node.inputs[port] ?? []).length > 0);
    const rows = Object.entries(node.params ?? {}).slice(0, 4).map(([k, v]) => ({ k, v: short(v) }));
    for (const port of ports) for (const ref of node.inputs[port]) {
      if (!ids.has(ref.node)) continue;
      edges.push({ id: `${ref.node}->${node.id}.${port}`, source: ref.node, sourceHandle: 'out', target: node.id, targetHandle: port });
      parents.set(node.id, [...(parents.get(node.id) ?? []), ref.node]);
    }
    return { node, ports, rows, height: 28 + 20 + 20 * (ports.length + rows.length) + 6 };
  });
  const { default: ELK } = await import('elkjs/lib/elk.bundled.js');
  const elk = new ELK();
  const laid = await elk.layout({
    id: 'root',
    layoutOptions: {
      'elk.algorithm': 'layered', 'elk.direction': 'RIGHT',
      'elk.layered.spacing.nodeNodeBetweenLayers': '70', 'elk.spacing.nodeNode': '24',
      'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX', 'elk.layered.crossingMinimization.strategy': 'LAYER_SWEEP',
    },
    children: items.map(item => ({ id: item.node.id, width: NODE_WIDTH, height: item.height })),
    edges: edges.map(edge => ({ id: edge.id, sources: [edge.source], targets: [edge.target] })),
  });
  const at = new Map((laid.children ?? []).map(child => [child.id, child]));
  const flowNodes: OpNode[] = items.map(item => ({
    id: item.node.id, type: 'op', position: { x: at.get(item.node.id)?.x ?? 0, y: at.get(item.node.id)?.y ?? 0 },
    draggable: true,
    data: {
      title: item.node.label || item.node.op.split('@')[0].split('.').slice(1).join('.') || item.node.op,
      op: item.node.op.split('@')[0], family: familyOf(item.node.op), rows: item.rows, ports: item.ports,
      output: outputs.has(item.node.id), dim: false, selected: false,
    },
  }));
  // Everything upstream of a node, for highlighting the chain that feeds it.
  const ancestors = new Map<string, Set<string>>();
  const upstream = (id: string): Set<string> => {
    const known = ancestors.get(id);
    if (known) return known;
    const set = new Set<string>([id]);
    ancestors.set(id, set);
    for (const parent of parents.get(id) ?? []) for (const a of upstream(parent)) set.add(a);
    return set;
  };
  for (const node of nodes) if (outputs.has(node.id) || nodes.length < 300) upstream(node.id);
  return { nodes: flowNodes, edges, ancestors };
}

function Inner({ doc, revision }: { doc: Document; revision: number }) {
  const [laid, setLaid] = useState<Laid | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(true);
  const [pick, setPick] = useState<string | null>(null);
  const [query, setQuery] = useState('');

  useEffect(() => {
    let live = true;
    setBusy(true);
    layout(doc).then(result => { if (live) { setLaid(result); setError(''); } })
      .catch(e => { if (live) setError(String(e)); })
      .finally(() => { if (live) setBusy(false); });
    return () => { live = false; };
    // The layout is redone when a new revision arrives, not on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [revision, doc.nodes.length]);

  const needle = query.trim().toLowerCase();
  const nodes = useMemo(() => {
    if (!laid) return [];
    const chain = pick ? laid.ancestors.get(pick) : null;
    return laid.nodes.map(node => {
      const matches = !needle || node.data.title.toLowerCase().includes(needle) || node.data.op.includes(needle);
      const inChain = !chain || chain.has(node.id);
      return { ...node, data: { ...node.data, dim: !(matches && inChain), selected: node.id === pick } };
    });
  }, [laid, pick, needle]);
  const edges = useMemo(() => {
    if (!laid) return [];
    const chain = pick ? laid.ancestors.get(pick) : null;
    const many = laid.edges.length > 250;
    return laid.edges.map(edge => ({
      ...edge, type: many ? 'straight' : 'default',
      animated: !!chain && chain.has(edge.source) && chain.has(edge.target),
      style: { opacity: chain && !(chain.has(edge.source) && chain.has(edge.target)) ? 0.12 : 1 },
    }));
  }, [laid, pick]);

  return (
    <div className="graph">
      <div className="graph-bar">
        <h2>Node graph</h2>
        <span className="chip">{laid ? `${laid.nodes.length} nodes` : '…'}</span>
        <div className="grow" />
        <input type="search" placeholder="Find a node" value={query} onChange={e => setQuery(e.target.value)} aria-label="Find a node" />
      </div>
      <div className="graph-canvas">
        {error ? <div className="graph-msg">{error}</div>
          : !laid && busy ? <div className="graph-msg"><LoaderCircle className="spin" /></div>
            : laid && laid.nodes.length === 0 ? <div className="graph-msg">The document has no nodes yet.</div>
              : (
                <ReactFlow<OpNode> nodes={nodes} edges={edges} nodeTypes={nodeTypes} fitView minZoom={0.05} maxZoom={1.6}
                  onlyRenderVisibleElements nodesConnectable={false} proOptions={{ hideAttribution: true }}
                  onNodeClick={(_, node) => setPick(previous => previous === node.id ? null : node.id)} onPaneClick={() => setPick(null)}>
                  <Background variant={BackgroundVariant.Dots} gap={22} size={1.2} />
                  <Controls showInteractive={false} />
                  {laid && laid.nodes.length < 600 && (
                    <MiniMap pannable zoomable nodeColor={node => colourOf((node.data as Data).family)} maskColor="rgba(0,0,0,0.25)" />
                  )}
                </ReactFlow>
              )}
        {busy && laid && <div className="graph-msg" style={{ pointerEvents: 'none', alignItems: 'start', justifyItems: 'end', padding: 12 }}><LoaderCircle className="spin" size={16} /></div>}
      </div>
    </div>
  );
}

export function GraphView({ doc, revision }: { doc: Document; revision: number }) {
  return <ReactFlowProvider><Inner doc={doc} revision={revision} /></ReactFlowProvider>;
}
