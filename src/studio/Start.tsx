import { useMemo, useRef, useState } from 'react';
import { ArrowUp, FileUp, Sparkles } from 'lucide-react';
import { EXAMPLES } from './examples';
import { loadWorldFile, loadWorldText } from './loadWorld';

/**
 * What the person sees with an empty document: say what to build, pick an example world, or open a text file. It sits over the stage and goes
 * away as soon as the document has anything in it (or when they choose to start blank).
 */
export function Start({ onPrompt, onBlank }: { onPrompt: (text: string) => void; onBlank: () => void }) {
  const [prompt, setPrompt] = useState('');
  const [kind, setKind] = useState('all');
  const [busy, setBusy] = useState<string | null>(null);
  const [problem, setProblem] = useState('');
  const picker = useRef<HTMLInputElement>(null);

  const kinds = useMemo(() => ['all', ...Array.from(new Set(EXAMPLES.map((example) => example.kind))).sort()], []);
  const shown = EXAMPLES.filter((example) => kind === 'all' || example.kind === kind);

  const submit = () => {
    const text = prompt.trim();
    if (!text) return;
    onPrompt(text);
    setPrompt('');
  };
  const open = async (id: string, title: string, text: string) => {
    setBusy(id);
    setProblem('');
    try { await loadWorldText(text, id); } catch (error) { setProblem(`${title} could not be loaded: ${error instanceof Error ? error.message : String(error)}`); }
    setBusy(null);
  };
  const pick = async (file: File | undefined) => {
    if (!file) return;
    setProblem('');
    if (!(await loadWorldFile(file))) setProblem('That is not a text program. Choose a .txt file written in the scene language.');
  };

  return (
    <section className="start glass strong" aria-label="Start">
      <div className="start-head">
        <h1><Sparkles size={20} />What do you want to build?</h1>
        <p>Describe it and the assistant writes the scene, or start from an example. Everything is generated from text, so you can open, read and change any world.</p>
      </div>

      <form className="start-prompt" onSubmit={(event) => { event.preventDefault(); submit(); }}>
        <textarea
          value={prompt}
          rows={2}
          placeholder="A four-storey house on a cliff at golden hour, with a pool and a long reflecting pond…"
          aria-label="Describe what to build"
          onChange={(event) => setPrompt(event.target.value)}
          onKeyDown={(event) => { if (event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); submit(); } }}
        />
        <button className="btn primary icon" type="submit" disabled={!prompt.trim()} aria-label="Build it" title="Build it"><ArrowUp size={16} /></button>
      </form>

      <div className="start-bar">
        <div className="segmented" role="group" aria-label="Filter examples">
          {kinds.map((name) => <button key={name} type="button" aria-pressed={kind === name} onClick={() => setKind(name)}>{name === 'all' ? 'All examples' : name}</button>)}
        </div>
        <div className="grow" />
        <input ref={picker} type="file" accept=".txt,.world,text/plain" hidden onChange={(event) => { void pick(event.target.files?.[0]); event.target.value = ''; }} />
        <button className="btn" type="button" onClick={() => picker.current?.click()}><FileUp size={15} />Open a file</button>
        <button className="btn" type="button" onClick={onBlank}>Start blank</button>
      </div>
      {problem && <p className="start-problem" role="alert">{problem}</p>}

      <div className="start-grid" role="list">
        {shown.map((example) => (
          <button key={example.id} className="card" type="button" role="listitem" disabled={busy !== null} data-busy={busy === example.id} onClick={() => void open(example.id, example.title, example.text)}>
            <span className="card-title">{example.title}</span>
            <span className="card-about">{example.about || 'A world written in the scene language.'}</span>
            <span className="chip">{busy === example.id ? 'Loading…' : example.kind}</span>
          </button>
        ))}
      </div>
      <p className="start-foot">You can also drop a .txt file anywhere on this window.</p>
    </section>
  );
}
