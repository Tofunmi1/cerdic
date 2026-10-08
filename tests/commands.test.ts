import { describe, expect, test } from "bun:test";
import {
  applyCommands,
  makeObject,
  responseSchema,
  sceneSchema,
} from "../shared/commands";
import { commit, createHistory, redo, undo } from "../shared/history";
import type { SceneDocument } from "../shared/commands";

const object = makeObject("box", "Box", { id: "stable-box" });
const scene: SceneDocument = { version: 1, objects: [object] };
describe("shared scene commands", () => {
  test("updates keep IDs and leave input unchanged", () => {
    const next = applyCommands(scene, [
      {
        type: "update",
        id: object.id,
        patch: { position: [2, 3, 4], color: "#abcdef" },
      },
    ]);
    expect(next.objects[0].id).toBe(object.id);
    expect(next.objects[0].position).toEqual([2, 3, 4]);
    expect(scene.objects[0].position).toEqual([0, 0.5, 0]);
  });
  test("a batch rejects atomically for missing or duplicated IDs", () => {
    expect(() =>
      applyCommands(scene, [
        { type: "update", id: object.id, patch: { name: "Changed" } },
        { type: "delete", id: "missing" },
      ]),
    ).toThrow("Unknown ID");
    expect(scene.objects[0].name).toBe("Box");
    expect(() => applyCommands(scene, [{ type: "create", object }])).toThrow(
      "Duplicate ID",
    );
  });
  test("rejects unsupported factories, invalid scales, nonfinite numbers and extra payload keys", () => {
    for (const patch of [
      { factory: "script" },
      { scale: [-1, 1, 1] },
      { position: [NaN, 0, 0] },
      { color: "url(x)" },
      { id: "new" },
    ]) {
      expect(() =>
        responseSchema.parse({
          message: "edit",
          commands: [{ type: "update", id: object.id, patch }],
        }),
      ).toThrow();
    }
    expect(() =>
      responseSchema.parse({ message: "x", commands: [], script: "x" }),
    ).toThrow();
  });
  test("saved scenes require a known version and unique IDs", () => {
    expect(() => sceneSchema.parse({ version: 2, objects: [] })).toThrow();
    expect(() =>
      sceneSchema.parse({ version: 1, objects: [object, object] }),
    ).toThrow();
    expect(sceneSchema.parse(JSON.parse(JSON.stringify(scene)))).toEqual(scene);
  });
  test("create then update in the same transaction works", () => {
    const next = applyCommands({ version: 1, objects: [] }, [
      { type: "create", object },
      { type: "update", id: object.id, patch: { name: "Renamed" } },
    ]);
    expect(next.objects[0].name).toBe("Renamed");
  });
  test("undo/redo operate on complete batches and new edits clear redo", () => {
    const first = commit(createHistory(scene), [
      { type: "update", id: object.id, patch: { name: "New" } },
    ]);
    const reverted = undo(first);
    expect(reverted.present).toEqual(scene);
    expect(redo(reverted).present).toEqual(first.present);
    expect(
      commit(reverted, [{ type: "delete", id: object.id }]).future,
    ).toHaveLength(0);
    expect(first.revision).toBe(1);
    expect(reverted.revision).toBe(2);
  });
});
