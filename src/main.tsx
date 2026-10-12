import React from "react";
import { createRoot } from "react-dom/client";
import Editor from "./engine/Editor";
import "./engine/editor.css";

const root = createRoot(document.getElementById("root")!);
if (new URLSearchParams(location.search).has('legacy')) {
  Promise.all([import('./App'), import('./scene/factories'), import('./styles.css')])
    .then(async ([{ default: App }, { initializeGeometry }]) => {
      await initializeGeometry();
      root.render(<React.StrictMode><App /></React.StrictMode>);
    }).catch(error => root.render(<pre>{String(error)}</pre>));
} else {
  root.render(<React.StrictMode><Editor /></React.StrictMode>);
}
