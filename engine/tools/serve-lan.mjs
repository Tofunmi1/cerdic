// LAN test server for trying the engine on a headset. It serves web/dist and accepts the benchmark
// report the page posts when a session ends, saving it under target/reports.
//
// WebXR needs a secure context. Two ways to get one on a Quest:
//   1. USB: `adb reverse tcp:8443 tcp:8443`, then open http://localhost:8443/ in the headset browser
//      (localhost counts as secure, so no certificate is involved). Run this tool with `--http`.
//   2. Wi-Fi: run it as is; it makes a self-signed certificate and prints https URLs for this
//      machine's addresses. The headset browser warns about the certificate; proceed past it.
//
// This tool is for private testing on a local network. It binds to the interfaces you ask for
// (`--host`, default all) and serves only web/dist and a write-only /report endpoint.
//
//   node tools/serve-lan.mjs [--http] [--host 0.0.0.0] [--port 8443]
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import https from "node:https";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const engine = path.join(here, "..");
const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".json": "application/json",
  ".png": "image/png",
  ".css": "text/css",
};
const MAX_REPORT_BYTES = 1_000_000;
/** Reports kept on disk; older ones are deleted as new ones arrive. */
const MAX_REPORTS = 200;

/** IPv4 addresses of this machine that another device on the network could use. */
export function lanAddresses() {
  return Object.values(os.networkInterfaces())
    .flat()
    .filter((entry) => entry && entry.family === "IPv4" && !entry.internal)
    .map((entry) => entry.address);
}

/** Creates (or reuses) a self-signed certificate valid for localhost and this machine's addresses. */
export function ensureCertificate(directory, addresses) {
  const key = path.join(directory, "key.pem");
  const cert = path.join(directory, "cert.pem");
  const names = ["DNS:localhost", "IP:127.0.0.1", ...addresses.map((a) => `IP:${a}`)].join(",");
  const stamp = path.join(directory, "names.txt");
  if (fs.existsSync(key) && fs.existsSync(cert) && fs.existsSync(stamp) && fs.readFileSync(stamp, "utf8") === names) {
    return { key: fs.readFileSync(key), cert: fs.readFileSync(cert) };
  }
  fs.mkdirSync(directory, { recursive: true });
  const config = path.join(directory, "openssl.cnf");
  fs.writeFileSync(config, [
    "[req]", "distinguished_name = dn", "x509_extensions = ext", "prompt = no",
    "[dn]", "CN = engine-lan-test",
    "[ext]", `subjectAltName = ${names}`, "basicConstraints = CA:FALSE",
  ].join("\n"));
  execFileSync("openssl", ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "30", "-keyout", key, "-out", cert, "-config", config], { stdio: "ignore" });
  fs.writeFileSync(stamp, names);
  return { key: fs.readFileSync(key), cert: fs.readFileSync(cert) };
}

/**
 * Resolves a URL path inside `root`, or null if it would leave it. A path that cannot be decoded
 * (bad percent encoding, a NUL byte) also gives null: it never throws.
 */
export function safeJoin(root, urlPath) {
  let relative;
  try {
    relative = decodeURIComponent(urlPath.split("?")[0]);
  } catch {
    return null;
  }
  if (relative.includes("\0")) return null;
  const target = path.normalize(path.join(root, relative === "/" ? "index.html" : relative));
  return target === root || target.startsWith(root + path.sep) ? target : null;
}

/** Writes `text` to a new file in `directory` named after `stem`, never overwriting. Returns the path. */
function writeUnique(directory, stem, text) {
  for (let attempt = 0; attempt < 100; attempt++) {
    const file = path.join(directory, `${stem}${attempt ? `-${attempt}` : ""}.json`);
    try {
      fs.writeFileSync(file, text, { flag: "wx" });
      return file;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
    }
  }
  throw new Error("could not find a free report file name");
}

