import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";
import { initializeGeometry } from "./scene/factories";

const root = createRoot(document.getElementById("root")!);
initializeGeometry()
  .then(() =>
    root.render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    ),
  )
  .catch((error: unknown) => {
    root.render(
      <main className="boot-error">
        <h1>Geometry engine could not start</h1>
        <p>
          Run <code>bun run wasm:build</code> and reload.
        </p>
        <pre>{error instanceof Error ? error.message : String(error)}</pre>
      </main>,
    );
  });
