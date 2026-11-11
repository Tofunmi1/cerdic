import { useEffect, useMemo, useRef, useState } from 'react';
import { Search, TriangleAlert } from 'lucide-react';
import { EXAMPLES } from './examples';
import { loadWorldText } from './loadWorld';

/**
 * The examples as a menu under the top bar, available at any time: search, arrow keys and Enter, Esc to close. Opening one replaces the scene, which the menu
 * says when there is something in it.
 */
export function ExamplesMenu({ hasScene, onClose, onError, onOpen }: { hasScene: boolean; onClose: () => void; onError: (message: string) => void; onOpen?: (text: string, name: string, origin: string) => Promise<void> }) {
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const [busy, setBusy] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);

  const shown = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return EXAMPLES.filter((example) => words.every((word) => `${example.title} ${example.about} ${example.kind}`.toLowerCase().includes(word)));
  }, [query]);

  useEffect(() => setActive(0), [query]);
  useEffect(() => { list.current?.children[active]?.scrollIntoView({ block: 'nearest' }); }, [active]);
  // Click outside or Esc closes it; the button that opens it is marked so its own click is not read as "outside".
  useEffect(() => {
    const away = (event: MouseEvent) => {
      const target = event.target as Element | null;
      if (root.current && !root.current.contains(target) && !target?.closest('[data-examples-trigger]')) onClose();
    };
    document.addEventListener('mousedown', away);
    return () => document.removeEventListener('mousedown', away);
  }, [onClose]);

  const open = async (index: number) => {
    const example = shown[index];
    if (!example || busy) return;
    setBusy(example.id);
    try {
      if (onOpen) await onOpen(example.text, example.title, example.id);
      else await loadWorldText(example.text, example.id);
      onClose();
    } catch (error) {
      onError(`${example.title} could not be loaded: ${error instanceof Error ? error.message : String(error)}`);
      setBusy(null);
    }
  };

  const key = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') { event.preventDefault(); onClose(); }
    else if (event.key === 'ArrowDown') { event.preventDefault(); setActive((i) => Math.min(shown.length - 1, i + 1)); }
    else if (event.key === 'ArrowUp') { event.preventDefault(); setActive((i) => Math.max(0, i - 1)); }
    else if (event.key === 'Enter') { event.preventDefault(); void open(active); }
  };

  return (
    <div className="examples glass strong" ref={root} role="dialog" aria-label="Examples" onKeyDown={key}>
      <label className="examples-search">
        <Search size={15} />
        <input autoFocus value={query} placeholder={`Search ${EXAMPLES.length} examples`} aria-label="Search examples" onChange={(event) => setQuery(event.target.value)} />
      </label>
      {hasScene && <p className="examples-warn"><TriangleAlert size={13} />{onOpen ? 'Each example opens as a new project. Your current project stays saved.' : 'Opening an example replaces the current scene. Export it first to keep it.'}</p>}
      <ul ref={list} className="examples-list" role="listbox" aria-label="Examples">
        {shown.map((example, index) => (
          <li key={example.id} role="option" aria-selected={index === active} data-active={index === active} data-busy={busy === example.id}
            onMouseEnter={() => setActive(index)} onClick={() => void open(index)}>
            <span className="ex-row"><span className="ex-title">{example.title}</span><span className="chip">{busy === example.id ? 'Loading…' : example.kind}</span></span>
            <span className="ex-about">{example.about || 'A world written in the scene language.'}</span>
          </li>
        ))}
        {shown.length === 0 && <li className="examples-empty" role="presentation">Nothing matches “{query}”.</li>}
      </ul>
    </div>
  );
}
