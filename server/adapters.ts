import { applyCommands, makeObject, responseSchema } from "../shared/commands";
import type {
  SceneCommand,
  SceneRequest,
  SceneResponse,
  Vec3,
} from "../shared/commands";

export interface SceneAdapter {
  mode: string;
  generate(request: SceneRequest): Promise<SceneResponse>;
}
const colors: Record<string, string> = {
  red: "#df786f",
  blue: "#8ca8df",
  green: "#8ebca3",
  purple: "#a99bd2",
  lavender: "#a99bd2",
  coral: "#e7a98d",
  white: "#eeeef1",
  black: "#343841",
  yellow: "#ebcd80",
};
export class MockAdapter implements SceneAdapter {
  mode = "mock";
  async generate({
    prompt,
    scene,
    selectedId,
  }: SceneRequest): Promise<SceneResponse> {
    const text = prompt.toLowerCase();
    const color = Object.entries(colors).find(([name]) =>
      new RegExp(`\\b${name}\\b`).test(text),
    )?.[1];
    const target =
      scene.objects.find(
        (o) =>
          text.includes(o.id.toLowerCase()) ||
          text.includes(o.name.toLowerCase()),
      ) ?? scene.objects.find((o) => o.id === selectedId);
    if (/\b(delete|remove)\b/.test(text)) {
      return target
        ? {
            message: `Removed ${target.name}.`,
            commands: [{ type: "delete", id: target.id }],
          }
        : { message: "Select an object to remove.", commands: [] };
    }
    if (
      /\b(move|rotate|scale|make|paint|color|colour|change)\b/.test(text) &&
      target &&
      !/\b(create|add|build)\b/.test(text)
    ) {
      const patch: Extract<SceneCommand, { type: "update" }>["patch"] = {};
      if (color) patch.color = color;
      if (/\b(move|higher|lower)\b/.test(text)) {
        const p = [...target.position] as Vec3;
        const distance = Math.min(
          10,
          Number(text.match(/\d+(?:\.\d+)?/)?.[0] ?? 1),
        );
        if (/left/.test(text)) p[0] -= distance;
        else if (/right/.test(text)) p[0] += distance;
        else if (/down|lower/.test(text)) p[1] -= distance;
        else if (/back/.test(text)) p[2] -= distance;
        else if (/forward/.test(text)) p[2] += distance;
        else p[1] += distance;
        patch.position = p;
      }
      if (/rotate/.test(text))
        patch.rotation = [
          target.rotation[0],
          target.rotation[1] + Math.PI / 4,
          target.rotation[2],
        ];
      if (/scale|bigger|larger|smaller/.test(text))
        patch.scale = target.scale.map((v) =>
          Math.max(0.02, Math.min(100, v * (/smaller/.test(text) ? 0.8 : 1.2))),
        ) as Vec3;
      return Object.keys(patch).length
        ? {
            message: `Updated ${target.name}, keeping its ID.`,
            commands: [{ type: "update", id: target.id, patch }],
          }
        : {
            message:
              "Mock mode supports move, rotate, bigger/smaller, named colours, and remove. For example: “make it blue”.",
            commands: [],
          };
    }
    if (/table/.test(text)) {
      const commands: SceneCommand[] = [
        {
          type: "create",
          object: makeObject("box", "Table top", {
            position: [0, 1.5, 2.5],
            scale: [2.6, 0.16, 1.6],
            color: color ?? "#d0a482",
          }),
        },
      ];
      for (const x of [-1, 1])
        for (const z of [-0.55, 0.55])
          commands.push({
            type: "create",
            object: makeObject("cylinder", "Table leg", {
              position: [x, 0.72, z + 2.5],
              scale: [0.14, 1.44, 0.14],
              color: "#746657",
            }),
          });
      return {
        message:
          "Composed a table from a box top and four cylinder legs. Select a part to edit it.",
        commands,
      };
    }
    const factory = /cylinder|column|tower/.test(text)
      ? "cylinder"
      : /plane|floor|ground/.test(text)
      ? "plane"
      : "box";
    const requestedCount = Number(
      text.match(/\b([1-6])\b/)?.[1] ??
        (/three/.test(text) ? 3 : /two/.test(text) ? 2 : 1),
    );
    const count = Math.min(6, Math.max(1, requestedCount));
    return {
      message: `Added ${count} ${factory}${
        count > 1 ? "s" : ""
      }. Mock mode uses a small deterministic vocabulary; no LLM was contacted.`,
      commands: Array.from({ length: count }, (_, i) => ({
        type: "create" as const,
        object: makeObject(
          factory,
          `${factory[0].toUpperCase()}${factory.slice(1)} ${
            scene.objects.length + i + 1
          }`,
          {
            color: color ?? "#9cafd5",
            position: [
              (i - (count - 1) / 2) * 1.3,
              factory === "plane" ? 0.03 : 0.7,
              2,
            ],
            scale: factory === "plane" ? [4, 1, 4] : [1, 1.4, 1],
          },
        ),
      })),
    };
  }
}

