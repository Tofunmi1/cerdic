import { StrictMode, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { boot, getCanvas } from "./engine.js";

// "persistent": React moves one engine-owned canvas into the panel. "naive": React renders its own
// canvas element (and re-creates it when the panel remounts), which is what the engine cannot follow.
function EngineCanvas({ naive }) {
  const host = useRef(null);
  useEffect(() => {
    const canvas = naive ? host.current.querySelector("canvas") : getCanvas();
    if (!naive) host.current.appendChild(canvas);
    boot().catch(() => {});
    return () => {
      if (!naive && canvas.parentNode === host.current) host.current.removeChild(canvas);
    };
  }, [naive]);
  return (
    <div ref={host} id="host" style={{ width: "100%", height: "100%" }}>
      {naive && <canvas id="ve" style={{ display: "block", width: "100%", height: "100%" }} />}
    </div>
  );
}

function App() {
  const [width, setWidth] = useState(360);
  const [remount, setRemount] = useState(0);
  const [typed, setTyped] = useState("");
  const naive = new URLSearchParams(location.search).get("naive") === "1";
  return (
    <div style={{ display: "flex", height: "100vh", color: "#eee", font: "13px system-ui" }}>
      <div id="left" style={{ width: 200, padding: 8 }}>
        <input id="field" value={typed} onChange={(e) => setTyped(e.target.value)} />
        <div id="typed">{typed}</div>
        <button id="remount" onClick={() => setRemount((n) => n + 1)}>remount panel</button>
        <button id="hide" onClick={() => setWidth(1)}>hide</button>
      </div>
      <div id="pane" style={{ width, height: "100%", flex: "none" }}>
        <EngineCanvas key={remount} naive={naive} />
      </div>
      <div id="right" style={{ padding: 8 }}>
        <button id="wide" onClick={() => setWidth(700)}>wide</button>
        <button id="narrow" onClick={() => setWidth(360)}>narrow</button>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
