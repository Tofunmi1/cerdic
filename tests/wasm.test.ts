import { expect, test } from "bun:test";
import init, {
  box_vertices,
  cylinder_vertices,
  plane_vertices,
} from "../src/wasm/geometry";

test("compiled wasm-bindgen factories execute and return real vertex buffers", async () => {
  await init({
    module_or_path: await Bun.file("src/wasm/geometry_bg.wasm").arrayBuffer(),
  });
  expect(box_vertices()).toBeInstanceOf(Float32Array);
  expect(box_vertices().length).toBe(108);
  expect(plane_vertices().length).toBe(18);
  expect(cylinder_vertices(24).length).toBe(24 * 36);
  expect([...cylinder_vertices(24)].every(Number.isFinite)).toBe(true);
});
