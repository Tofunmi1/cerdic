// The editing panels of the viewer page: outliner, inspector, history and problems, stacked above
// the document's text and JSON. They own no document state: every edit is a typed command sent
// through the page's document API, and selection is view state shared with the viewport.
import { createHistory, createProblems } from "./history.js";
import { createInspector } from "./inspector.js";
import { createOutliner } from "./outliner.js";
import { createSelection } from "./selection.js";

export const PANEL_CSS = `
  .dock details { margin: 6px 0; border-top: 1px solid rgba(255,255,255,.14); padding-top: 4px; }
  .dock summary { cursor: pointer; font-weight: 600; user-select: none; }
  .dock .body { max-height: 190px; overflow: auto; margin-top: 3px; }
  .dock .empty { opacity: .6; padding: 4px 2px; }
  .dock .orow { display: flex; align-items: center; gap: 5px; padding: 2px 4px; border-radius: 6px; cursor: pointer; white-space: nowrap; }
  .dock .orow:hover { background: rgba(255,255,255,.1); }
  .dock .orow.selected { background: rgba(120,170,255,.35); }
  .dock .orow.muted .oname { opacity: .5; text-decoration: line-through; }
  .dock .caret { width: 10px; text-align: center; opacity: .8; }
  .dock .oop { margin-left: auto; opacity: .5; font: 10px ui-monospace, monospace; }
  .dock .obadge { padding: 0 5px; border-radius: 8px; background: rgba(255,255,255,.18); font-size: 10px; }
  .dock .obadge.part { background: rgba(120,200,140,.4); }
  .dock .odot { color: #ff8e7e; }
  .dock .orow.ogroup { margin-top: 6px; font-weight: 600; border-top: 1px solid rgba(255,255,255,.12); border-radius: 0 0 6px 6px; }
  .dock .orow.unused .oname { opacity: .5; font-style: italic; }
  .dock .obadge.warn { background: rgba(255,170,90,.5); }
  .dock .banner { margin: 4px 0; padding: 4px 6px; border-radius: 6px; background: rgba(255,255,255,.1); font-size: 11px; }
  .dock .irow.overridden .cap { color: #9ccbff; font-weight: 600; }
  .dock .ohead { margin: 6px 0 2px; opacity: .6; font-size: 10px; text-transform: uppercase; letter-spacing: .06em; }
  .dock .title { width: 100%; box-sizing: border-box; font: 600 13px system-ui, sans-serif; background: transparent; color: #fff; border: 1px solid transparent; border-radius: 6px; padding: 2px 4px; }
  .dock .title:hover, .dock .title:focus { border-color: rgba(255,255,255,.3); }
  .dock .ophelp { opacity: .6; font-size: 10px; margin: 0 4px 4px; }
  .dock .irow { display: flex; align-items: center; gap: 6px; margin: 3px 0; flex-wrap: wrap; }
  .dock .cap { width: 78px; flex: none; opacity: .85; overflow: hidden; text-overflow: ellipsis; }
  .dock .cap.scrub { cursor: ew-resize; }
  .dock .field { display: inline-flex; align-items: center; gap: 2px; }
  .dock .vec { display: inline-flex; gap: 4px; }
  .dock input.num { width: 62px; }
  .dock input, .dock select, .dock textarea { background: rgba(0,0,0,.28); color: #fff; border: 1px solid rgba(255,255,255,.25); border-radius: 6px; font: 11px system-ui, sans-serif; padding: 2px 4px; }
  .dock textarea.points { width: 100%; box-sizing: border-box; font: 10px ui-monospace, monospace; }
  .dock textarea.invalid { border-color: #ff8e7e; }
  .dock .unit { opacity: .6; font-size: 10px; }
  .dock .links { display: inline-flex; gap: 4px; flex-wrap: wrap; }
  .dock .none { opacity: .5; }
  .dock button.link { margin: 0; padding: 0 6px; }
  .dock .issue { color: #ffb4a8; font-size: 11px; margin: 0 0 2px 84px; }
  .dock button.socket { width:100%;text-align:left;white-space:normal;font-size:10px; }
  .dock button.socket.selected { background:rgba(100,190,255,.35);border-color:rgba(150,210,255,.8); }
  .dock .danger { margin-left: auto; }
  .dock .hrow, .dock .prow { display: block; width: 100%; margin: 1px 0; text-align: left; background: transparent; border-color: transparent; }
  .dock .hrow:hover, .dock .prow:hover { background: rgba(255,255,255,.1); }
  .dock .hrow.undone { opacity: .45; }
  .dock .hnow { margin: 2px 0; text-align: center; font-size: 10px; opacity: .6; border-top: 1px dashed rgba(255,255,255,.3); }
  .dock .prow { color: #ffb4a8; white-space: normal; }
`;

/// Puts the panels into `root` (the document panel) and returns what the page calls after each change.
export function mountPanels({ root, api, runtime, evaluation, onOutcome }) {
  const dock = globalThis.document.createElement("div");
  dock.className = "dock";
  dock.innerHTML = `
    <details id="dock-problems" hidden><summary>Problems</summary><div class="body"></div></details>
    <details open><summary>Outliner</summary><div class="body" id="dock-outliner" role="tree"></div></details>
    <details open><summary>Inspector</summary><div class="body" id="dock-inspector" style="max-height:340px"></div></details>
    <details><summary>History</summary><div class="body" id="dock-history"></div></details>`;
  root.querySelector("#doc-message").after(dock);
  const part = (id) => dock.querySelector(`#${id}`);

  // The document as the panels show it: the staged edit while it is evaluating, else the committed one.
  const document_ = () => {
    const state = api.state();
    return state.pending_document ?? state.document;
  };
  // Everything the engine has said that is still true: document issues, the last evaluation failure,
  // and (in the inspector) the reasons the last edit was refused.
  const problems = () => {
    const failure = evaluation?.state.lastFailure;
    // The document and the evaluation can report the same failure; list it once, with the node if either names one.
    const unique = new Map();
    for (const problem of [...(api.state().issues ?? []), ...(failure ? [failure] : [])]) {
      const key = `${problem.code}|${problem.message}`;
      if (!unique.get(key)?.node) unique.set(key, problem);
    }
    return [...unique.values()];
  };

  const selection = createSelection({ runtime, state: () => ({ document: document_() }) });
  const outliner = createOutliner({ root: part("dock-outliner"), selection, document: document_, problems });
  const inspector = createInspector({ root: part("dock-inspector"), api, selection, document: document_, problems, onOutcome, evaluation });
  const history = createHistory({ root: part("dock-history"), api, onStep: () => onOutcome(null) });
  const issues = createProblems({
    root: dock.querySelector("#dock-problems .body"), details: part("dock-problems"), selection, list: problems,
  });
  const panels = [outliner, inspector, history, issues];
  const render = () => {
    selection.refresh();
    for (const panel of panels) panel.render();
  };
  selection.subscribe(render);
  return { render, selection, dispose: selection.dispose };
}
