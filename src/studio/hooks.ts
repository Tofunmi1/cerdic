import { useEffect, useReducer, useState } from 'react';
import { host } from '../engine/host';

/** Re-renders when the engine host reports a change (document, evaluation, boot). */
export function useHost() {
  const [, refresh] = useReducer((n: number) => n + 1, 0);
  useEffect(() => host.subscribe(refresh), []);
  return host;
}

export type Theme = 'auto' | 'dark' | 'light';

/** The colour theme: follows the system until chosen, and is remembered. */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(() => {
    try { return (localStorage.getItem('studio-theme') as Theme) || 'auto'; } catch { return 'auto'; }
  });
  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'auto') root.removeAttribute('data-theme'); else root.setAttribute('data-theme', theme);
    try { localStorage.setItem('studio-theme', theme); } catch { /* storage may be blocked */ }
  }, [theme]);
  const next = () => setTheme(theme === 'auto' ? 'dark' : theme === 'dark' ? 'light' : 'auto');
  return { theme, next };
}

/** The operator family without its version: "solid.box@1" becomes "solid". */
export const familyOf = (op: string) => op.split('.')[0];
