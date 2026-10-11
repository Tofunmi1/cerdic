// Shared helpers for the browser tests: a static server for web/dist, pixel utilities, golden-image
// comparison and the list of console messages that describe the test environment rather than our code.
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const here = path.dirname(fileURLToPath(import.meta.url));
export const dist = path.join(here, "..", "dist");
/** sRGB of the clear colour set in ve-runtime. */
export const CLEAR = [26, 51, 102];
export const GOLDEN = { channelTolerance: 10, maxBadFraction: 0.002 };
/** Environment noise that is ignored; everything else the page logs is a failure. */
export const driverAdvisory = /GL Driver Message \(OpenGL, Performance,|only supports software rendering/;

const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };

/** Serves web/dist on an ephemeral IPv4 loopback port. */
export async function serve() {
  const server = http.createServer((req, res) => {
    const requested = req.url.split("?")[0];
    const file = path.join(dist, requested === "/" ? "index.html" : requested);
    let body;
    try {
      body = fs.readFileSync(file);
    } catch {
      res.writeHead(404);
      res.end();
      return;
    }
    res.writeHead(200, { "content-type": types[path.extname(file)] ?? "application/octet-stream" });
    res.end(body);
  });
  // Bind IPv4 loopback explicitly: the default wildcard bind made Chrome's connection stall at random.
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return { server, port: server.address().port };
}

export const pixelAt = (png, x, y) => {
  const i = (png.width * y + x) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2]];
};

/** Counts pixels in a rectangle whose colour differs from the clear colour by more than 20. */
export function drawnPixels(png, x0, x1) {
  let drawn = 0;
  for (let y = 0; y < png.height; y++) {
    for (let x = x0; x < x1; x++) {
      const p = pixelAt(png, x, y);
      if (Math.max(...p.map((c, i) => Math.abs(c - CLEAR[i]))) > 20) drawn++;
    }
  }
  return drawn;
}

/** Fraction (0..1) of pixels that differ by more than `tolerance` in any channel. */
export function differingFraction(a, b, tolerance = GOLDEN.channelTolerance) {
  if (a.width !== b.width || a.height !== b.height) return 1;
  let bad = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    const worst = Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2]));
    if (worst > tolerance) bad++;
  }
  return bad / (a.width * a.height);
}
