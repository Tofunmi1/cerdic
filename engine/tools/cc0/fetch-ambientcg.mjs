#!/usr/bin/env node
// Fetches every ambientCG material (CC0 1.0, https://ambientcg.com) at one resolution into a library folder, one folder per material, with a provenance
// record each (ADR 0071: imported content is admitted only with source, licence, hash and date). Resumable: a material whose meta.json exists is skipped.
//
//   node tools/cc0/fetch-ambientcg.mjs [--out DIR] [--res 1K-JPG] [--jobs 3] [--limit N] [--only Id,Id]
//
// Keeps the maps an engine uses (Color, Roughness, NormalGL, AmbientOcclusion, Displacement, Metalness, Opacity, Emission) and the preview picture; drops the
// DirectX normal map, the Blender, MaterialX and Godot files. Writes DIR/index.json (one line per material) at the end.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
const out = args.get("out") ?? join(homedir(), "Data", "cc0", "ambientcg");
const res = args.get("res") ?? "1K-JPG";
const jobs = Number(args.get("jobs") ?? 3);
const limit = Number(args.get("limit") ?? Infinity);
const only = args.get("only")?.split(",");
const KEEP = ["Color", "Roughness", "NormalGL", "AmbientOcclusion", "Displacement", "Metalness", "Opacity", "Emission"];
const API = "https://ambientcg.com/api/v2/full_json";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function json(url, tries = 4) {
  for (let t = 1; ; t++) {
    try {
      const r = await fetch(url);
      if (!r.ok) throw new Error(`${r.status} ${url}`);
      return await r.json();
    } catch (e) {
      if (t >= tries) throw e;
      await sleep(1500 * t);
    }
  }
}

async function listMaterials() {
  const found = [];
  for (let offset = 0; ; offset += 100) {
    const page = await json(`${API}?type=Material&sort=Alphabet&limit=100&offset=${offset}&include=downloadData`);
    found.push(...page.foundAssets);
    if (offset + 100 >= page.numberOfResults || found.length >= limit) break;
    await sleep(300);
  }
  return found;
}

async function download(url, file, tries = 4) {
  for (let t = 1; ; t++) {
    try {
      const r = await fetch(url, { redirect: "follow" });
      if (!r.ok) throw new Error(`${r.status} ${url}`);
      const bytes = Buffer.from(await r.arrayBuffer());
      await writeFile(file, bytes);
      return bytes;
    } catch (e) {
      if (t >= tries) throw e;
      await sleep(2000 * t);
    }
  }
}

async function fetchOne(asset) {
  const id = asset.assetId;
  const dir = join(out, id);
  if (await stat(join(dir, "meta.json")).then(() => true, () => false)) return "skip";
  const zip = asset.downloadFolders?.default?.downloadFiletypeCategories?.zip?.downloads?.find((d) => d.attribute === res);
  if (!zip) return `no ${res}`;
  const tmp = join(out, `.${id}.tmp`);
  await rm(tmp, { recursive: true, force: true });
  await mkdir(tmp, { recursive: true });
  const bytes = await download(zip.downloadLink, join(tmp, "src.zip"));
  if (bytes.length !== zip.size) throw new Error(`${id}: got ${bytes.length} bytes, expected ${zip.size}`);
  await run("unzip", ["-q", "-o", join(tmp, "src.zip"), "-d", join(tmp, "x")]);
  const files = await readdir(join(tmp, "x"));
  const maps = [];
  await mkdir(join(tmp, "maps"), { recursive: true });
  for (const kind of KEEP) {
    const name = files.find((f) => f === `${id}_${res}_${kind}.${res.endsWith("PNG") ? "png" : "jpg"}`);
    if (name) {
      await rename(join(tmp, "x", name), join(tmp, "maps", `${id}_${kind}${name.slice(name.lastIndexOf("."))}`));
      maps.push(kind);
    }
  }
  const preview = files.find((f) => f === `${id}.png`);
  if (preview) await rename(join(tmp, "x", preview), join(tmp, "maps", `${id}_preview.png`));
  if (!maps.includes("Color")) throw new Error(`${id}: no colour map in ${files.join(",")}`);
  const meta = {
    id,
    name: asset.displayName,
    category: asset.displayCategory,
    tags: asset.tags,
    sizeMetres: [asset.dimensionX, asset.dimensionY],
    creationMethod: asset.creationMethod,
    maps,
    resolution: res,
    source: asset.shortLink,
    licence: "CC0-1.0",
    attribution: "not required",
    sha256: createHash("sha256").update(bytes).digest("hex"),
    zipBytes: bytes.length,
    fetched: new Date().toISOString().slice(0, 10),
  };
  await writeFile(join(tmp, "maps", "meta.json"), JSON.stringify(meta, null, 1));
  await rm(dir, { recursive: true, force: true });
  await rename(join(tmp, "maps"), dir);
  await rm(tmp, { recursive: true, force: true });
  return "ok";
}

async function writeIndex() {
  const lines = [];
  for (const id of (await readdir(out)).filter((n) => !n.startsWith(".")).sort()) {
    try {
      const m = JSON.parse(await readFile(join(out, id, "meta.json"), "utf8"));
      lines.push(JSON.stringify({ id: m.id, name: m.name, category: m.category, tags: m.tags, sizeMetres: m.sizeMetres, maps: m.maps }));
    } catch {
      // not a material folder
    }
  }
  await writeFile(join(out, "index.jsonl"), lines.join("\n") + "\n");
  return lines.length;
}

await mkdir(out, { recursive: true });
let assets = await listMaterials();
if (only) assets = assets.filter((a) => only.includes(a.assetId));
assets = assets.slice(0, limit);
console.log(`${assets.length} materials, ${res}, ${jobs} at a time, into ${out}`);
let next = 0, done = 0, skipped = 0;
const failed = [];
await Promise.all(
  Array.from({ length: jobs }, async () => {
    while (next < assets.length) {
      const asset = assets[next++];
      try {
        const r = await fetchOne(asset);
        if (r === "skip") skipped++;
        else if (r !== "ok") failed.push(`${asset.assetId}: ${r}`);
        else await sleep(150);
      } catch (e) {
        failed.push(`${asset.assetId}: ${e.message}`);
      }
      if (++done % 25 === 0) console.log(`${done}/${assets.length} (${skipped} already there, ${failed.length} failed)`);
    }
  }),
);
const indexed = await writeIndex();
console.log(`done: ${indexed} materials in the index, ${failed.length} failed`);
if (failed.length) console.log(failed.join("\n"));
