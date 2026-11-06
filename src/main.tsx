import React from "react";
import { createRoot } from "react-dom/client";

const root = createRoot(document.getElementById("root")!);
// The studio is the app. The bare engine editor (used by the end-to-end tests) stays at ?editor.
if (new URLSearchParams(location.search).has('editor')) {
  Promise.all([import('./engine/Editor'), import('./engine/editor.css')])
    .then(([{ default: Editor }]) => root.render(<React.StrictMode><Editor /></React.StrictMode>));
} else {
  import('./studio/Studio').then(({ default: Studio }) => root.render(<React.StrictMode><Studio /></React.StrictMode>));
}
