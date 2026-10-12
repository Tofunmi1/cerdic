// Desktop viewer for the engine: look around any scene on a laptop, with a wide field of view and an
// optional split-screen mock headset. Opt in with ?viewer=1 (the tests and the headset pages are
// untouched). Click the view to hold the mouse and look around; Esc lets go again, and while the
// mouse is released you can hover, click and drag objects as usual.
//
// The engine owns the camera. This file only turns the DOM's mouse and keyboard into the few calls
// the runtime exposes: viewer_configure, viewer_look, viewer_move, viewer_set_locked, viewer_reset.

const SCENES = [
  ["xr", "Room: bitten cube, floor, lights"],
  ["shadows", "Shadows: box, ball, pillar"],
  ["kernel", "Kernel: cube minus sphere"],
  ["materials", "Materials: roughness x metal"],
  ["lights", "Lights: three coloured lamps"],
  ["stereo", "Split-screen mock headset"],
  ["bench1", "Benchmark S1: 50k triangles"],
  ["bench2", "Benchmark S2: 200k triangles"],
  ["bench3", "Benchmark S3: 400k triangles"],
];

const KEYS = {
  KeyW: [0, 0, 1], ArrowUp: [0, 0, 1],
  KeyS: [0, 0, -1], ArrowDown: [0, 0, -1],
  KeyD: [1, 0, 0], ArrowRight: [1, 0, 0],
  KeyA: [-1, 0, 0], ArrowLeft: [-1, 0, 0],
  KeyE: [0, 1, 0], Space: [0, 1, 0],
  KeyQ: [0, -1, 0], KeyC: [0, -1, 0],
};

