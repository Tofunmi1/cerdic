// Static server for the browser check (cross-origin isolated, like the planned engine worker).
const root = new URL(".", import.meta.url).pathname;
Bun.serve({
  port: 5199, hostname: "127.0.0.1",
  async fetch(req) {
    const path = new URL(req.url).pathname;
    const file = Bun.file(root + (path === "/" ? "index.html" : path.slice(1)));
    if (!(await file.exists())) return new Response("not found", { status: 404 });
    return new Response(file, { headers: {
      "Cross-Origin-Opener-Policy": "same-origin", "Cross-Origin-Embedder-Policy": "require-corp",
    } });
  },
});
console.log("http://127.0.0.1:5199");