/** Deletes the oldest reports beyond `keep`, so repeated uploads cannot fill the disk. */
function pruneReports(directory, keep) {
  const files = fs.readdirSync(directory).filter((name) => name.endsWith(".json")).sort();
  for (const name of files.slice(0, Math.max(0, files.length - keep))) fs.rmSync(path.join(directory, name), { force: true });
}

/**
 * Starts the server. `secure` serves https with a self-signed certificate; otherwise plain http.
 * Returns the server, the port it bound and a function that closes it.
 */
export async function startServer({
  dist = path.join(engine, "web", "dist"),
  reportsDir = path.join(engine, "target", "reports"),
  certDir = path.join(engine, "target", "lan-cert"),
  host = "0.0.0.0",
  port = 8443,
  secure = true,
} = {}) {
  const handler = (request, response) => {
    if (request.method === "POST" && request.url === "/report") {
      const chunks = [];
      let size = 0;
      request.on("data", (chunk) => {
        size += chunk.length;
        if (size > MAX_REPORT_BYTES) request.destroy();
        else chunks.push(chunk);
      });
      request.on("end", () => {
        try {
          const report = JSON.parse(Buffer.concat(chunks).toString("utf8"));
          const scene = String(report.scene ?? "unknown").replace(/[^a-z0-9]/gi, "").slice(0, 24) || "unknown";
          if (report === null || typeof report !== "object" || Array.isArray(report)) throw new Error("a report is a JSON object");
          fs.mkdirSync(reportsDir, { recursive: true });
          const file = writeUnique(reportsDir, `${new Date().toISOString().replace(/[:.]/g, "-")}-${scene}`, JSON.stringify(report, null, 2));
          pruneReports(reportsDir, MAX_REPORTS);
          console.log(`report saved: ${file}`);
          response.writeHead(204).end();
        } catch {
          response.writeHead(400).end();
        }
      });
      return;
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      response.writeHead(405).end();
      return;
    }
    const file = safeJoin(dist, request.url ?? "/");
    if (file === null && /%(?![0-9a-f]{2})/i.test(request.url ?? "")) {
      response.writeHead(400).end();
      return;
    }
    let body;
    try {
      body = file ? fs.readFileSync(file) : null;
    } catch {
      body = null;
    }
    if (!body) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "content-type": TYPES[path.extname(file)] ?? "application/octet-stream", "cache-control": "no-store" });
    response.end(request.method === "HEAD" ? undefined : body);
  };
  const server = secure
    ? https.createServer(ensureCertificate(certDir, lanAddresses()), handler)
    : http.createServer(handler);
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(port, host, resolve); });
  return { server, port: server.address().port, close: () => new Promise((resolve) => server.close(resolve)) };
}

if (process.argv[1] && fs.realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const option = (name, fallback) => (args.includes(name) ? args[args.indexOf(name) + 1] : fallback);
  const secure = !args.includes("--http");
  const host = option("--host", "0.0.0.0");
  const requested = Number(option("--port", 8443));
  const { port } = await startServer({ host, port: requested, secure });
  const scheme = secure ? "https" : "http";
  console.log(`serving web/dist on ${scheme}://${host}:${port} (Ctrl-C to stop)\n`);
  const queries = ["?scene=xr&xr=auto", "?scene=bench1&bench=1&seconds=60", "?scene=bench2&bench=1&seconds=60", "?scene=bench3&bench=1&seconds=60"];
  const hosts = host === "0.0.0.0" ? ["localhost", ...lanAddresses()] : [host];
  for (const name of hosts) {
    console.log(`  ${scheme}://${name}:${port}/`);
  }
  console.log("\nuseful pages (append to a URL above; press Enter VR on the page):");
  for (const query of queries) console.log(`  ${query}`);
  console.log("\nbenchmark pages post their result here when the session ends; reports go to target/reports/");
  if (!secure) console.log("with --http, use USB: adb reverse tcp:8443 tcp:8443  then open http://localhost:8443/ on the headset");
}
