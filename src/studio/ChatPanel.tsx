import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { ArrowUp, ChevronDown, ChevronRight, CircleAlert, Check, LoaderCircle, Plug, Plus, Square, Trash2, Wrench } from 'lucide-react';
import type { Conversation, Entry, ToolCall } from './transcript';
import { host } from '../engine/host';
import { AssistantConnection } from './AssistantConnection';
import type { SidebarAgent } from './sidebarAgent';

/** Paragraphs, ``` fences and `inline code`: all the assistant's text needs. */
function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  const parts = text.split(/```(?:[a-z0-9_-]*)\n?/i);
  parts.forEach((part, index) => {
    if (index % 2 === 1) { blocks.push(<pre key={index}><code>{part.replace(/\n$/, '')}</code></pre>); return; }
    part.split(/\n{2,}/).filter(Boolean).forEach((paragraph, p) => {
      const pieces = paragraph.split(/(`[^`]+`|\*\*[^*]+\*\*)/g).map((piece, k) =>
        piece.startsWith('`') ? <code key={k}>{piece.slice(1, -1)}</code>
          : piece.startsWith('**') ? <strong key={k}>{piece.slice(2, -2)}</strong> : piece);
      blocks.push(<p key={`${index}-${p}`}>{pieces}</p>);
    });
  });
  return <div className="body">{blocks}</div>;
}

const pretty = (value: unknown) => typeof value === 'string' ? value : JSON.stringify(value, null, 2);

function ToolCard({ tool }: { tool: ToolCall }) {
  const [open, setOpen] = useState(false);
  const status = tool.status === 'running'
    ? <span className="chip live"><LoaderCircle size={12} className="spin" />running</span>
    : tool.status === 'error'
      ? <span className="chip bad"><CircleAlert size={12} />failed</span>
      : <span className="chip ok"><Check size={12} />done</span>;
  const hasDetail = tool.input !== undefined || tool.output !== undefined;
  return (
    <div className="tool" data-open={open}>
      <button type="button" onClick={() => hasDetail && setOpen(!open)} aria-expanded={open}>
        <Wrench size={14} />
        <span className="name">{tool.name}</span>
        <span className="summary">{tool.summary}</span>
        {status}
        {hasDetail && <ChevronRight size={14} className="caret" />}
      </button>
      {open && (
        <div className="detail">
          {tool.input !== undefined && <><h4>Input</h4><pre>{pretty(tool.input)}</pre></>}
          {tool.output !== undefined && <><h4>Result</h4><pre>{pretty(tool.output)}</pre></>}
        </div>
      )}
    </div>
  );
}

const STAGES = ['Queued', 'Evaluating', 'Uploading', 'Refining'] as const;

/** Where the engine is in building the current document, from the evaluation host's own state. */
export function buildStage(): { index: number; label: string; fraction?: number } | null {
  const e = host.evaluation?.state;
  if (!e) return null;
  if (e.worker === 'evaluating' || e.worker === 'overdue') {
    const { evaluated, total } = e.progress;
    const fraction = total > 0 ? Math.min(1, evaluated / total) : undefined;
    const base = e.worker === 'overdue' ? 'Still evaluating' : 'Evaluating the model';
    return { index: 1, fraction, label: total > 0 ? `${base} · ${evaluated} of ${total} nodes` : `${base}…` };
  }
  if (e.worker === 'uploading') return { index: 2, label: 'Sending geometry to the viewer' };
  if (e.queue > 0) return { index: 0, label: 'Waiting for the evaluator' };
  if (e.refining) return { index: 3, label: 'Refining the detail' };
  return null;
}

export function BuildCard() {
  const stage = buildStage();
  if (!stage) return null;
  return (
    <div className="build" role="status" aria-live="polite">
      <header><LoaderCircle size={14} className="spin" />{stage.label}</header>
      <div className="steps">
        {STAGES.map((name, i) => (
          <div key={name} className="step" data-state={i < stage.index ? 'done' : i === stage.index ? (stage.fraction === undefined ? 'active' : 'measured') : 'todo'}
            style={i === stage.index && stage.fraction !== undefined ? { ['--fill' as string]: `${Math.round(stage.fraction * 100)}%` } : undefined} />
        ))}
      </div>
      <div className="labels">
        {STAGES.map((name, i) => <span key={name} data-state={i === stage.index ? 'active' : ''}>{name}</span>)}
      </div>
    </div>
  );
}

