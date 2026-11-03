// node texgallery.mjs <gens.json> <outPrefix> [perSheet=48] [cols=8]
// One contact sheet per perSheet generator nodes: each is `n = NODE(); albedo = n[.k]` on a thin plate, in a grid, labelled by a second canvas pass.
import { chromium } from "/Users/mac/Work/victoria-vr-hack/engine/web/node_modules/playwright/index.mjs";
import { readFileSync, writeFileSync } from "node:fs";
const [gensPath, prefix, perSheetArg, colsArg] = process.argv.slice(2);
const gens = JSON.parse(readFileSync(gensPath, "utf8")).generators.filter((g) => !/^sd|^surface_|^meshmap|^pixels/.test(g.name)).filter((g) => (process.env.FILTERS ? g.inputs > 0 : true));
const per = Number(perSheetArg ?? 48), cols = Number(colsArg ?? 8);
const W = 1600, H = 1000, gap = 1.15;
const pickOut = (g) => (g.outs.find((o) => o.type === "rgb" || o.type === "rgba") ?? g.outs[0]).k;
const b = await chromium.launch({ channel: "chromium", args: ["--enable-unsafe-webgpu", "--enable-features=Metal,WebGPU", "--ignore-gpu-blocklist"] });
const p = await b.newPage({ viewport: { width: W, height: H } });
const failures = [];
p.on("console", (m) => { if (/texture bake|bake_failed/i.test(m.text())) failures.push(m.text().slice(0, 200)); });
let sheetNo = 0;
for (let s = 0; s < gens.length; s += per) {
  const chunk = gens.slice(s, s + per);
  const rows = Math.ceil(chunk.length / cols); const lift = (rows * gap) / 2 + 0.8;
  const lines = ["sky = scene.sky@1(sun_elevation=70, sun_azimuth=0, sun_intensity=3, turbidity=2, clouds=false)", "output sky = sky", "env = scene.environment@1(intensity=1.2, gi=false)", "output env = env"];
  const cells = [];
  chunk.forEach((g, i) => {
    const x = ((i % cols) - (cols - 1) / 2) * gap, y = ((rows - 1) / 2 - Math.floor(i / cols)) * gap;
    const k = pickOut(g);
    const prog = `${process.env.FILTERS ? "src = bricks(); n = " + g.name + "(src)" : "n = " + g.name + "()"}; albedo = ${k ? `n.${k}` : "n"}`;
    lines.push(`p${i} = solid.box@1(size=[1, 1, 0.02])`, `m${i} = solid.material@1(p${i}, color=[0.5, 0.5, 0.5], roughness=0.9)`,
      `t${i} = solid.texture@1(m${i}, program="${prog}", tile=1, size="256")`, `x${i} = xform.move@1(t${i}, by=[${x.toFixed(3)}, ${(y + lift).toFixed(3)}, 0])`, `output ${g.name} = x${i}`);
    cells.push({ name: g.name, x, y });
  });
  await p.goto(`http://127.0.0.1:8801/?viewer=1&eval=1&scene=open&docname=texgallery-${Date.now()}`);
  await p.waitForFunction(() => window.__ve?.ready && window.__ve.evaluation?.state.worker === "idle", null, { timeout: 120000 });
  await p.evaluate(async () => { window.__rt = await import("./ve_runtime.js"); });
  const staged = await p.evaluate((t) => window.__ve.doc.sendText(t, "texture gallery"), lines.join("\n"));
  if (!staged.pending) { console.log("not staged", JSON.stringify(staged).slice(0, 600)); }
  await p.waitForFunction(() => JSON.parse(window.__rt.eval_projection_status()).committed?.quality === "final" || window.__ve.evaluation.state.failureCurrent, null, { timeout: 300000 });
  const fail = await p.evaluate(() => window.__ve.evaluation.state.lastFailure);
  if (fail && (await p.evaluate(() => window.__ve.evaluation.state.failureCurrent))) { console.log("failure", JSON.stringify(fail).slice(0, 800)); }
  await p.waitForTimeout(4000);
  await p.keyboard.press("h");
  const fov = 40, span = Math.max(cols, rows) * gap;
  const dist = (rows * gap * 0.5 + 0.2) / Math.tan((fov / 2) * Math.PI / 180);
  await p.evaluate(([f, d, l]) => { window.__rt.viewer_configure(true, f, false, 8); window.__rt.viewer_look_at(0, l, d, 0, l, 0); document.querySelectorAll("body > *:not(canvas)").forEach((e) => { e.style.display = "none"; }); }, [fov, dist, lift]);
  await p.waitForTimeout(5000);
  const shot = await p.screenshot();
  const warn = await p.evaluate(() => window.__ve.evaluation.state.warnings.filter((w) => /texture/.test(w.code)).map((w) => w.code + ": " + w.message.slice(0, 120)));
  const f = (H / 2) / Math.tan((fov / 2) * Math.PI / 180);
  const labels = cells.map((c) => ({ name: c.name, sx: W / 2 + f * (c.x / dist), sy: H / 2 - f * (c.y / dist) + (f * 0.5 * gap) / dist }));
  const out = await p.evaluate(async ([b64, labels]) => {
    const img = new Image(); img.src = "data:image/png;base64," + b64; await img.decode();
    const c = document.createElement("canvas"); c.width = img.width; c.height = img.height; const g = c.getContext("2d");
    g.drawImage(img, 0, 0); g.font = "bold 13px monospace"; g.textAlign = "center";
    for (const l of labels) { g.lineWidth = 3; g.strokeStyle = "#000"; g.strokeText(l.name, l.sx, l.sy + 12); g.fillStyle = "#fff"; g.fillText(l.name, l.sx, l.sy + 12); }
    return c.toDataURL("image/png").split(",")[1];
  }, [shot.toString("base64"), labels]);
  writeFileSync(`${prefix}_${sheetNo}.png`, Buffer.from(out, "base64"));
  console.log(`sheet ${sheetNo}: ${chunk.length} nodes, warnings`, JSON.stringify(warn).slice(0, 600));
  sheetNo++;
}
console.log("bake failures in console:", failures.length, failures.slice(0, 5).join(" | "));
await b.close();
