import { Component } from 'react';
import type { ReactNode } from 'react';

type Props = { children: ReactNode; name?: string; /** What to show when something inside fails; nothing by default, so a broken extra never hides the page. */ fallback?: (error: Error) => ReactNode };

/**
 * Keeps a failure inside it from blanking the page: it logs the error and shows `fallback` (or nothing). Wrap each optional section in one, and the whole page
 * in one that prints the error, so a blank screen always comes with a reason.
 */
export class Boundary extends Component<Props, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) { return { error }; }
  componentDidCatch(error: Error) { console.error(`[landing${this.props.name ? `:${this.props.name}` : ''}]`, error); }
  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return this.props.fallback ? this.props.fallback(error) : null;
  }
}

/** The page-level fallback: plain, readable on any background, and it says what failed. */
export const showError = (error: Error) => (
  <div role="alert" style={{ maxWidth: 720, margin: '12vh auto', padding: 24, font: '14px/1.5 system-ui, sans-serif', color: 'inherit' }}>
    <h1 style={{ fontSize: 20, margin: '0 0 8px' }}>The page could not load</h1>
    <p style={{ margin: '0 0 12px', opacity: 0.7 }}>Reloading usually fixes it. If it does not, this is what went wrong:</p>
    <pre style={{ whiteSpace: 'pre-wrap', margin: 0, padding: 12, borderRadius: 8, background: 'rgba(127,127,127,.15)', font: '12px/1.5 ui-monospace, monospace' }}>{error.message}{'\n'}{String(error.stack ?? '').split('\n').slice(1, 6).join('\n')}</pre>
    <p style={{ margin: '12px 0 0' }}><a href="/studio" style={{ color: 'inherit' }}>Open the studio instead</a></p>
  </div>
);
