// A small panel that shows the authoring document and lets you edit it with a few buttons. Every
// button sends the same typed commands any other editor would; the panel owns no state. Opt in with
// ?doc=1 or ?viewer=1. Bottom-right, so it does not cover the viewer's panel on the left.

import { PANEL_CSS, mountPanels } from "./panels/dock.js";

const BOX = { add_node: { op: "solid.box@1", params: { size: [1, 1, 1] }, label: "box" } };
const SPHERE = { add_node: { op: "solid.sphere@1", params: { radius: 0.5 }, label: "sphere" } };

export function startDocPanel({ api, name = "default", evaluation = null, runtime = null }) {
  const panel = document.createElement("div");
  panel.id = "doc-panel";
  panel.innerHTML = `
    <style>
      #doc-panel { position: fixed; right: 12px; bottom: 12px; width: 320px; max-height: calc(100vh - 24px); overflow: auto; padding: 10px 12px; color: #fff;
        font: 12px/1.45 system-ui, sans-serif; background: rgba(20,24,40,.72); border: 1px solid rgba(255,255,255,.22);
        border-radius: 14px; backdrop-filter: blur(14px) saturate(1.4); -webkit-backdrop-filter: blur(14px) saturate(1.4); }
      body.split #doc-panel { opacity: .2; }
      body.split #doc-panel:hover { opacity: 1; }
      #doc-panel button { margin: 4px 4px 0 0; padding: 3px 9px; border-radius: 8px; border: 1px solid rgba(255,255,255,.3);
        background: rgba(255,255,255,.12); color: #fff; font: inherit; cursor: pointer; }
      #doc-panel button:disabled { opacity: .4; cursor: default; }
      #doc-status { margin: 4px 0 2px; }
      #doc-message { min-height: 1.4em; opacity: .85; }
      #doc-message.bad { color: #ffb4a8; }
      #doc-json, #doc-text { max-height: 180px; overflow: auto; font: 11px/1.35 ui-monospace, monospace; white-space: pre-wrap; }
      ${PANEL_CSS}
    </style>
    <b>Document</b>
    <div id="doc-status"></div>
    <div>
      <button id="doc-box">+ box</button><button id="doc-sphere">+ sphere</button>
      <button id="doc-undo">undo</button><button id="doc-redo">redo</button>
      <button id="doc-save">save</button><button id="doc-load">reload</button><button id="doc-export">export .glb</button><button id="doc-texture-preview" disabled>3D texture preview</button>
    </div>
    <div id="doc-character" title="Adds a skinned, animated, toon-shaded character built from a skeleton tree (a few lines of the text language per bone)">
      <select id="char-plan" aria-label="Body plan"><option>biped</option><option>quadruped</option><option>serpent</option></select>
      <label>height <input id="char-size" type="number" min="0.2" max="20" step="0.1" value="1.7" style="width:54px"> m</label>
      <label><input id="char-extras" type="checkbox"> horns and wings</label>
      <button id="char-make">+ character</button>
    </div>
    <div id="doc-message"></div>
    <details id="doc-text-box" hidden open><summary>Text</summary><div id="doc-text"></div></details>
    <details><summary>JSON</summary><div id="doc-json"></div></details>`;
  document.body.append(panel);

  const $ = (id) => panel.querySelector(`#${id}`);
  const say = (text, bad = false) => {
    $("doc-message").textContent = text;
    $("doc-message").classList.toggle("bad", bad);
  };
  let evaluationNotice = null;
  const describe = (diagnostics) =>
    diagnostics.map((d) => `${d.code}: ${d.message}`).join(" | ");
  const render = () => {
    const s = api.state();
    $("doc-status").textContent =
      `rev ${s.revision} · ${s.nodes} node${s.nodes === 1 ? "" : "s"}` +
      (s.saved_generation == null ? " · unsaved" : ` · saved #${s.saved_generation}`) +
      (s.read_only ? " · READ-ONLY" : "");
    $("doc-undo").disabled = !s.can_undo || s.read_only;
    $("doc-redo").disabled = !s.can_redo || s.read_only;
    $("doc-json").textContent = JSON.stringify(s.document, null, 1);
    if (api.text) {
      // Keep the Text panel canonical; model-facing text uses the compact printer.
      $("doc-text-box").hidden = false;
      $("doc-text").textContent = api.text().text ?? "";
    }
    if (evaluation) {
      const e = evaluation.state;
      $("doc-texture-preview").disabled = !evaluation.parts.some(part => part.material?.texture_program);
      $("doc-status").textContent += ` · ${e.refining ? 'refining' : e.worker}`;
      if (e.lastFailure && e.failureCurrent) {
        evaluationNotice = `${e.lastFailure.code}: ${e.lastFailure.message}`;
        say(evaluationNotice, true);
      } else if (evaluationNotice) {
        if ($("doc-message").textContent === evaluationNotice) say("");
        evaluationNotice = null;
      }
    }
    if (s.issues.length) say(describe(s.issues), true);
    panels?.render();
    return s;
  };
  const report = (outcome) => {
    if (!outcome) say("");
    else if (outcome.committed) say(`committed revision ${outcome.committed.revision}`);
    else if (outcome.rejected) say(describe(outcome.rejected.diagnostics), true);
    else if (outcome.conflict) say("the document changed underneath: reload and retry", true);
    else if (outcome.already_applied) say("that edit was already applied");
    render();
  };
  // Outliner, inspector, history and problems; they need the engine's selection (opt in with a runtime).
  const panels = runtime?.selection_set && api.operators
    ? mountPanels({ root: panel, api, runtime, evaluation, onOutcome: report })
    : null;

  $("doc-box").addEventListener("click", () => report(api.send([BOX], "add box")));
  $("doc-sphere").addEventListener("click", () => report(api.send([SPHERE], "add sphere")));
  // The character maker: the engine writes the whole skeleton, body, skin and clips as text, which goes in like any edit.
  let made = 0;
  const addCharacter = (plan, size, extras) => {
    if (!runtime?.creature_make) { say("this build has no character maker", true); return; }
    const request = {
      plan,
      name: `${plan}${Date.now().toString(36).slice(-4)}`,
      at: [made * 2.4, 0, 0],
      sliders: { size, horns: extras && plan === "quadruped", wings: extras && plan === "quadruped" },
    };
    const text = JSON.parse(runtime.creature_make(JSON.stringify(request)));
    if (text.error) { say(text.error, true); return; }
    const read = JSON.parse(runtime.doc_parse_text(text.text));
    if (!read.commands) { say(describe(read.diagnostics ?? []), true); return; }
    made += 1;
    report(api.send(read.commands, `add ${plan}`));
  };
  $("char-make").addEventListener("click", () => addCharacter($("char-plan").value, Number($("char-size").value), $("char-extras").checked));
  // ?characters=biped:1.7,quadruped:2.4:wings,serpent:1 opens with those characters already in the document.
  const wanted = new URLSearchParams(location.search).get("characters");
  if (wanted) {
    for (const item of wanted.split(",")) {
      const [plan, size, extras] = item.split(":");
      addCharacter(plan, Number(size) || 1.7, extras === "wings");
    }
  }
  $("doc-undo").addEventListener("click", () => { api.undo(); say("undone"); render(); });
  $("doc-redo").addEventListener("click", () => { api.redo(); say("redone"); render(); });
  $("doc-save").addEventListener("click", () => {
    const result = api.save();
    if (result.error) say(`${result.error.code}: ${result.error.message}`, true);
    else say(`saved #${result.generation}`);
    render();
  });
  $("doc-load").addEventListener("click", () => { api.open(name); say("reloaded from storage"); render(); });
  // The evaluated model as a glTF file for Blender, game engines and web viewers (needs the evaluation pipeline).
  $("doc-export").addEventListener("click", async () => {
    say("preparing glTF export…");
    try {
      const glb = await api.exportGlb?.();
      if (!glb) { say("nothing to export yet", true); return; }
      const link = document.createElement("a");
      link.href = URL.createObjectURL(new Blob([glb], { type: "model/gltf-binary" }));
      link.download = `${name || "model"}.glb`;
      link.click();
      setTimeout(() => URL.revokeObjectURL(link.href), 1000);
      say(`exported ${(glb.byteLength / 1024).toFixed(1)} KB`);
    } catch (error) {
      say(error instanceof Error ? error.message : String(error), true);
    }
  });
  $("doc-texture-preview").addEventListener("click", async () => {
    const part = evaluation?.parts.find(item => item.material?.texture_program);
    if (!part) { say("add a solid.texture program to preview", true); return; }
    say("baking 3D material preview…");
    try {
      const { openTexturePreview } = await import("./texture-preview.js");
      await openTexturePreview(part);
      say("");
    } catch (error) {
      say(error instanceof Error ? error.message : String(error), true);
    }
  });

  render();
  return { render, panel, selection: panels?.selection ?? null };
}
