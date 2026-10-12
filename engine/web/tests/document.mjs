// The authoring document through the page bridge (cases G1b-10 to G1b-12 and the page side of the
// storage cases): batches sent from the page change the engine's document, bad input is a rejection
// and never an error, saves survive reloads and two tabs, and a file the build cannot edit opens
// read-only and is never overwritten. The engine keeps rendering throughout.
import { chromium } from "playwright";
import { driverAdvisory, serve } from "./support.mjs";

const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const context = await browser.newContext({ viewport: { width: 640, height: 360 } });
  const logs = [];
  const watch = (page) => {
    page.on("console", (m) => {
      if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
    });
    page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
  };
  const load = async (name) => {
    const page = await context.newPage();
    watch(page);
    await page.goto(`http://127.0.0.1:${port}/?scene=xr&doc=1&docname=${name}`);
    await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
    check(!(await page.evaluate(() => window.__ve.error)), `start failed: ${await page.evaluate(() => window.__ve.error)}`);
    return page;
  };
  const doc = (page, fn, ...args) => page.evaluate(([f, a]) => window.__ve.doc[f](...a), [fn, args]);
  const box = (label) => ({ add_node: { op: "solid.box@1", params: { size: [1, 2, 3] }, label } });

  // G1b-10: a batch sent from the page changes the engine's document, and the panel shows it.
  const a = await load("t10");
  const empty = await doc(a, "state");
  check(empty.revision === 0 && empty.nodes === 0 && !empty.read_only, `a new document is not empty: ${JSON.stringify(empty)}`);
  const sent = await doc(a, "send", [box("first")], "add a box");
  check(sent.committed?.revision === 1, `the batch did not commit: ${JSON.stringify(sent)}`);
  const one = await doc(a, "state");
  check(one.nodes === 1 && same(one.document.nodes[0].params.size, [1, 2, 3]), "the node or its parameters are wrong");
  check(one.document.nodes[0].label === "first", "the label was lost");
  await a.click("#doc-sphere");
  const status = await a.textContent("#doc-status");
  check(/rev 2/.test(status) && /2 nodes/.test(status), `the panel does not show the new revision: ${status}`);
  const frames = await a.evaluate(() => window.__ve.read().frames);
  await a.waitForFunction((n) => window.__ve.read().frames > n + 5, frames, { timeout: 30000 });

  // G1b-11: malformed or invalid input is a rejection, never an exception or a console error.
  const syntax = await doc(a, "apply", "{");
  check(syntax.rejected?.diagnostics[0].code === "parse.syntax", `malformed JSON: ${JSON.stringify(syntax)}`);
  const state = await doc(a, "state");
  const wrongDoc = await doc(a, "apply", { doc: "d_00000000", base_revision: 0, transaction: "t_x1", commands: [box("x")] });
  check(wrongDoc.rejected?.diagnostics[0].code === "revision.document", `wrong document: ${JSON.stringify(wrongDoc)}`);
  const unknown = await doc(a, "send", [{ add_node: { op: "solid.revolv@1", params: {} } }]);
  check(unknown.rejected?.diagnostics[0].code === "parse.unknown_op", "an unknown operator was not rejected");
  check(/solid\.revolve@1/.test(unknown.rejected?.diagnostics[0].suggestion ?? ""), "no did-you-mean suggestion");
  const extra = await doc(a, "apply", `{"doc":"${state.id}","base_revision":2,"transaction":"t_x2","commands":[{"run_code":{}}]}`);
  check(extra.rejected?.diagnostics[0].code === "parse.syntax", "an unknown command was not rejected");
  check(same((await doc(a, "state")).document, state.document), "a rejected batch changed the document");
  const stale = await doc(a, "apply", { doc: state.id, base_revision: 0, transaction: "t_stale", commands: [box("stale")] });
  check(stale.conflict?.current_revision === state.revision, `a stale batch did not conflict: ${JSON.stringify(stale)}`);

  // Undo, redo and the engine's operator list.
  const undone = await doc(a, "undo");
  check(undone?.revision === state.revision + 1 && (await doc(a, "state")).nodes === 1, "undo did not remove the last node");
  await doc(a, "redo");
  check((await doc(a, "state")).nodes === 2, "redo did not restore it");
  const ops = await doc(a, "operators");
  const ready = new Set(ops.filter((o) => o.readiness === "ready").map((o) => o.id));
  check(ready.has("solid.revolve@1") && ready.has("check.min_wall@1") && ready.has("profile.offset@1") && ops.length === ready.size,
    "operator list is wrong: every shipped operator passes its conformance cases and is ready");

  // Save, then a fresh page (a reload) gets the identical document back.
  const saved = await doc(a, "save");
  check(saved.generation === 1, `save: ${JSON.stringify(saved)}`);
  const before = await doc(a, "state");
  const reloaded = await load("t10");
  const after = await doc(reloaded, "state");
  check(same(after.document, before.document) && after.saved_generation === 1, "the reloaded document differs from the saved one");
  const addAgain = await doc(reloaded, "send", [box("after reload")]);
  check(addAgain.committed?.revision === before.revision + 1, "editing after a reload failed");
  const ids = (await doc(reloaded, "state")).document.nodes.map((n) => n.id);
  check(new Set(ids).size === ids.length, "a reloaded document issued a duplicate ID");

  // G1b-12: two tabs share the storage. Each save is whole; a later load is one tab's document,
  // never a mixture. (Edits in two tabs are not merged: the last save wins.)
  const tabA = await load("t12");
  const tabB = await load("t12");
  await doc(tabA, "send", [box("from A")]);
  check((await doc(tabA, "save")).generation === 1, "tab A first save");
  const openedB = await doc(tabB, "open", "t12");
  check(openedB.nodes === 1 && openedB.document.nodes[0].label === "from A", "tab B does not see tab A's save");
  await doc(tabB, "send", [box("from B")]);
  check((await doc(tabB, "save")).generation === 2, "tab B save is generation 2");
  await doc(tabA, "send", [box("A again")]);
  check((await doc(tabA, "save")).generation === 3, "tab A (stale) save is generation 3, not a merge");
  const finalA = (await doc(tabA, "state")).document;
  const third = await load("t12");
  const seen = await doc(third, "state");
  check(same(seen.document, finalA) && seen.saved_generation === 3, "a later load is not exactly the last saved document");

  // A file this build cannot edit opens read-only and is never touched.
  const ro = await load("t_ro");
  await doc(ro, "send", [box("keep")]);
  await doc(ro, "save");
  const key = "ve.doc.t_ro.a";
  const raw = await ro.evaluate((k) => localStorage.getItem(k), key);
  const newer = raw.replace('"schema":1,', '"schema":99,');
  check(newer !== raw, "could not build a newer-version file for the test");
  await ro.evaluate(([k, v]) => localStorage.setItem(k, v), [key, newer]);
  const opened = await doc(ro, "open", "t_ro");
  check(opened.read_only && opened.issues[0]?.code === "store.newer_version", `newer file: ${JSON.stringify(opened.issues)}`);
  const refused = await doc(ro, "send", [box("no")]);
  check(refused.rejected?.diagnostics[0].code === "store.read_only", "a read-only document accepted an edit");
  check((await doc(ro, "save")).error?.code === "store.read_only", "a read-only document was saved");
  check((await ro.evaluate((k) => localStorage.getItem(k), key)) === newer, "the newer file was changed");
  await ro.reload();
  await ro.waitForFunction(() => window.__ve?.ready, null, { timeout: 90000 });
  check(/READ-ONLY/.test(await ro.textContent("#doc-status")), "the panel does not say read-only");

  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`document test FAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("document test passed");
