import { z } from "zod";

export const factoryIds = ["box", "cylinder", "plane"] as const;
const id = z
  .string()
  .min(1)
  .max(80)
  .regex(/^[\w-]+$/);
const vector = z.tuple([
  z.number().finite().min(-1000).max(1000),
  z.number().finite().min(-1000).max(1000),
  z.number().finite().min(-1000).max(1000),
]);
const scale = z.tuple([
  z.number().min(0.02).max(100),
  z.number().min(0.02).max(100),
  z.number().min(0.02).max(100),
]);
export const objectSchema = z
  .object({
    id,
    name: z.string().trim().min(1).max(100),
    factory: z.enum(factoryIds),
    position: vector,
    rotation: vector,
    scale,
    color: z.string().regex(/^#[0-9a-fA-F]{6}$/),
  })
  .strict();
export const patchSchema = objectSchema
  .omit({ id: true })
  .partial()
  .refine(
    (patch) => Object.keys(patch).length > 0,
    "Patch must contain a change",
  );
export const commandSchema = z.discriminatedUnion("type", [
  z.object({ type: z.literal("create"), object: objectSchema }).strict(),
  z.object({ type: z.literal("update"), id, patch: patchSchema }).strict(),
  z.object({ type: z.literal("delete"), id }).strict(),
]);
export const responseSchema = z
  .object({
    message: z.string().min(1).max(2000),
    commands: z.array(commandSchema).max(100),
  })
  .strict();
export const sceneSchema = z
  .object({ version: z.literal(1), objects: z.array(objectSchema).max(250) })
  .strict()
  .refine(
    (scene) =>
      new Set(scene.objects.map((object) => object.id)).size ===
      scene.objects.length,
    "Object IDs must be unique",
  );
export const requestSchema = z
  .object({
    prompt: z.string().trim().min(1).max(4000),
    scene: sceneSchema,
    selectedId: id.nullable(),
  })
  .strict();
export type SceneObject = z.infer<typeof objectSchema>;
export type SceneDocument = z.infer<typeof sceneSchema>;
export type SceneCommand = z.infer<typeof commandSchema>;
export type ScenePatch = z.infer<typeof patchSchema>;
export type SceneRequest = z.infer<typeof requestSchema>;
export type SceneResponse = z.infer<typeof responseSchema>;
export type Vec3 = SceneObject["position"];
export type FactoryId = SceneObject["factory"];

// Transactional: any invalid command rejects the entire batch.
export function applyCommands(
  scene: SceneDocument,
  commands: SceneCommand[],
): SceneDocument {
  let objects = sceneSchema
    .parse(scene)
    .objects.map((object) => structuredClone(object));
  for (const raw of commands) {
    const command = commandSchema.parse(raw);
    if (command.type === "create") {
      if (objects.some((object) => object.id === command.object.id))
        throw new Error(`Duplicate ID: ${command.object.id}`);
      objects.push(command.object);
    } else {
      if (!objects.some((object) => object.id === command.id))
        throw new Error(`Unknown ID: ${command.id}`);
      objects =
        command.type === "delete"
          ? objects.filter((object) => object.id !== command.id)
          : objects.map((object) =>
              object.id === command.id
                ? objectSchema.parse({ ...object, ...command.patch })
                : object,
            );
    }
  }
  return sceneSchema.parse({ version: 1, objects });
}

export function makeObject(
  factory: FactoryId,
  name: string,
  overrides: Partial<SceneObject> = {},
): SceneObject {
  return objectSchema.parse({
    id: crypto.randomUUID(),
    name,
    factory,
    position: [0, 0.5, 0],
    rotation: [0, 0, 0],
    scale: [1, 1, 1],
    color: "#a9b8e8",
    ...overrides,
  });
}
