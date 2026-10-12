// A small panel that shows the authoring document and lets you edit it with a few buttons. Every
// button sends the same typed commands any other editor would; the panel owns no state. Opt in with
// ?doc=1 or ?viewer=1. Bottom-left, so it does not cover the viewer's panel.

const BOX = { add_node: { op: "solid.box@1", params: { size: [1, 1, 1] }, label: "box" } };
const SPHERE = { add_node: { op: "solid.sphere@1", params: { radius: 0.5 }, label: "sphere" } };

export function startDocPanel({ api, name = "default" }) {
  const panel = document.createElement("div");
  panel.id = "doc-panel";
  panel.innerHTML = `
    <style>
      #doc-panel { position: fixed; left: 12px; bottom: 12px; width: 300px; padding: 10px 12px; color: #fff;
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
      #doc-json { max-height: 180px; overflow: auto; font: 11px/1.35 ui-monospace, monospace; white-space: pre-wrap; }
    </style>
    <b>Document</b>
    <div id="doc-status"></div>
    <div>
      <button id="doc-box">+ box</button><button id="doc-sphere">+ sphere</button>
      <button id="doc-undo">undo</button><button id="doc-redo">redo</button>
      <button id="doc-save">save</button><button id="doc-load">reload</button>
    </div>
    <div id="doc-message"></div>
    <details><summary>JSON</summary><div id="doc-json"></div></details>`;
  document.body.append(panel);

  const $ = (id) => panel.querySelector(`#${id}`);
  const say = (text, bad = false) => {
    $("doc-message").textContent = text;
    $("doc-message").classList.toggle("bad", bad);
  };
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
    if (s.issues.length) say(describe(s.issues), true);
    return s;
  };
  const report = (outcome) => {
    if (outcome.committed) say(`committed revision ${outcome.committed.revision}`);
    else if (outcome.rejected) say(describe(outcome.rejected.diagnostics), true);
    else if (outcome.conflict) say("the document changed underneath: reload and retry", true);
    else if (outcome.already_applied) say("that edit was already applied");
    render();
  };

  $("doc-box").addEventListener("click", () => report(api.send([BOX], "add box")));
  $("doc-sphere").addEventListener("click", () => report(api.send([SPHERE], "add sphere")));
  $("doc-undo").addEventListener("click", () => { api.undo(); say("undone"); render(); });
  $("doc-redo").addEventListener("click", () => { api.redo(); say("redone"); render(); });
  $("doc-save").addEventListener("click", () => {
    const result = api.save();
    if (result.error) say(`${result.error.code}: ${result.error.message}`, true);
    else say(`saved #${result.generation}`);
    render();
  });
  $("doc-load").addEventListener("click", () => { api.open(name); say("reloaded from storage"); render(); });

  render();
  return { render, panel };
}
