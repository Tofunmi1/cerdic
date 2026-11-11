import { ZodError } from "zod";
import { resolve, sep } from "node:path";
import {
  applyCommands,
  requestSchema,
  responseSchema,
} from "../shared/commands";
import { createAdapter } from "./adapters";
import { LocalAssistant } from './assistant/session';

const adapter = createAdapter(process.env);
const root = resolve("dist");
const server = Bun.serve<{ assistant?: LocalAssistant }>({
  hostname: "127.0.0.1",
  port: Number(process.env.PORT ?? 3001),
  maxRequestBodySize: 512 * 1024,
  async fetch(request): Promise<Response | undefined> {
    const path = new URL(request.url).pathname;
    if (path === '/api/assistant/socket') {
      const origin = request.headers.get('origin');
      if (!origin || !['http://localhost:5173', 'http://127.0.0.1:5173', `http://localhost:${server.port}`, `http://127.0.0.1:${server.port}`].includes(origin)) return new Response('Origin rejected', { status: 403 });
      return server.upgrade(request, { data: {} }) ? undefined : new Response('WebSocket required', { status: 426 });
    }
    if (path === '/api/assistant/status') return Response.json({ available: Boolean(Bun.which(process.env.CERDIC_CODEX_BIN ?? 'codex')), provider: 'local-codex' });
    if (path === "/api/status") return Response.json({ mode: adapter.mode });
    if (path === "/api/scene" && request.method === "POST") {
      // Same-origin requests only; this local development server is not a public deployment.
      const origin = request.headers.get("origin");
      if (
        origin &&
        ![
          "http://localhost:5173",
          "http://127.0.0.1:5173",
          `http://localhost:${server.port}`,
          `http://127.0.0.1:${server.port}`,
        ].includes(origin)
      )
        return Response.json({ error: "Origin rejected" }, { status: 403 });
      try {
        const input = requestSchema.parse(await request.json());
        const result = responseSchema.parse(await adapter.generate(input));
        applyCommands(input.scene, result.commands);
        return Response.json(result);
      } catch (error) {
        const invalid =
          error instanceof ZodError || error instanceof SyntaxError;
        return Response.json(
          {
            error: invalid
              ? "Invalid scene data or provider command response."
              : error instanceof Error
              ? error.message
              : "Scene request failed",
          },
          { status: invalid ? 400 : 502 },
        );
      }
    }
    if (path.startsWith("/api/"))
      return Response.json({ error: "Not found" }, { status: 404 });
    const filename = resolve(root, path === "/" ? "./index.html" : `.${path}`);
    if (!filename.startsWith(root + sep))
      return new Response("Not found", { status: 404 });
    const file = Bun.file(filename);
    if (await file.exists()) return new Response(file);
    const index = Bun.file(resolve(root, "index.html"));
    return (await index.exists())
      ? new Response(index)
      : new Response("Run bun run dev, or build the app first.", {
          status: 404,
        });
  },
  websocket: {
    maxPayloadLength: 24 * 1024 * 1024,
    open(socket) { socket.data.assistant = new LocalAssistant(event => { if (socket.readyState === 1) socket.send(JSON.stringify(event)); }); },
    message(socket, raw) {
      try { void socket.data.assistant?.receive(JSON.parse(String(raw))); }
      catch { socket.send(JSON.stringify({ type: 'error', message: 'Invalid assistant message' })); }
    },
    close(socket) { socket.data.assistant?.close(); },
  },
});
console.log(
  `Cerdic API: http://localhost:${server.port} (sidebar: local Codex; legacy scene adapter: ${adapter.mode})`,
);
