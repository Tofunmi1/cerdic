import { makeObject, sceneSchema } from "../../shared/commands";
import type { SceneDocument } from "../../shared/commands";

const key = "spatial-lab.scene.v2";
export const starterScene: SceneDocument = {
  version: 1,
  objects: [
    makeObject("plane", "Studio floor", {
      id: "studio-floor",
      position: [0, 0, 0],
      scale: [12, 1, 12],
      color: "#dae1e7",
    }),
    makeObject("cylinder", "Pedestal", {
      id: "pedestal",
      position: [0, 0.22, 0],
      scale: [2.6, 0.44, 2.6],
      color: "#dee5ed",
    }),
    makeObject("box", "Lavender block", {
      id: "lavender-block",
      position: [-0.75, 0.98, 0],
      rotation: [0, 0.22, 0],
      scale: [0.85, 1.1, 0.85],
      color: "#a99bd2",
    }),
    makeObject("cylinder", "Coral column", {
      id: "coral-column",
      position: [0.6, 1.12, 0.15],
      scale: [0.72, 1.4, 0.72],
      color: "#e7a98d",
    }),
  ],
};
export function readScene(): SceneDocument {
  const raw = localStorage.getItem(key);
  return raw
    ? sceneSchema.parse(JSON.parse(raw))
    : structuredClone(starterScene);
}
export function saveScene(scene: SceneDocument) {
  localStorage.setItem(key, JSON.stringify(sceneSchema.parse(scene)));
}