type ChatProps = { entries: Entry[]; onSend: (text: string) => void; onClear: () => void; projectName?: string; disabled: boolean;
  conversations: Conversation[]; conversationId: string; onNewChat: () => void; onSelectChat: (id: string) => void; error?: string; agent: SidebarAgent };
export function ChatPanel({ entries, onSend, onClear, projectName, disabled, conversations, conversationId, onNewChat, onSelectChat, error, agent }: ChatProps) {
  const scroll = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState('');
  const [connectionOpen, setConnectionOpen] = useState(false);
  const stick = useRef(true);

  useLayoutEffect(() => {
    const element = scroll.current;
    if (element && stick.current) element.scrollTop = element.scrollHeight;
  });
  useEffect(() => {
    const element = box.current;
    if (!element) return;
    element.style.height = 'auto';
    element.style.height = `${Math.min(element.scrollHeight, 160)}px`;
  }, [text]);

  const submit = () => {
    const value = text.trim();
    if (!value || disabled) return;
    onSend(value);
    setText('');
  };

  return (
    <aside className="chat glass" aria-label="Assistant">
      <div className="chat-head">
        <h2>Assistant</h2>
        <button className={`btn assistant-connect-trigger${agent.status === 'ready' ? ' icon' : ''}`} type="button" aria-label={agent.status === 'ready' ? 'Assistant connection settings' : 'Connect assistant'} title={agent.status === 'ready' ? 'Assistant connection settings' : 'Connect assistant'} onClick={() => setConnectionOpen(true)}><Plug size={14} />{agent.status !== 'ready' && 'Connect assistant'}</button>
        <button className="btn icon" type="button" onClick={onClear} disabled={disabled} aria-label="Clear conversation" title="Clear conversation"><Trash2 size={15} /></button>
      </div>
      <div className="chat-context"><span>{projectName ? `${projectName} / World 01` : 'Create or open a project to start'}</span><div className="chat-context-row">
        <div className="chat-conversation-select"><select aria-label="Conversation" value={conversationId} disabled={disabled} onChange={event => onSelectChat(event.target.value)}>{conversations.map(chat => <option key={chat.id} value={chat.id}>{chat.title}</option>)}</select><ChevronDown size={14} strokeWidth={1.8} aria-hidden="true" /></div>
        <button className="btn icon" type="button" disabled={disabled} onClick={onNewChat} aria-label="New conversation" title="New conversation"><Plus size={15} /></button>
      </div>{agent.status === 'ready' && <label className="chat-model-label">Model<select aria-label="Assistant model" value={agent.model} disabled={agent.running} onChange={event => agent.setModel(event.target.value)}>{agent.models.map(model => <option key={model.id} value={model.id}>{model.name}</option>)}</select></label>}</div>
      <div className="chat-scroll" ref={scroll} onScroll={e => {
        const el = e.currentTarget;
        stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
      }}>
        {entries.length === 0 && (
          <div className="chat-empty">
            <h3>Describe what to build</h3>
            <p>This conversation belongs to this world. Start another chat whenever you want a fresh direction.</p>
          </div>
        )}
        {entries.map(entry => (
          <div key={entry.id} className={`msg ${entry.role === 'user' ? 'user' : 'assistant'}`}>
            {entry.text && (entry.role === 'user' ? <div className="body"><p>{entry.text}</p></div> : <Markdown text={entry.text} />)}
            {entry.tool && <ToolCard tool={entry.tool} />}
          </div>
        ))}
        <BuildCard />
        {agent.running && <div className="assistant-working" role="status"><LoaderCircle size={14} className="spin" />Assistant is working…</div>}
      </div>
      <form className="composer" onSubmit={e => { e.preventDefault(); submit(); }}>
        <div className="box">
          <textarea
            ref={box} rows={1} value={text} disabled={disabled} placeholder="Message the assistant" aria-label="Message"
            onChange={e => setText(e.target.value)}
            onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); submit(); } }}
          />
          {agent.running ? <button className="send" type="button" onClick={agent.stop} aria-label="Stop assistant"><Square size={14} fill="currentColor" /></button> : <button className="send" type="submit" disabled={disabled || !text.trim()} aria-label="Send"><ArrowUp size={18} strokeWidth={2.4} /></button>}
        </div>
        <p className="note">{error || (agent.status === 'ready' ? `API key in this browser · this world’s conversation` : host.mcpStatus === 'connected' ? 'External MCP connected. Sidebar chat uses its own assistant connection.' : 'Messages are saved locally. Connect an assistant to get replies here.')}</p>
      </form>
      {connectionOpen && <AssistantConnection agent={agent} onClose={() => setConnectionOpen(false)} />}
    </aside>
  );
}
