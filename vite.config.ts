import { createReadStream, existsSync, statSync } from "node:fs";
import { cp, mkdir } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { defineConfig } from "vite";
import type { Plugin } from "vite";
import react from "@vitejs/plugin-react";

// The engine (the Bevy WebGPU bundle: runtime and evaluator wasm, their glue, the worker and the example worlds) is built into engine/web/dist-bevy-gpu.
// The dev server serves it at /engine; a production build copies it into dist/engine, so one deploy carries the app and the engine.
const engineDir = new URL("./engine/web/dist-bevy-gpu/", import.meta.url).pathname;
const TYPES: Record<string, string> = {
  ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".json": "application/json",
  ".txt": "text/plain; charset=utf-8", ".html": "text/html", ".pack": "application/octet-stream",
};

function engine(): Plugin {
  return {
    name: "engine-bundle",
    configureServer(server) {
      server.middlewares.use("/engine", (request, response, next) => {
        const path = normalize(decodeURIComponent((request.url ?? "/").split("?")[0])).replace(/^(\.\.[/\\])+/, "");
        const file = join(engineDir, path);
        if (!file.startsWith(engineDir) || !existsSync(file) || !statSync(file).isFile()) return next();
        response.setHeader("Content-Type", TYPES[extname(file)] ?? "application/octet-stream");
        response.setHeader("Cache-Control", "no-cache");
        createReadStream(file).pipe(response);
      });
    },
    async closeBundle() {
      if (!existsSync(engineDir)) return;
      await mkdir("dist/engine", { recursive: true });
      await cp(engineDir, "dist/engine", { recursive: true, filter: (source) => !/\/(node_modules|tests)(\/|$)/.test(source) });
    },
  };
}

export default defineConfig({
  plugins: [react(), engine()],
  optimizeDeps: { entries: ['index.html'] },
  build: { target: "es2022", chunkSizeWarningLimit: 1500 },
  server: {
    port: 5173,
    strictPort: true,
    proxy: { "/api": "http://127.0.0.1:3001" },
  },
});