const systemPrompt = `You author small 3D scenes via JSON commands. Return only {"message":string,"commands":array}.
Commands: {"type":"create","object":{"id":unique string of letters/digits/hyphens,"name":string,"factory":"box"|"cylinder"|"plane","position":[x,y,z],"rotation":[x,y,z],"scale":[x,y,z],"color":"#rrggbb"}}, {"type":"update","id":existing ID,"patch":nonempty partial object excluding id}, {"type":"delete","id":existing ID}.
Y is up; rotations are radians; units are meters. Box and cylinder are unit size, cylinder diameter 1. Plane lies in XZ and is unit width/depth. Scales must be positive 0.02..100; positions within +/-1000. Use multiple primitives to compose varied objects. Maximum 60 commands. Follow-up prompts edit existing IDs, never recreate them. If selectedId is supplied, "it" means that object. Never modify the studio floor unless requested. No code, markdown, URLs, or other keys.`;

export class ChatCompletionsAdapter implements SceneAdapter {
  mode = "chat-completions";
  constructor(private config: { url: string; key: string; model: string }) {}
  async generate(request: SceneRequest): Promise<SceneResponse> {
    const response = await fetch(this.config.url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${this.config.key}`,
      },
      body: JSON.stringify({
        model: this.config.model,
        temperature: 0.2,
        response_format: { type: "json_object" },
        messages: [
          { role: "system", content: systemPrompt },
          { role: "user", content: JSON.stringify(request) },
        ],
      }),
      signal: AbortSignal.timeout(45000),
    });
    if (!response.ok)
      throw new Error(`Provider request failed (${response.status})`);
    const body = (await response.json()) as {
      choices?: { message?: { content?: string } }[];
    };
    const content = body.choices?.[0]?.message?.content;
    if (!content) throw new Error("Provider returned no scene commands");
    const result = responseSchema.parse(JSON.parse(content));
    applyCommands(request.scene, result.commands);
    return result;
  }
}

export function createAdapter(
  env: Record<string, string | undefined>,
): SceneAdapter {
  if (!env.LLM_PROVIDER || env.LLM_PROVIDER === "mock")
    return new MockAdapter();
  if (env.LLM_PROVIDER !== "chat-completions")
    throw new Error(`Unsupported provider: ${env.LLM_PROVIDER}`);
  if (!env.LLM_API_URL || !env.LLM_API_KEY || !env.LLM_MODEL)
    throw new Error(
      "Configure LLM_API_URL, LLM_API_KEY and LLM_MODEL on the server",
    );
  const url = new URL(env.LLM_API_URL);
  if (
    url.protocol !== "https:" &&
    url.hostname !== "localhost" &&
    url.hostname !== "127.0.0.1"
  )
    throw new Error("Provider URL must use HTTPS (or localhost)");
  return new ChatCompletionsAdapter({
    url: url.href,
    key: env.LLM_API_KEY,
    model: env.LLM_MODEL,
  });
}
