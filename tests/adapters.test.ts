import { afterEach, expect, test } from "bun:test";
import { applyCommands, makeObject, responseSchema } from "../shared/commands";
import {
  ChatCompletionsAdapter,
  createAdapter,
  MockAdapter,
} from "../server/adapters";
import type { SceneRequest } from "../shared/commands";

const request: SceneRequest = {
  prompt: "Build a table",
  scene: { version: 1, objects: [] },
  selectedId: null,
};
const mock = new MockAdapter();
test("unconfigured provider uses labelled mock; bad configuration fails clearly", () => {
  expect(createAdapter({}).mode).toBe("mock");
  expect(() => createAdapter({ LLM_PROVIDER: "unknown" })).toThrow(
    "Unsupported provider",
  );
  expect(() => createAdapter({ LLM_PROVIDER: "chat-completions" })).toThrow(
    "Configure",
  );
});
test("mock composes a table using registered primitives and unique IDs", async () => {
  const response = responseSchema.parse(await mock.generate(request));
  const scene = applyCommands(request.scene, response.commands);
  expect(scene.objects).toHaveLength(5);
  expect(scene.objects.filter((o) => o.factory === "box")).toHaveLength(1);
  expect(scene.objects.filter((o) => o.factory === "cylinder")).toHaveLength(4);
  expect(new Set(scene.objects.map((o) => o.id)).size).toBe(5);
});
test("mock follow-ups keep the selected ID while changing position and colour", async () => {
  const object = makeObject("box", "A block", { id: "original" });
  const input: SceneRequest = {
    prompt: "Move it left 2 and make it blue",
    selectedId: object.id,
    scene: { version: 1, objects: [object] },
  };
  const result = await mock.generate(input);
  const next = applyCommands(input.scene, result.commands);
  expect(next.objects[0].id).toBe("original");
  expect(next.objects[0].position[0]).toBe(-2);
  expect(next.objects[0].color).toBe("#8ca8df");
});
test("mock primitive counts are bounded", async () => {
  const result = await mock.generate({
    ...request,
    prompt: "Add 3 blue columns",
  });
  expect(result.commands).toHaveLength(3);
  expect(
    applyCommands(request.scene, result.commands).objects.every(
      (o) => o.factory === "cylinder",
    ),
  ).toBe(true);
});
const realFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = realFetch;
});
test("live adapter validates provider output and sends scene context server-side", async () => {
  let payload = "";
  globalThis.fetch = (async (_url: unknown, options: RequestInit) => {
    payload = String(options.body);
    return Response.json({
      choices: [
        {
          message: {
            content: JSON.stringify({ message: "Done", commands: [] }),
          },
        },
      ],
    });
  }) as typeof fetch;
  const adapter = new ChatCompletionsAdapter({
    url: "https://example.test/scene",
    key: "server-secret",
    model: "test-model",
  });
  expect((await adapter.generate(request)).commands).toEqual([]);
  expect(payload).toContain("selectedId");
  expect(payload).not.toContain("server-secret");
});
test("live adapter rejects structurally valid commands with unknown IDs", async () => {
  globalThis.fetch = (async () =>
    Response.json({
      choices: [
        {
          message: {
            content: JSON.stringify({
              message: "Bad",
              commands: [{ type: "delete", id: "nonexistent" }],
            }),
          },
        },
      ],
    })) as unknown as typeof fetch;
  await expect(
    new ChatCompletionsAdapter({
      url: "https://example.test",
      key: "k",
      model: "m",
    }).generate(request),
  ).rejects.toThrow("Unknown ID");
});