export function startViewer({ canvas, api, params }) {
  const state = {
    fov: Number(params.get("fov") ?? 90),
    stereo: params.get("stereo") === "1",
    speed: Number(params.get("speed") ?? 3),
    lens: params.get("lens") ?? "off",
  };
  // A fisheye needs a wide view to bend; a headset lens needs two eyes. Pick lens, field of view
  // and split screen together so every choice shows something sensible.
  if (state.lens === "fisheye") { state.stereo = false; state.fov = Math.max(state.fov, 130); }
  if (state.lens === "headset") state.stereo = true;
  const held = new Set();
  let sprint = false;

  const apply = () => {
    api.viewer_configure(true, state.fov, state.stereo, state.speed);
    api.viewer_set_lens(state.lens);
    document.body.classList.toggle("split", state.stereo);
  };
  const chooseLens = (lens) => {
    state.lens = lens;
    if (lens === "fisheye") { state.stereo = false; state.fov = Math.max(state.fov, 130); }
    if (lens === "headset") state.stereo = true;
    syncControls();
    apply();
  };
  const setStereo = (on) => {
    state.stereo = on;
    // The lens that needs the other layout would do nothing, so drop it.
    if (state.lens === (on ? "fisheye" : "headset")) state.lens = "off";
    syncControls();
    apply();
  };
  const sendMotion = () => {
    let x = 0, y = 0, z = 0;
    for (const code of held) {
      const [dx, dy, dz] = KEYS[code];
      x += dx; y += dy; z += dz;
    }
    api.viewer_move(x, y, z, sprint);
  };

  canvas.addEventListener("click", () => {
    // Newer browsers return a promise that rejects when the lock is refused (for example in a
    // headless run); the page simply stays in cursor mode then.
    if (document.pointerLockElement !== canvas) Promise.resolve(canvas.requestPointerLock?.()).catch(() => {});
  });
  document.addEventListener("pointerlockchange", () => {
    const locked = document.pointerLockElement === canvas;
    api.viewer_set_locked(locked);
    hud.classList.toggle("locked", locked);
    if (!locked) { held.clear(); sendMotion(); }
  });
  document.addEventListener("mousemove", (event) => {
    if (document.pointerLockElement === canvas) api.viewer_look(event.movementX, event.movementY);
  });
  window.addEventListener("keydown", (event) => {
    if (event.target instanceof HTMLSelectElement || event.target instanceof HTMLInputElement) return;
    if (event.code in KEYS) { held.add(event.code); event.preventDefault(); sendMotion(); }
    else if (event.code === "ShiftLeft" || event.code === "ShiftRight") { sprint = true; sendMotion(); }
    else if (event.code === "KeyR") api.viewer_reset();
    else if (event.code === "KeyV") setStereo(!state.stereo);
    else if (event.code === "KeyL") chooseLens({ off: "fisheye", fisheye: "headset", headset: "off" }[state.lens] ?? "off");
    else if (event.code === "KeyH") hud.hidden = !hud.hidden;
  });
  window.addEventListener("keyup", (event) => {
    if (event.code in KEYS) { held.delete(event.code); sendMotion(); }
    else if (event.code === "ShiftLeft" || event.code === "ShiftRight") { sprint = false; sendMotion(); }
  });
  window.addEventListener("blur", () => { held.clear(); sprint = false; sendMotion(); });

  // The panel: scene, field of view, split screen, quality, and the key reference.
  const hud = document.createElement("div");
  hud.id = "viewer-hud";
  hud.innerHTML = `
    <style>
      #viewer-hud { position: fixed; left: 12px; top: 12px; width: 270px; padding: 12px 14px; color: #fff;
        font: 13px/1.45 system-ui, sans-serif; background: rgba(20,24,40,.72); border: 1px solid rgba(255,255,255,.22);
        border-radius: 14px; backdrop-filter: blur(14px) saturate(1.4); -webkit-backdrop-filter: blur(14px) saturate(1.4); }
      #viewer-hud[hidden] { display: none; }
      #viewer-hud label { display: block; margin: 8px 0 2px; opacity: .8; }
      #viewer-hud select, #viewer-hud input[type=range] { width: 100%; }
      #viewer-hud select { padding: 4px; border-radius: 6px; }
      #viewer-hud .keys { margin-top: 10px; opacity: .75; font-size: 12px; }
      #viewer-hud.locked { opacity: .35; }
      /* In split screen the panel sits over an eye: fade it until the cursor is on it. */
      body.split #viewer-hud { opacity: .2; }
      body.split #viewer-hud:hover { opacity: 1; }
      #crosshair { position: fixed; left: 50%; top: 50%; width: 10px; height: 10px; margin: -5px 0 0 -5px;
        border: 2px solid rgba(255,255,255,.85); border-radius: 50%; pointer-events: none; display: none; mix-blend-mode: difference; }
      body.look #crosshair { display: block; }
    </style>
    <b>Viewer</b>
    <label>Scene</label><select id="v-scene"></select>
    <label>Field of view <span id="v-fov-text"></span></label><input id="v-fov" type="range" min="50" max="150" step="1">
    <label><input id="v-stereo" type="checkbox"> Split-screen mock headset (V)</label>
    <label>Lens (L)</label><select id="v-lens">
      <option value="off">Flat perspective</option>
      <option value="fisheye">Fisheye (whole view)</option>
      <option value="headset">Headset lenses (split screen)</option></select>
    <label>Quality</label><select id="v-quality"><option>low</option><option>medium</option><option selected>high</option></select>
    <div class="keys">Click the view to look around (Esc to release).<br>
      WASD / arrows move, Space / Q up/down, Shift sprint,<br>R reset view, V split screen, L lens, H hide this panel.<br>
      Released: hover, click and drag objects. Held: the dot picks and grabs; the wheel pushes and pulls.</div>
    <div class="keys">Headset benchmark pages (for a Quest on the LAN server):
      <a href="?scene=bench1&bench=1" style="color:#9cf">S1</a> ·
      <a href="?scene=bench2&bench=1" style="color:#9cf">S2</a> ·
      <a href="?scene=bench3&bench=1" style="color:#9cf">S3</a></div>`;
  document.body.append(hud);
  const cross = document.createElement("div");
  cross.id = "crosshair";
  document.body.append(cross);

  const sceneSelect = hud.querySelector("#v-scene");
  for (const [id, label] of SCENES) sceneSelect.add(new Option(label, id));
  sceneSelect.value = params.get("scene") || "xr";
  sceneSelect.addEventListener("change", () => {
    const next = new URLSearchParams(location.search);
    next.set("scene", sceneSelect.value);
    next.set("viewer", "1");
    location.search = next.toString();
  });
  const fovInput = hud.querySelector("#v-fov");
  const fovText = hud.querySelector("#v-fov-text");
  const stereoBox = hud.querySelector("#v-stereo");
  const lensSelect = hud.querySelector("#v-lens");
  function syncControls() {
    fovInput.value = state.fov;
    fovText.textContent = `${state.fov}°`;
    stereoBox.checked = state.stereo;
    lensSelect.value = state.lens;
  }
  syncControls();
  fovInput.addEventListener("input", () => {
    state.fov = Number(fovInput.value);
    syncControls();
    apply();
  });
  stereoBox.addEventListener("change", () => setStereo(stereoBox.checked));
  lensSelect.addEventListener("change", () => chooseLens(lensSelect.value));
  const quality = hud.querySelector("#v-quality");
  quality.value = params.get("quality") || "high";
  quality.addEventListener("change", () => api.set_quality(quality.value));
  document.addEventListener("pointerlockchange", () => {
    document.body.classList.toggle("look", document.pointerLockElement === canvas);
  });

  apply();
  // `api` is returned so tests can drive the viewer without a real pointer lock.
  return { state, hud, api };
}
