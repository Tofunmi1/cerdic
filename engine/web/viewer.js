import { attachFxAudio } from "./fx-audio.js";
import { startGamepad } from "./gamepad.js";

// Desktop viewer for the engine: look around any scene on a laptop, with a wide field of view and an
// optional split-screen mock headset. Opt in with ?viewer=1 (the tests and the headset pages are
// untouched). Click the view to hold the mouse and look around; Esc lets go again, and while the
// mouse is released you can hover, click and drag objects as usual.
//
// The engine owns the camera. This file only turns the DOM's mouse and keyboard into the few calls
// the runtime exposes: viewer_configure, viewer_look, viewer_move, viewer_set_locked, viewer_reset.

const SCENES = [
  ["fx", "Fx: fire, smoke, fountain, rain, explosions, sound"],
  ["showcase", "Showcase: colourful props, studio lights"],
  ["stage", "Studio: your model in a lit room"],
  ["viaduct", "Model: Stone aqueduct · 150k triangles"],
  ["pegasus_studio", "Model: white pegasus in the studio"],
  ["angel_studio", "Model: winged angel in the studio"],
  ["building_asset", "Model: Art Deco building asset"],
  ["architecture_gallery", "City: 12 architecture families + streetscape"],
  ["church_sanctuary", "Model: Gothic parish church and garden"],
  ["main_street", "City: main street block with park and street life"],
  ["model_y", "Model: electric crossover in the studio"],
  ["anime_girl", "Model: anime girl with a bouquet in the studio"],
  ["eiffel_tower", "Model: Eiffel Tower lattice in the studio"],
  ["physics", "Demo: physics playground (press Play)"],
  ["physics_big", "Demo: bowling, dominoes, brick wall, funnel (press Play)"],
  ["physics_joints", "Demo: pendulum, door, drawer, spring, windmill, bridge, bell (press Play)"],
  ["physics_walk", "Demo: moon walk, platforms, stairs, crates (press Play, then WASD)"],
  ["animation", "Demo: animation and behaviours (press Play)"],
  ["scenery", "Demo: procedural lake, terrain and sky"],
  ["village", "Demo: village at sunset, walk around it (press Play)"],
  ["village_hd", "Demo: detailed village, tiles, windows, grass (press Play)"],
  ["beach_arch", "Demo: beach with a rock arch, palms and ocean (press Play)"],
  ["weathered_stone", "Demo: carved stone, realistic and toon, wear from the shape"],
  ["style_gallery", "Demo: one object in six styles (anime, realistic, painterly, flat, retro, ink)"],
  ["outdoor_sky", "Demo: terrain, trees and lake under a real sky, cascaded sun shadows"],
  ["materials_grid", "Demo: roughness x metal spheres reflecting the sky"],
  ["lit_room", "Demo: a lit room (the dark-room reference for global illumination)"],
  ["modern_room", "Room: contemporary living room with detailed furniture and warm materials"],
  ["mannequin", "Demo: procedural character with skeleton and walk cycle"],
  ["toon_gallery", "Demo: cel-shading gallery (physical vs 1 to 4 bands)"],
  ["gallery", "Editor comparison scene"],
  ["xr", "Room: bitten cube, floor, lights"],
  ["shadows", "Shadows: box, ball, pillar"],
  ["kernel", "Kernel: cube minus sphere"],
  ["materials", "Materials: roughness x metal"],
  ["lights", "Lights: three coloured lamps"],
  ["stereo", "Split-screen mock headset"],
  ["bench1", "Benchmark S1: 50k triangles"],
  ["bench2", "Benchmark S2: 200k triangles"],
  ["bench3", "Benchmark S3: 400k triangles"],
  ["bench4", "Benchmark S4: 1 million triangles"],
  ["bench5", "Benchmark S5: 2 million triangles"],
];

const KEYS = {
  KeyW: [0, 0, 1], ArrowUp: [0, 0, 1],
  KeyS: [0, 0, -1], ArrowDown: [0, 0, -1],
  KeyD: [1, 0, 0], ArrowRight: [1, 0, 0],
  KeyA: [-1, 0, 0], ArrowLeft: [-1, 0, 0],
  KeyE: [0, 1, 0], Space: [0, 1, 0],
  KeyQ: [0, -1, 0], KeyC: [0, -1, 0],
};

/**
 * A small panel for worlds that carry an animation state machine (`anim.machine`): a speed slider, a jump button and a box to set any
 * parameter or raise any trigger by name. It appears by itself once such a world is on screen and goes away when the world has none,
 * so a person can drive a character the model built (a game will drive the same parameters from its rules).
 */
function mountAnimationControls(api) {
  let panel = null;
  const build = () => {
    panel = document.createElement("div");
    panel.id = "anim-controls";
    panel.style.cssText = "position:fixed;left:12px;bottom:12px;z-index:30;padding:10px 12px;border-radius:10px;background:rgba(20,22,28,.82);color:#eee;font:12px/1.4 system-ui,sans-serif;min-width:230px;backdrop-filter:blur(8px)";
    panel.innerHTML = `<div style="font-weight:600;margin-bottom:6px">Animation</div>
      <label style="display:flex;gap:8px;align-items:center">speed <input id="ac-speed" type="range" min="0" max="4" step="0.05" value="0" style="flex:1"><span id="ac-speed-v" style="width:30px;text-align:right">0</span></label>
      <div style="display:flex;gap:6px;margin-top:8px"><button id="ac-jump">jump</button><button id="ac-air">in air</button><button id="ac-ground">on ground</button></div>
      <div style="display:flex;gap:6px;margin-top:8px"><input id="ac-name" placeholder="name" style="width:80px"><input id="ac-value" placeholder="value (blank = trigger)" style="flex:1"><button id="ac-set">go</button></div>`;
    document.body.appendChild(panel);
    const speed = panel.querySelector("#ac-speed");
    const shown = panel.querySelector("#ac-speed-v");
    speed.addEventListener("input", () => { shown.textContent = speed.value; api.animation_set_float("", "speed", Number(speed.value)); });
    panel.querySelector("#ac-jump").addEventListener("click", () => api.animation_trigger("", "jump"));
    panel.querySelector("#ac-air").addEventListener("click", () => api.animation_set_bool("", "grounded", false));
    panel.querySelector("#ac-ground").addEventListener("click", () => api.animation_set_bool("", "grounded", true));
    panel.querySelector("#ac-set").addEventListener("click", () => {
      const name = panel.querySelector("#ac-name").value.trim();
      const value = panel.querySelector("#ac-value").value.trim();
      if (!name) return;
      if (value === "") api.animation_trigger("", name);
      else if (value === "true" || value === "false") api.animation_set_bool("", name, value === "true");
      else if (Number.isFinite(Number(value))) api.animation_set_float("", name, Number(value));
    });
    api.viewer_animation(true, -1);
  };
  setInterval(() => {
    const has = Boolean(globalThis.__ve?.evaluation?.parts?.some((part) => part.state_machine));
    if (has && !panel) build();
    else if (!has && panel) { panel.remove(); panel = null; }
  }, 1000);
}

