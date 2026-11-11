import { useMemo, useRef, useState } from 'react';
import { Check, Search } from 'lucide-react';
import { ALL_OPERATORS } from './operators';
import './landing-extra.css';

/**
 * Every operator in the registry, searchable. The family chips above it narrow the list; clicking an operator copies how it is written in a scene
 * (`solid.box@1`), so the page is a quick reference as well as a pitch.
 */
export function OperatorExplorer({ family, onFamily }: { family: string | null; onFamily: (family: string | null) => void }) {
  const [query, setQuery] = useState('');
  const [copied, setCopied] = useState<string | null>(null);
  const timer = useRef<number>(0);

  const shown = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return ALL_OPERATORS.filter((name) => (!family || name.startsWith(`${family}.`)) && words.every((word) => name.includes(word)));
  }, [query, family]);

  const copy = async (name: string) => {
    const text = `${name}@1`;
    try { await navigator.clipboard.writeText(text); } catch { /* the clipboard can be refused; the chip still shows what it is */ }
    setCopied(name);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setCopied(null), 1400);
  };

  return (
    <div className="explorer" data-reveal>
      <div className="explorer-head">
        <label className="explorer-search">
          <Search size={15} />
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={`Search ${ALL_OPERATORS.length} operators: bevel, loft, erode, ik…`} aria-label="Search operators" />
        </label>
        <span className="explorer-count">{shown.length} of {ALL_OPERATORS.length}{family ? <> in <b>{family}</b> <button type="button" onClick={() => onFamily(null)}>clear</button></> : null}</span>
      </div>
      <ul className="explorer-list" aria-label="Operators">
        {shown.map((name) => {
          const [group, rest] = [name.slice(0, name.indexOf('.')), name.slice(name.indexOf('.') + 1)];
          return (
            <li key={name}>
              <button type="button" onClick={() => void copy(name)} title={`Copy ${name}@1`}>
                <span className="op-family">{group}.</span><span className="op-name">{rest}</span>
                {copied === name && <span className="op-copied"><Check size={11} />copied</span>}
              </button>
            </li>
          );
        })}
        {shown.length === 0 && <li className="explorer-empty">Nothing matches “{query}”.</li>}
      </ul>
    </div>
  );
}
