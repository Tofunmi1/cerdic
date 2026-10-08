import { mkdir } from "node:fs/promises";

async function run(args: string[]) {
  const child = Bun.spawn(args, { stdout: "inherit", stderr: "inherit" });
  if ((await child.exited) !== 0)
    throw new Error(
      `${args[0]} failed. See README for Rust/WASM prerequisites.`,
    );
}
await run([
  "cargo",
  "build",
  "--manifest-path",
  "rust/geometry/Cargo.toml",
  "--target",
  "wasm32-unknown-unknown",
  "--release",
]);
await mkdir("src/wasm", { recursive: true });
await run([
  "wasm-bindgen",
  "rust/geometry/target/wasm32-unknown-unknown/release/spatial_geometry.wasm",
  "--target",
  "web",
  "--out-dir",
  "src/wasm",
  "--out-name",
  "geometry",
]);