export function startViewer({ canvas, api, params, frameBounds = () => null, frameSelectionBounds = () => null,
  frameSocketPivot = () => null, profilePoints = () => null, curveControlPoints = () => null,
  overlayGeometry = () => ({ wire: [], normals: [] }),
  diagnosticNodes = () => [], parameterHandles = () => null, commitTransform = () => null,
  socketFrames = () => [], selectedSocket = () => null, pickSocket = () => {} }) {
  const evalOn = params.get("eval") === "1" || params.has("mcp");
  const state = {
    orbitMode: false,
    fov: Number(params.get("fov") ?? 45),
    stereo: params.get("stereo") === "1",
    speed: Number(params.get("speed") ?? 3),
    lens: params.get("lens") ?? "off",
    gizmoMode: "translate",
  };
  // A fisheye needs a wide view to bend; a headset lens needs two eyes. Pick lens, field of view
  // and split screen together so every choice shows something sensible.
  if (state.lens === "fisheye") { state.stereo = false; state.fov = Math.max(state.fov, 130); }
  if (state.lens === "headset") state.stereo = true;
  const held = new Set();
  let sprint = false;
  let gizmoDrag = null;
  let pivotSelect = null;

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
  // Inspecting a model: the wheel (or a trackpad scroll or pinch) zooms toward it, and dragging while
  // holding Alt, or with "drag to orbit" on, circles it. These run before the engine's own pointer
  // handling and keep an orbit drag from also grabbing an object.
  const frame = () => {
    const bounds = frameSelectionBounds() ?? frameBounds();
    if (bounds) api.viewer_frame(...bounds.min, ...bounds.max);
  };

  const gizmo = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  gizmo.id = "viewer-gizmo";
  gizmo.setAttribute("aria-label", "Viewport transform gizmo");
  document.body.append(gizmo);
  const projectGizmo = (bounds, width, height) => {
    const socketPivot = pivotSelect?.value === "socket" ? frameSocketPivot() : null;
    if (pivotSelect?.value === "socket" && !socketPivot) {
      gizmoStatus.textContent = "Select a named socket in the inspector to use it as the pivot.";
      return null;
    }
    const pivot = pivotSelect?.value === "origin" ? [0, 0, 0] : socketPivot;
    const raw = pivot
      ? api.viewer_gizmo_at(...bounds.min, ...bounds.max, ...pivot, width, height)
      : api.viewer_gizmo(...bounds.min, ...bounds.max, width, height);
    return raw ? JSON.parse(raw) : null;
  };
  // A message about what just happened to an edit (discarded, refused, committed) must outlast the next
  // redraw, which would otherwise replace it with the idle hint within a fraction of a second.
  let notice = { text: "", until: 0 };
  const say = (text) => {
    notice = { text, until: performance.now() + 6000 };
    gizmoStatus.textContent = text;
  };
  const idleStatus = (text) => { gizmoStatus.textContent = performance.now() < notice.until ? notice.text : text; };
  const placeGizmo = (rect) => {
    gizmo.style.left = `${rect.left}px`; gizmo.style.top = `${rect.top}px`;
    gizmo.setAttribute("width", rect.width); gizmo.setAttribute("height", rect.height);
    gizmo.setAttribute("viewBox", `0 0 ${rect.width} ${rect.height}`);
  };
  // Bounds of every node a current document or evaluation issue names, drawn red. A view aid like the
  // other overlays: it needs no selection, and it never touches the document or its evaluation hash.
  const diagnosticChildren = (rect) => {
    const children = [];
    if (!diagnosticsToggle.checked) return children;
    for (const diagnosticBounds of diagnosticNodes()) {
      // The pivot choice is about editing; these boxes are drawn from the plain projection.
      const raw = api.viewer_gizmo(...diagnosticBounds.min, ...diagnosticBounds.max, rect.width, rect.height);
      const diagnostic = raw ? JSON.parse(raw) : null;
      if (!diagnostic) continue;
      for (const [from, to] of diagnostic.bounds) {
        const edge = document.createElementNS(gizmo.namespaceURI, "line");
        edge.setAttribute("x1", from[0]); edge.setAttribute("y1", from[1]);
        edge.setAttribute("x2", to[0]); edge.setAttribute("y2", to[1]);
        edge.classList.add("gizmo-diagnostic"); children.push(edge);
      }
    }
    return children;
  };
  // The named sockets (frames) on the selected parts, drawn as oriented triads: X red, Y green, Z blue,
  // with a disc at the origin to pick one. They show where an attachment would match and what a socket
  // pivot is; picking is the same selection the inspector's socket list makes.
  const socketChildren = (rect, bounds) => {
    const children = [];
    const span = bounds.max.map((value, index) => value - bounds.min[index]);
    const length = Math.max(Math.hypot(...span) * 0.18, 0.05);
    const chosen = selectedSocket();
    const project = (world) => {
      const raw = api.viewer_project(...world, rect.width, rect.height);
      return raw ? JSON.parse(raw) : null;
    };
    for (const frame of socketFrames()) {
      const origin = project(frame.position);
      if (!origin) continue;
      frame.axes.forEach((direction, axis) => {
        const tip = project(frame.position.map((value, index) => value + direction[index] * length));
        if (!tip) return;
        const line = document.createElementNS(gizmo.namespaceURI, "line");
        line.setAttribute("x1", origin.x); line.setAttribute("y1", origin.y);
        line.setAttribute("x2", tip.x); line.setAttribute("y2", tip.y);
        line.classList.add("gizmo-frame", `axis-${axis}`); children.push(line);
      });
      const disc = document.createElementNS(gizmo.namespaceURI, "circle");
      disc.setAttribute("cx", origin.x); disc.setAttribute("cy", origin.y); disc.setAttribute("r", 8);
      disc.dataset.socketPart = frame.part; disc.dataset.socketName = frame.name;
      disc.classList.add("gizmo-socket");
      if (chosen?.part === frame.part && chosen?.name === frame.name) disc.classList.add("selected");
      const title = document.createElementNS(gizmo.namespaceURI, "title");
      title.textContent = `Socket ${frame.name}`;
      disc.append(title);
      children.push(disc);
    }
    return children;
  };
  // Nothing editable is selected: no handles, but the view overlays that need no selection still show.
  const drawOverlaysOnly = (rect, message) => {
    if (rect.width && rect.height) placeGizmo(rect);
    gizmo.replaceChildren(...(rect.width && rect.height ? diagnosticChildren(rect) : []));
    idleStatus(message);
  };
  const curveSamples = (curve, movedIndex = null, delta = null) => {
    if (curve.kind === "circle") {
      const count = Math.max(12, Math.min(256, curve.segments));
      return Array.from({ length: count + 1 }, (_, index) => {
        const angle = Math.PI * 2 * index / count;
        return [curve.center[0] + Math.cos(angle) * curve.radius, curve.center[1],
          curve.center[2] + Math.sin(angle) * curve.radius];
      });
    }
    const controls = curve.points.map((value, index) => index === movedIndex && delta
      ? value.map((component, axis) => component + delta[axis]) : value);
    if (curve.kind !== "bezier" || controls.length !== 4) {
      return curve.closed && controls.length ? [...controls, controls[0]] : controls;
    }
    return Array.from({ length: 49 }, (_, step) => {
      const t = step / 48, inverse = 1 - t;
      return [0, 1, 2].map((axis) => inverse ** 3 * controls[0][axis]
        + 3 * inverse ** 2 * t * controls[1][axis] + 3 * inverse * t ** 2 * controls[2][axis]
        + t ** 3 * controls[3][axis]);
    });
  };
  const drawGizmo = () => {
    if (gizmoDrag) return;
    const selection = JSON.parse(api.selection_get());
    const rect = canvas.getBoundingClientRect();
    if (selection.instance !== null) {
      drawOverlaysOnly(rect, "Instance placements are selected as a set; individual placement edits are unavailable.");
      return;
    }
    const curve = curveControlPoints(selection.node);
    if (curve && rect.width && rect.height) {
      placeGizmo(rect);
      const children = [];
      const project = (world) => {
        const raw = api.viewer_project(...world, rect.width, rect.height);
        return raw ? JSON.parse(raw) : null;
      };
      const projected = curveSamples(curve).map(project).filter(Boolean);
      if (projected.length > 1) {
        const path = document.createElementNS(gizmo.namespaceURI, "polyline");
        path.setAttribute("points", projected.map(({ x, y }) => `${x},${y}`).join(" "));
        path.classList.add("gizmo-curve"); children.push(path);
      }
      if (curve.kind === "bezier") {
        const control = curve.points.map(project).filter(Boolean);
        if (control.length === curve.points.length) {
          const polygon = document.createElementNS(gizmo.namespaceURI, "polyline");
          polygon.setAttribute("points", control.map(({ x, y }) => `${x},${y}`).join(" "));
          polygon.classList.add("gizmo-control-polygon"); children.push(polygon);
        }
      }
      (curve.points ?? []).forEach((world, index) => {
        const location = project(world);
        if (!location) return;
        const hit = document.createElementNS(gizmo.namespaceURI, "circle");
        hit.setAttribute("cx", location.x); hit.setAttribute("cy", location.y); hit.setAttribute("r", 12);
        hit.dataset.curvePoint = index; hit.classList.add("curve-point-hit"); children.push(hit);
        const visible = hit.cloneNode(false); visible.removeAttribute("data-curve-point");
        visible.classList.remove("curve-point-hit"); visible.classList.add("curve-point"); children.push(visible);
      });
      gizmo.replaceChildren(...children);
      idleStatus(curve.kind === "circle" ? "Sampled circle path · edit centre and radius in the inspector"
        : "Curve control points · drag a point to edit · Esc cancels");
      return;
    }
    const bounds = frameSelectionBounds();
    if (!bounds || !rect.width || !rect.height) {
      drawOverlaysOnly(rect, selection.node ? "Selected output has no current bounds." : "Select a part to show handles.");
      return;
    }
    let geometry;
    try { geometry = projectGizmo(bounds, rect.width, rect.height); } catch { gizmo.replaceChildren(); return; }
    if (!geometry) { gizmo.replaceChildren(); return; }
    placeGizmo(rect);
    const children = [];
    const parameters = parameterHandles(selection.node);
    const line = (start, end, axis, tag = "axis", parameter = null) => {
      const hit = document.createElementNS(gizmo.namespaceURI, "line");
      hit.setAttribute("x1", start[0]); hit.setAttribute("y1", start[1]);
      hit.setAttribute("x2", end[0]); hit.setAttribute("y2", end[1]);
      hit.dataset[tag] = axis;
      if (parameter) hit.dataset.parameter = parameter;
      hit.classList.add("gizmo-hit"); children.push(hit);
      const visible = hit.cloneNode(false); visible.removeAttribute(`data-${tag}`);
      visible.removeAttribute("data-parameter");
      visible.classList.remove("gizmo-hit"); visible.classList.add(`gizmo-${tag}`, `axis-${axis}`);
      children.push(visible);
    };
    if (state.gizmoMode === "rotate") {
      geometry.rings.forEach((points, axis) => {
        if (points.length < 3) return;
        if (parameters?.kind === "revolve" && axis !== 1) return;
        const ring = document.createElementNS(gizmo.namespaceURI, "polyline");
        ring.setAttribute("points", points.map((point) => point.join(",")).join(" "));
        ring.dataset.ringAxis = axis;
        if (parameters?.kind === "revolve" && axis === 1) ring.dataset.parameter = "revolve.angle";
        if (parameters?.kind === "frame" && parameters.rotation) ring.dataset.parameter = "rotation";
        ring.classList.add("gizmo-hit"); children.push(ring);
        const visible = ring.cloneNode(false); visible.removeAttribute("data-ring-axis");
        visible.removeAttribute("data-parameter");
        visible.classList.remove("gizmo-hit"); visible.classList.add("gizmo-ring", `axis-${axis}`); children.push(visible);
      });
    } else {
      for (const axis of geometry.axes) {
        const parameter = parameters?.kind === "array" ? "spacing"
          : parameters?.kind === "repeat" ? "step"
            : parameters?.kind === "frame" ? "position" : null;
        line(axis.start, axis.end, axis.axis, "axis", parameter);
      }
      const profile = profilePoints(selection.node);
      const handles = profile?.kind === "polygon" ? profile.points
        : profile?.kind === "radius" ? [[profile.radius, 0]] : [];
      if (handles.length) {
        handles.forEach((point, index) => {
          const projected = api.viewer_project(point[0], point[1], 0, rect.width, rect.height);
          if (!projected) return;
          const location = JSON.parse(projected);
          const hit = document.createElementNS(gizmo.namespaceURI, "circle");
          hit.setAttribute("cx", location.x); hit.setAttribute("cy", location.y); hit.setAttribute("r", 12);
          if (profile.kind === "radius") hit.dataset.profileRadius = "true";
          else hit.dataset.profilePoint = index;
          hit.classList.add("profile-hit"); children.push(hit);
          const visible = hit.cloneNode(false); visible.removeAttribute("data-profile-point"); visible.removeAttribute("data-profile-radius");
          visible.classList.remove("profile-hit"); visible.classList.add("profile-point"); children.push(visible);
        });
      }
    }
    if (boundsToggle.checked) {
      for (const [from, to] of geometry.bounds) {
        const edge = document.createElementNS(gizmo.namespaceURI, "line");
        edge.setAttribute("x1", from[0]); edge.setAttribute("y1", from[1]);
        edge.setAttribute("x2", to[0]); edge.setAttribute("y2", to[1]);
        edge.classList.add("gizmo-bounds"); children.push(edge);
      }
    }
    const addSegments = (segments, className) => {
      for (const [from, to] of segments) {
        const a = api.viewer_project(...from, rect.width, rect.height);
        const b = api.viewer_project(...to, rect.width, rect.height);
        if (!a || !b) continue;
        const pa = JSON.parse(a), pb = JSON.parse(b);
        const edge = document.createElementNS(gizmo.namespaceURI, "line");
        edge.setAttribute("x1", pa.x); edge.setAttribute("y1", pa.y);
        edge.setAttribute("x2", pb.x); edge.setAttribute("y2", pb.y);
        edge.classList.add(className); children.push(edge);
      }
    };
    if (wireToggle.checked || normalsToggle.checked) {
      const overlay = overlayGeometry();
      if (wireToggle.checked) addSegments(overlay.wire ?? [], "gizmo-wire");
      if (normalsToggle.checked) addSegments(overlay.normals ?? [], "gizmo-normal");
    }
    children.push(...socketChildren(rect, bounds));
    children.push(...diagnosticChildren(rect));
    gizmo.replaceChildren(...children);
    idleStatus("Drag a handle to edit the selected part.");
  };

  const previewPoint = (point, delta, pivot) => {
    if (delta.kind === "translate") return point.map((value, index) => value + delta.value[index]);
    const axis = delta.kind === "rotate" ? delta.axis : delta.axis;
    const offset = point.map((value, index) => value - pivot[index]);
    if (delta.kind === "scale") {
      return offset.map((value, index) => pivot[index] + value * (index === axis.indexOf(1) ? delta.factor : 1));
    }
    const amount = delta.radians, cosine = Math.cos(amount), sine = Math.sin(amount);
    const projection = axis.reduce((sum, value, index) => sum + value * offset[index], 0);
    const cross = [axis[1] * offset[2] - axis[2] * offset[1],
      axis[2] * offset[0] - axis[0] * offset[2], axis[0] * offset[1] - axis[1] * offset[0]];
    return offset.map((value, index) => pivot[index] + value * cosine + cross[index] * sine
      + axis[index] * projection * (1 - cosine));
  };
  const drawDragPreview = (active, delta) => {
    const rect = canvas.getBoundingClientRect();
    const point = (world) => {
      const projected = api.viewer_project(...world, rect.width, rect.height);
      return projected ? JSON.parse(projected) : null;
    };
    const children = [];
    if (delta.kind === "translate" && active.screenBounds?.length) {
      const span = active.bounds.max.map((value, index) => value - active.bounds.min[index]);
      const worldLength = Math.max(Math.hypot(...span) * 0.5, 0.12) * 1.35;
      const axis = active.axes.find((item) => item.axis === active.axis);
      const shift = axis ? [0, 1].map((index) => (axis.end[index] - axis.start[index])
        * delta.value[active.axis] / worldLength) : [0, 0];
      for (const [from, to] of active.screenBounds) {
        const line = document.createElementNS(gizmo.namespaceURI, "line");
        line.setAttribute("x1", from[0] + shift[0]); line.setAttribute("y1", from[1] + shift[1]);
        line.setAttribute("x2", to[0] + shift[0]); line.setAttribute("y2", to[1] + shift[1]);
        line.classList.add("gizmo-preview"); children.push(line);
      }
    } else if (active.bounds) {
      const corners = Array.from({ length: 8 }, (_, corner) => [0, 1, 2].map((axis) =>
        corner & (1 << axis) ? active.bounds.max[axis] : active.bounds.min[axis]));
      const edges = corners.map((corner, index) => [1, 2, 4].filter((bit) => !(index & bit))
        .map((bit) => [index, index ^ bit])).flat();
      for (const [from, to] of edges) {
        const a = point(previewPoint(corners[from], delta, active.pivot));
        const b = point(previewPoint(corners[to], delta, active.pivot));
        if (!a || !b) continue;
        const line = document.createElementNS(gizmo.namespaceURI, "line");
        line.setAttribute("x1", a.x); line.setAttribute("y1", a.y);
        line.setAttribute("x2", b.x); line.setAttribute("y2", b.y);
        line.classList.add("gizmo-preview"); children.push(line);
      }
    }
    if (active.profilePoint && active.profileScreen && delta.kind === "translate") {
      const span = active.bounds.max.map((value, index) => value - active.bounds.min[index]);
      const worldLength = Math.max(Math.hypot(...span) * 0.5, 0.12) * 1.35;
      const shift = [0, 1].reduce((sum, index) => {
        const axis = active.axes.find((item) => item.axis === index);
        if (!axis) return sum;
        return sum.map((value, screenAxis) => value + (axis.end[screenAxis] - axis.start[screenAxis])
          * delta.value[index] / worldLength);
      }, [0, 0]);
      const marker = document.createElementNS(gizmo.namespaceURI, "circle");
      marker.setAttribute("cx", active.profileScreen[0] + shift[0]);
      marker.setAttribute("cy", active.profileScreen[1] + shift[1]); marker.setAttribute("r", 7);
      marker.classList.add("gizmo-preview-point"); children.push(marker);
    }
    if (active.profile && delta.kind === "translate") {
      let profilePoints = [];
      if (active.profile.kind === "polygon") {
        profilePoints = active.profile.points.map((profilePoint, index) => index === active.profileIndex
          ? [profilePoint[0] + delta.value[0], profilePoint[1] + delta.value[1]] : profilePoint);
      } else if (active.profile.kind === "radius") {
        const radius = Math.max(1e-4, active.profile.radius + delta.value[0]);
        profilePoints = Array.from({ length: 49 }, (_, index) => {
          const angle = Math.PI * 2 * index / 48;
          return [Math.cos(angle) * radius, Math.sin(angle) * radius];
        });
      }
      const projected = profilePoints.map(([x, y]) => point([x, y, 0])).filter(Boolean);
      if (projected.length > 2) {
        const outline = document.createElementNS(gizmo.namespaceURI, "polyline");
        outline.setAttribute("points", projected.map(({ x, y }) => `${x},${y}`).join(" "));
        outline.classList.add("gizmo-preview-profile"); children.push(outline);
      }
    }
    if (active.curve && Number.isInteger(active.curveIndex) && delta.kind === "translate") {
      const projected = curveSamples(active.curve, active.curveIndex, delta.value).map(point).filter(Boolean);
      if (projected.length > 1) {
        const path = document.createElementNS(gizmo.namespaceURI, "polyline");
        path.setAttribute("points", projected.map(({ x, y }) => `${x},${y}`).join(" "));
        path.classList.add("gizmo-preview-curve"); children.push(path);
      }
    }
    gizmo.replaceChildren(...children);
  };

  const localPointer = (event) => {
    const rect = canvas.getBoundingClientRect();
    return { x: ((event.clientX - rect.left) / rect.width) * 2 - 1,
      y: 1 - ((event.clientY - rect.top) / rect.height) * 2, aspect: rect.width / rect.height };
  };
  // Every way a drag can end without a commit: Escape, pointer cancel, lost capture, losing focus, or
  // the document moving under it. Nothing was committed during the drag, so abandoning is just clearing.
  const cancelGizmo = (message = "Drag cancelled.") => {
    if (!gizmoDrag) return;
    gizmoDrag = null;
    api.gizmo_cancel();
    say(message);
    drawGizmo();
  };
  gizmo.addEventListener("pointerdown", (event) => {
    // A socket disc selects that socket on the press itself: the overlay is redrawn several times a
    // second, so waiting for a click could lose the element between press and release.
    const socketHit = event.target.closest?.("[data-socket-name]");
    if (socketHit && event.button === 0 && document.pointerLockElement !== canvas) {
      pickSocket(socketHit.dataset.socketPart, socketHit.dataset.socketName);
      event.preventDefault(); event.stopPropagation();
      return;
    }
    const profileHit = event.target.closest?.("[data-profile-point], [data-profile-radius]");
    const curveHit = event.target.closest?.("[data-curve-point]");
    const hit = event.target.closest?.("[data-axis], [data-ring-axis]");
    if ((!hit && !profileHit && !curveHit) || document.pointerLockElement === canvas || event.button !== 0) return;
    const selected = JSON.parse(api.selection_get());
    if (selected.instance !== null) {
      say("Select the instance set to edit its spacing; individual placement edits are unavailable.");
      event.preventDefault(); event.stopPropagation();
      return;
    }
    const bounds = frameSelectionBounds();
    if (!selected.node || (!bounds && !curveHit)) return;
    const points = bounds ? projectGizmo(bounds, canvas.clientWidth, canvas.clientHeight) : null;
    if (bounds && !points) return;
    const docState = api.document_state();
    if (docState.pending_document) {
      // The last good scene stays visible and selectable; a new edit waits for the one being evaluated.
      say("The previous edit is still being evaluated; try again when it finishes.");
      event.preventDefault(); event.stopPropagation();
      return;
    }
    const pointer = localPointer(event);
    const curve = curveHit ? curveControlPoints(selected.node) : null;
    const curveIndex = curveHit ? Number(curveHit.dataset.curvePoint) : null;
    const curvePoint = curve && Number.isInteger(curveIndex) ? curve.points[curveIndex] : null;
    const profile = profileHit ? profilePoints(selected.node) : null;
    const profileRadius = profileHit?.dataset.profileRadius === "true";
    const profileIndex = profileHit && !profileRadius ? Number(profileHit.dataset.profilePoint) : null;
    const profilePoint = profileRadius ? [profile.radius, 0]
      : profile && profileIndex !== null ? profile.points[profileIndex] : null;
    const profileScreen = profileHit
      ? [Number(profileHit.getAttribute("cx")), Number(profileHit.getAttribute("cy"))] : null;
    const curveScreen = curveHit
      ? [Number(curveHit.getAttribute("cx")), Number(curveHit.getAttribute("cy"))] : null;
    const axis = hit ? Number(hit.dataset.axis ?? hit.dataset.ringAxis) : null;
    const started = curvePoint
      ? api.gizmo_begin_camera_plane(pointer.x, pointer.y, pointer.aspect, ...curvePoint)
      : profilePoint
      ? api.gizmo_begin_profile(pointer.x, pointer.y, pointer.aspect, profilePoint[0], profilePoint[1])
      : hit && api.gizmo_begin(state.gizmoMode, axis, pointer.x, pointer.y, pointer.aspect, ...points.pivot);
    if (!started) return;
    gizmoDrag = { pointerId: event.pointerId, node: selected.node, axis,
      pivot: points?.pivot ?? curvePoint, bounds, profilePoint, profileScreen, curve, curveIndex, curveScreen,
      screenBounds: points?.bounds, axes: points?.axes, profile, profileIndex, profileRadius,
      parameter: hit?.dataset.parameter ?? null,
      revision: docState.revision, last: null };
    gizmo.setPointerCapture(event.pointerId);
    event.preventDefault(); event.stopPropagation();
    gizmoStatus.textContent = Number.isInteger(curveIndex) ? "Move curve control point · release to commit · Esc to cancel"
      : `${state.gizmoMode} ${"XYZ"[axis]} · release to commit · Esc to cancel`;
  });
  gizmo.addEventListener("pointermove", (event) => {
    if (!gizmoDrag || gizmoDrag.pointerId !== event.pointerId) return;
    if (api.document_state().revision !== gizmoDrag.revision) {
      cancelGizmo("Document changed during drag; discarded.");
      event.preventDefault(); event.stopPropagation();
      return;
    }
    const pointer = localPointer(event);
    const raw = api.gizmo_update(pointer.x, pointer.y, pointer.aspect,
      Number(gridInput.value), Number(angleInput.value) * Math.PI / 180, Number(scaleInput.value));
    if (!raw) return;
    gizmoDrag.last = JSON.parse(raw);
    const delta = gizmoDrag.last;
    drawDragPreview(gizmoDrag, delta);
    gizmoStatus.textContent = delta.kind === "translate" ? `Move · ${delta.value.map((v) => v.toFixed(3)).join(", ")} m`
      : delta.kind === "rotate" ? `Rotate · ${(delta.radians * 180 / Math.PI).toFixed(1)}°`
        : `Scale · ${delta.factor.toFixed(3)}×`;
    event.preventDefault(); event.stopPropagation();
  });
  const finishGizmo = (event, commit) => {
    if (!gizmoDrag || gizmoDrag.pointerId !== event.pointerId) return;
    const active = gizmoDrag;
    gizmoDrag = null;
    api.gizmo_cancel();
    const outcome = commit && active.last ? commitTransform({ node: active.node, revision: active.revision,
      pivot: active.pivot, axis: active.axis, profileIndex: active.profileIndex,
      profileRadius: active.profileRadius, curveIndex: active.curveIndex,
      parameter: active.parameter, delta: active.last }) : null;
    say(!commit ? "Drag cancelled."
      : outcome?.conflict === "pending" ? "The previous edit is still being evaluated; transform discarded."
      : outcome?.conflict ? "Document changed during drag; transform discarded."
        : outcome?.refused ? "This handle edit was refused; check the inspector diagnostic."
          : outcome?.rejected ? "This handle edit was refused by the document."
            : outcome?.committed || outcome?.pending ? "Transform committed." : "No transform change.");
    drawGizmo();
    event.preventDefault(); event.stopPropagation();
  };
  gizmo.addEventListener("pointerup", (event) => finishGizmo(event, true));
  gizmo.addEventListener("pointercancel", (event) => finishGizmo(event, false));
  // Losing the capture without a pointerup (a dragged-away window, an alert, a lost device) must not
  // leave a half-made edit; after a normal pointerup the drag is already gone and this does nothing.
  gizmo.addEventListener("lostpointercapture", (event) => {
    if (gizmoDrag?.pointerId === event.pointerId) cancelGizmo("Pointer capture lost; drag cancelled.");
  });
  window.addEventListener("blur", () => cancelGizmo("Window lost focus; drag cancelled."));
  window.addEventListener("keydown", (event) => {
    if (event.code === "Escape" && gizmoDrag) { cancelGizmo(); event.preventDefault(); event.stopImmediatePropagation(); }
  }, true);
  canvas.addEventListener("wheel", (event) => {
    if (document.pointerLockElement === canvas || event.buttons & 1) return;
    event.preventDefault();
    api.viewer_zoom(event.deltaY * (event.deltaMode === 1 ? 33 : 1));
  }, { passive: false });
  let orbiting = null;
  const orbitWanted = (event) => event.button === 1 || (event.button === 0 && (event.altKey || state.orbitMode));
  window.addEventListener("pointerdown", (event) => {
    if (event.target !== canvas || document.pointerLockElement === canvas || !orbitWanted(event)) return;
    orbiting = event.pointerId;
    canvas.setPointerCapture?.(event.pointerId);
    event.preventDefault();
    event.stopImmediatePropagation();
  }, true);
  window.addEventListener("pointermove", (event) => {
    if (orbiting !== event.pointerId) return;
    api.viewer_orbit(event.movementX, event.movementY);
    event.stopImmediatePropagation();
  }, true);
  const endOrbit = (event) => {
    if (orbiting !== event.pointerId) return;
    orbiting = null;
    event.stopImmediatePropagation();
  };
  window.addEventListener("pointerup", endOrbit, true);
  window.addEventListener("pointercancel", endOrbit, true);
  // The desktop hand of a game: hold G to grip (pick things up), hold T to pull the trigger (use). A headset uses its controllers.
  const handKeys = { KeyG: false, KeyT: false };
  const sendHand = () => api.game_desktop_hand?.(handKeys.KeyG, handKeys.KeyT);
  window.addEventListener("keydown", (event) => {
    if (event.target instanceof HTMLSelectElement || event.target instanceof HTMLInputElement) return;
    if (event.code in handKeys) { if (!handKeys[event.code]) { handKeys[event.code] = true; sendHand(); } return; }
    if (event.code === "KeyF") { frame(); return; }
    if (event.code === "KeyE" && params.get("scene") === "fx") api.fx_explode?.();
    if (event.code === "KeyO") { state.orbitMode = !state.orbitMode; syncControls(); return; }
    if (event.code in KEYS) { held.add(event.code); event.preventDefault(); sendMotion(); }
    else if (event.code === "ShiftLeft" || event.code === "ShiftRight") { sprint = true; sendMotion(); }
    else if (event.code === "KeyR") api.viewer_reset();
    else if (event.code === "KeyV") setStereo(!state.stereo);
    else if (event.code === "KeyL") chooseLens({ off: "fisheye", fisheye: "headset", headset: "off" }[state.lens] ?? "off");
    else if (event.code === "KeyH") hud.hidden = !hud.hidden;
  });
  window.addEventListener("keyup", (event) => {
    if (event.code in handKeys) { handKeys[event.code] = false; sendHand(); return; }
    if (event.code in KEYS) { held.delete(event.code); sendMotion(); }
    else if (event.code === "ShiftLeft" || event.code === "ShiftRight") { sprint = false; sendMotion(); }
  });
  window.addEventListener("blur", () => { held.clear(); sprint = false; sendMotion(); handKeys.KeyG = handKeys.KeyT = false; sendHand(); });
  // Scripts hear the keyboard and, while the mouse is locked to the view, its buttons: on("key", e) with e = { code: "KeyQ" | "Mouse0" ..., down, repeat }.
  const typing = (target) => target instanceof HTMLSelectElement || target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target?.isContentEditable;
  const tell = (code, down, repeat = false) => globalThis.__ve?.scripts?.emit("key", { code, down, repeat });
  window.addEventListener("keydown", (event) => { if (!typing(event.target)) tell(event.code, true, event.repeat); });
  window.addEventListener("keyup", (event) => { if (!typing(event.target)) tell(event.code, false); });
  window.addEventListener("mousedown", (event) => { if (document.pointerLockElement === canvas) tell(`Mouse${event.button}`, true); });
  window.addEventListener("mouseup", (event) => { if (document.pointerLockElement === canvas) tell(`Mouse${event.button}`, false); });
  window.addEventListener("contextmenu", (event) => { if (document.pointerLockElement === canvas) event.preventDefault(); });
  // ?play=1 starts play mode (physics, walking, scripts' player) as soon as the scene has something to play, so a shared link opens straight into the game.
  if (params.get("play") === "1") {
    const autoPlay = setInterval(() => {
      const button = document.getElementById("play-mode");
      if (button && !button.hidden) { clearInterval(autoPlay); if (button.getAttribute("aria-pressed") !== "true") button.click(); }
    }, 500);
  }

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
      #viewer-gizmo { position: fixed; z-index: 20; overflow: visible; pointer-events: none; }
      #viewer-gizmo .gizmo-hit { fill: none; stroke: transparent; stroke-width: 18; pointer-events: stroke; cursor: grab; }
      #viewer-gizmo .gizmo-axis, #viewer-gizmo .gizmo-ring { fill: none; stroke-width: 3; pointer-events: none; }
      #viewer-gizmo .gizmo-bounds { stroke: rgba(255,255,255,.55); stroke-width: 1; stroke-dasharray: 3 4; pointer-events: none; }
      #viewer-gizmo .gizmo-wire { stroke: rgba(80,210,255,.65); stroke-width: 1; pointer-events: none; }
      #viewer-gizmo .gizmo-normal { stroke: #ffd166; stroke-width: 1.5; pointer-events: none; }
      #viewer-gizmo .gizmo-diagnostic { stroke: #ff465b; stroke-width: 2; stroke-dasharray: 4 3; pointer-events: none; }
      #viewer-gizmo .gizmo-preview { stroke: #65f0ff; stroke-width: 2; stroke-dasharray: 5 3; pointer-events: none; }
      #viewer-gizmo .gizmo-preview-profile { fill: rgba(101,240,255,.08); stroke: #65f0ff; stroke-width: 2; stroke-dasharray: 5 3; pointer-events: none; }
      #viewer-gizmo .gizmo-preview-point { fill: #65f0ff; stroke: #09242b; stroke-width: 2; pointer-events: none; }
      #viewer-gizmo .profile-hit { fill: transparent; stroke: transparent; pointer-events: all; cursor: grab; }
      #viewer-gizmo .profile-point { fill: #ffd166; stroke: #33250a; stroke-width: 2; pointer-events: none; }
      #viewer-gizmo .gizmo-curve { fill: none; stroke: #65f0ff; stroke-width: 2.5; pointer-events: none; }
      #viewer-gizmo .gizmo-control-polygon { fill: none; stroke: rgba(255,255,255,.48); stroke-width: 1.2; stroke-dasharray: 4 4; pointer-events: none; }
      #viewer-gizmo .curve-point-hit { fill: transparent; stroke: transparent; pointer-events: all; cursor: grab; }
      #viewer-gizmo .curve-point { fill: #ffd166; stroke: #33250a; stroke-width: 2; pointer-events: none; }
      #viewer-gizmo .gizmo-preview-curve { fill: none; stroke: #65f0ff; stroke-width: 3; stroke-dasharray: 5 3; pointer-events: none; }
      #viewer-gizmo .gizmo-frame { stroke-width: 2.5; pointer-events: none; }
      #viewer-gizmo .gizmo-socket { fill: rgba(255,255,255,.88); stroke: #1c2030; stroke-width: 2; pointer-events: all; cursor: pointer; }
      #viewer-gizmo .gizmo-socket.selected { fill: #ffd166; }
      #viewer-gizmo .axis-0 { stroke: #ff5d68; } #viewer-gizmo .axis-1 { stroke: #72dc8b; } #viewer-gizmo .axis-2 { stroke: #62a8ff; }
      #viewer-gizmo-controls { position: fixed; z-index: 30; top: 12px; left: 300px; padding: 7px 10px; color: white;
        background: rgba(18,22,34,.78); border: 1px solid rgba(255,255,255,.2); border-radius: 10px;
        backdrop-filter: blur(12px); font: 12px system-ui,sans-serif; display: flex; align-items: center; gap: 8px; }
      #viewer-gizmo-controls input { width: 48px; } #viewer-gizmo-status { min-width: 180px; opacity: .8; }
    </style>
    <b>Viewer</b>
    <label>Scene</label><select id="v-scene"></select>
    <label>Field of view <span id="v-fov-text"></span></label><input id="v-fov" type="range" min="35" max="150" step="1">
    <label><input id="v-stereo" type="checkbox"> Split-screen mock headset (V)</label>
    <label><input id="v-orbit" type="checkbox"> Drag to orbit (O)</label>
    <button id="v-sound" type="button" style="margin-top:6px;padding:3px 10px;border-radius:8px;border:1px solid rgba(255,255,255,.3);background:rgba(255,255,255,.12);color:#fff;font:inherit;cursor:pointer">Sound off</button>
    <button id="v-frame" type="button" style="margin-top:6px;padding:3px 10px;border-radius:8px;border:1px solid rgba(255,255,255,.3);background:rgba(255,255,255,.12);color:#fff;font:inherit;cursor:pointer">Frame selected (F)</button>
    <label>Lens (L)</label><select id="v-lens">
      <option value="off">Flat perspective</option>
      <option value="fisheye">Fisheye (whole view)</option>
      <option value="headset">Headset lenses (split screen)</option></select>
    <label>Quality</label><select id="v-quality"><option>low</option><option>quest</option><option>medium</option><option selected>high</option></select>
    <label><input id="v-shadows" type="checkbox"> Sun shadows</label>
    <label><input id="v-specular-aa" type="checkbox"> Specular anti-aliasing</label>
    <label><input id="v-ssao" type="checkbox"> Screen-space ambient occlusion</label>
    <label><input id="v-bloom" type="checkbox"> Bloom</label>
    <label><input id="v-auto-exposure" type="checkbox"> Auto exposure</label>
    <label>Exposure <span id="v-exposure-text"></span></label><input id="v-exposure" type="range" min="0.25" max="4" step="0.05">
    <label>Grade</label><select id="v-grade">
      <option value="neutral">Neutral</option>
      <option value="studio">Studio</option>
      <option value="warm">Warm</option>
      <option value="cool">Cool</option>
      <option value="mono">Mono</option>
      <option value="sunset">Sunset</option>
      <option value="vivid">Vivid</option>
      <option value="film">Film</option>
      <option value="night">Night</option>
      <option value="noir">Noir</option></select>
    <div class="keys">Click the view to look around (Esc to release).<br>
      WASD / arrows move, Space / Q up/down, Shift sprint,<br>R reset view, V split screen, L lens, H hide this panel.<br>In a game: hold G to grip (pick up), hold T to pull the trigger (use).<br>
      Not holding the mouse: wheel or scroll zooms, Alt + drag (or O, then drag) orbits, F frames the model.<br>
      Released: hover, click and drag objects. Held: the dot picks and grabs; the wheel pushes and pulls.</div>
    <div class="keys">Headset benchmark pages (for a Quest on the LAN server):
      <a href="?scene=bench1&bench=1" style="color:#9cf">S1</a> ·
      <a href="?scene=bench2&bench=1" style="color:#9cf">S2</a> ·
      <a href="?scene=bench3&bench=1" style="color:#9cf">S3</a> ·
      <a href="?scene=bench4&bench=1" style="color:#9cf">S4 (1M)</a> ·
      <a href="?scene=bench5&bench=1" style="color:#9cf">S5 (2M)</a></div>`;
  document.body.append(hud);
  if (params.get("scene") === "fx") {
    attachFxAudio({ api, params, button: hud.querySelector("#v-sound") });
  } else { hud.querySelector("#v-sound").hidden = true; }
  const gizmoControls = document.createElement("div");
  gizmoControls.id = "viewer-gizmo-controls";
  gizmoControls.innerHTML = `<label>Tool <select id="gizmo-mode"><option value="translate">Move</option><option value="rotate">Rotate</option><option value="scale">Scale</option></select></label>
    <label>Pivot <select id="gizmo-pivot"><option value="center">Part centre</option><option value="origin">World origin</option><option value="socket">Selected socket</option></select></label>
    <label>Grid m <input id="gizmo-grid" type="number" min="0" step="0.01" value="0.1"></label>
    <label>Angle ° <input id="gizmo-angle" type="number" min="0" step="1" value="15"></label>
    <label>Scale step <input id="gizmo-scale" type="number" min="0" step="0.01" value="0.1"></label>
    <label><input id="gizmo-bounds" type="checkbox"> Bounds</label>
    <label><input id="gizmo-wire" type="checkbox"> Wire</label>
    <label><input id="gizmo-normals" type="checkbox"> Normals</label>
    <label><input id="gizmo-diagnostics" type="checkbox"> Diagnostics</label><div id="gizmo-floating-parts" aria-live="polite"></div>
    <span id="viewer-gizmo-status">Select a part to show handles.</span>`;
  document.body.append(gizmoControls);
  const gizmoStatus = gizmoControls.querySelector("#viewer-gizmo-status");
  const gridInput = gizmoControls.querySelector("#gizmo-grid");
  const angleInput = gizmoControls.querySelector("#gizmo-angle");
  const scaleInput = gizmoControls.querySelector("#gizmo-scale");
  pivotSelect = gizmoControls.querySelector("#gizmo-pivot");
  pivotSelect.addEventListener("change", drawGizmo);
  const boundsToggle = gizmoControls.querySelector("#gizmo-bounds");
  const wireToggle = gizmoControls.querySelector("#gizmo-wire");
  const normalsToggle = gizmoControls.querySelector("#gizmo-normals");
  const diagnosticsToggle = gizmoControls.querySelector("#gizmo-diagnostics");
  const floatingParts = gizmoControls.querySelector("#gizmo-floating-parts");
  boundsToggle.addEventListener("change", drawGizmo);
  wireToggle.addEventListener("change", drawGizmo);
  normalsToggle.addEventListener("change", drawGizmo);
  diagnosticsToggle.addEventListener("change", drawGizmo);
  floatingParts.style.cssText = "display:none;max-width:380px;max-height:100px;overflow:auto;font:11px system-ui,sans-serif";
  const renderFloatingParts = () => {
    floatingParts.replaceChildren();
    if (!diagnosticsToggle.checked) { floatingParts.style.display = "none"; return; }
    floatingParts.style.display = "block";
    const attachments = globalThis.__ve?.evaluation?.attachments;
    const title = document.createElement("strong"); title.textContent = "Floating parts"; floatingParts.append(title);
    if (!attachments) { floatingParts.append(document.createTextNode(" · measuring final result…")); return; }
    if (attachments.error) { floatingParts.append(document.createTextNode(` · ${attachments.error}`)); return; }
    if (!attachments.floating.length) { floatingParts.append(document.createTextNode(" · all parts attached")); return; }
    for (const part of attachments.floating) {
      const row = document.createElement("button");
      row.type = "button"; row.textContent = `${part.name}  ${part.gap.toFixed(2)} m from  ${part.nearest}`;
      row.style.cssText = "display:block;width:100%;text-align:left;background:transparent;color:inherit;border:0;padding:2px;cursor:pointer";
      row.addEventListener("click", () => {
        const match = globalThis.__ve?.evaluation?.parts?.find(candidate => candidate.name === part.name);
        if (match) globalThis.__ve?.docPanel?.selection?.select(match.node);
      });
      floatingParts.append(row);
    }
  };
  gizmoControls.querySelector("#gizmo-mode").addEventListener("change", (event) => {
    state.gizmoMode = event.target.value;
    drawGizmo();
  });
  window.addEventListener("resize", drawGizmo);
  setInterval(drawGizmo, 120);
  setInterval(renderFloatingParts, 250);
  const cross = document.createElement("div");
  cross.id = "crosshair";
  document.body.append(cross);

  const sceneSelect = hud.querySelector("#v-scene");
  for (const [id, label] of SCENES) sceneSelect.add(new Option(label, id));
  sceneSelect.value = params.get("model") || params.get("scene") || (evalOn ? "stage" : "xr");
  sceneSelect.addEventListener("change", () => {
    const next = new URLSearchParams(location.search);
    if (["viaduct", "physics", "physics_big", "physics_joints", "physics_walk", "animation", "scenery", "village", "village_hd", "beach_arch", "mannequin", "toon_gallery", "weathered_stone", "style_gallery", "outdoor_sky", "materials_grid", "lit_room", "modern_room", "pegasus_studio", "angel_studio", "building_asset", "architecture_gallery", "model_y", "anime_girl", "eiffel_tower"].includes(sceneSelect.value)) {
      // Demos that bring their own sky and ground sit in the bare `open` scene; the rest sit in the stage room.
      next.set("scene", ["outdoor_sky", "materials_grid", "lit_room", "modern_room", "building_asset", "architecture_gallery", "church_sanctuary", "main_street"].includes(sceneSelect.value) ? "open" : "stage");
      next.set("model", sceneSelect.value);
      next.set("eval", "1");
      next.set("docname", `spatial-lab-${sceneSelect.value}`);
      if (["pegasus_studio", "angel_studio", "building_asset", "architecture_gallery", "model_y", "anime_girl", "eiffel_tower"].includes(sceneSelect.value)) next.set("frame", "1");
    } else {
      next.set("scene", sceneSelect.value);
      next.delete("model");
    }
    next.set("viewer", "1");
    location.search = next.toString();
  });
  const fovInput = hud.querySelector("#v-fov");
  const fovText = hud.querySelector("#v-fov-text");
  const stereoBox = hud.querySelector("#v-stereo");
  const lensSelect = hud.querySelector("#v-lens");
  const orbitBox = hud.querySelector("#v-orbit");
  orbitBox.addEventListener("change", () => { state.orbitMode = orbitBox.checked; });
  hud.querySelector("#v-frame").addEventListener("click", frame);
  function syncControls() {
    orbitBox.checked = state.orbitMode;
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
  const shadows = hud.querySelector("#v-shadows");
  shadows.checked = params.get("shadows") !== "off";
  shadows.addEventListener("change", () => api.set_shadows(shadows.checked));
  const specularAa = hud.querySelector("#v-specular-aa");
  specularAa.checked = params.get("specular-aa") === "on";
  api.set_specular_aa(specularAa.checked);
  specularAa.addEventListener("change", () => api.set_specular_aa(specularAa.checked));
  const ssao = hud.querySelector("#v-ssao");
  // Contact shadows are on unless asked off: they are most of what makes surfaces sit in a scene instead of floating on it.
  ssao.checked = params.get("ssao") !== "off";
  api.set_ssao(ssao.checked);
  ssao.addEventListener("change", () => api.set_ssao(ssao.checked));
  const bloom = hud.querySelector("#v-bloom");
  bloom.checked = params.get("bloom") === "on";
  api.set_bloom(bloom.checked);
  bloom.addEventListener("change", () => api.set_bloom(bloom.checked));
  const autoExposure = hud.querySelector("#v-auto-exposure");
  autoExposure.checked = params.get("autoexposure") === "on";
  api.set_auto_exposure(autoExposure.checked);
  autoExposure.addEventListener("change", () => api.set_auto_exposure(autoExposure.checked));
  quality.addEventListener("change", () => {
    api.set_quality(quality.value);
    api.set_shadows(shadows.checked);
  });
  const grade = hud.querySelector("#v-grade");
  grade.value = params.get("grade") || "neutral";
  const exposure = hud.querySelector("#v-exposure");
  const exposureText = hud.querySelector("#v-exposure-text");
  function syncGradeControls() {
    const current = JSON.parse(api.grading_status());
    exposure.value = String(current.exposure);
    exposureText.textContent = `${current.exposure.toFixed(2)}×`;
  }
  grade.addEventListener("change", () => {
    api.set_grading_preset(grade.value);
    syncGradeControls();
  });
  api.set_grading_preset(grade.value);
  syncGradeControls();
  exposure.addEventListener("input", () => {
    const current = JSON.parse(api.grading_status());
    const tint = current.tint;
    api.set_grading(Number(exposure.value), current.contrast, current.saturation, tint[0], tint[1], tint[2]);
    syncGradeControls();
  });
  document.addEventListener("pointerlockchange", () => {
    document.body.classList.toggle("look", document.pointerLockElement === canvas);
  });

  mountAnimationControls(api);
  // A controller does what the keyboard, mouse and desktop hand do (see gamepad.js); with none connected it costs a comparison a frame.
  const stopGamepad = startGamepad(api);
  apply();
  // `api` is returned so tests can drive the viewer without a real pointer lock.
  return { state, hud, api, stopGamepad };
}
