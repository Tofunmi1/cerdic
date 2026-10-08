import { ZodError } from "zod";
import { resolve, sep } from "node:path";
import {
  applyCommands,
  requestSchema,
  responseSchema,
} from "../shared/commands";
import { createAdapter } from "./adapters";

const adapter = createAdapter(process.env);
const root = resolve("dist");
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: Number(process.env.PORT ?? 3001),
  maxRequestBodySize: 512 * 1024,
  async fetch(request) {
    const path = new URL(request.url).pathname;
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
});
console.log(
  `Spatial Lab API: http://localhost:${server.port} (${adapter.mode})`,
);
