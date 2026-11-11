import React from "react";
import { createRoot } from "react-dom/client";

const root = createRoot(document.getElementById("root")!);
const params = new URLSearchParams(location.search);
// A bare "/" is the landing page. The studio is the app at /studio, and any address that carries a setting (?world=, ?scene=, ?mcp=, ?docname= ...) goes straight to it,
// so existing links and tests keep working. The bare engine editor (used by the end-to-end tests) stays at ?editor.
const landing = (location.pathname === '/' || location.pathname === '/index.html') && [...params.keys()].length === 0;
if (params.has('editor')) {
  Promise.all([import('./engine/Editor'), import('./engine/editor.css')])
    .then(([{ default: Editor }]) => root.render(<React.StrictMode><Editor /></React.StrictMode>));
} else if (landing) {
  // The whole page sits in a boundary that prints what went wrong, and a module that fails to load says so too, so this never ends as a blank screen.
  Promise.all([import('./landing/Landing'), import('./landing/Boundary')])
    .then(([{ default: Landing }, { Boundary, showError }]) => root.render(<React.StrictMode><Boundary name="page" fallback={showError}><Landing /></Boundary></React.StrictMode>))
    .catch(error => import('./landing/Boundary').then(({ showError }) => root.render(showError(error instanceof Error ? error : new Error(String(error))))));
} else {
  // The engine is large, so only the studio asks for it early; the landing page does not.
  const preload = (attributes: Record<string, string>) => { const link = document.createElement('link'); Object.entries(attributes).forEach(([key, value]) => link.setAttribute(key, value)); document.head.append(link); };
  preload({ rel: 'preload', href: '/engine/ve_runtime_bg.wasm', as: 'fetch', type: 'application/wasm', crossorigin: '' });
  preload({ rel: 'modulepreload', href: '/engine/ve_runtime.js' });
  import('./studio/Studio').then(({ default: Studio }) => root.render(<React.StrictMode><Studio /></React.StrictMode>));
}
