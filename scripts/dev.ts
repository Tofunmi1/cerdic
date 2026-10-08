const children = [
  Bun.spawn(["bun", "--watch", "server/index.ts"], {
    stdout: "inherit",
    stderr: "inherit",
  }),
  Bun.spawn(["bun", "x", "vite", "--host", "127.0.0.1"], {
    stdout: "inherit",
    stderr: "inherit",
  }),
];
const stop = () => children.forEach((child) => child.kill());
process.on("SIGINT", () => {
  stop();
  process.exit(0);
});
process.on("SIGTERM", () => {
  stop();
  process.exit(0);
});
const exitCode = await Promise.race(children.map((child) => child.exited));
stop();
process.exit(exitCode);
export {};
