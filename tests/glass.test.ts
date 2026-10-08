import { expect, test } from "bun:test";
import { lensNormal } from "../src/ui/glassOptics";

test("the rounded lens keeps its center and exterior undistorted", () => {
  expect(lensNormal(150, 100, 300, 200, 28)).toEqual([0, 0]);
  expect(lensNormal(-20, 50, 300, 200, 28)).toEqual([0, 0]);
});
test("opposite rims bend in opposite directions with bounded strength", () => {
  const left = lensNormal(9, 100, 300, 200, 28);
  const right = lensNormal(291, 100, 300, 200, 28);
  expect(left[0]).toBeLessThan(0);
  expect(right[0]).toBeGreaterThan(0);
  expect(left[0]).toBeCloseTo(-right[0]);
  expect(Math.abs(left[0])).toBeLessThanOrEqual(0.8);
  expect(left[1]).toBe(0);
});
test("rounded corner normals bend both axes and remain finite", () => {
  const normal = lensNormal(16, 16, 300, 200, 28);
  expect(normal[0]).toBeLessThan(0);
  expect(normal[1]).toBeLessThan(0);
  for (const n of lensNormal(4, 4, 10, 10, 40))
    expect(Number.isFinite(n)).toBe(true);
});
