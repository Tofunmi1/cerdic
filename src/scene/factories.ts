import { BufferGeometry, Float32BufferAttribute } from "three";
import init, {
  box_vertices,
  cylinder_vertices,
  plane_vertices,
} from "../wasm/geometry";
import type { FactoryId } from "../../shared/commands";

type GeometryFactory = () => Float32Array;
const factories = new Map<FactoryId, GeometryFactory>();
export function registerFactory(id: FactoryId, factory: GeometryFactory) {
  if (factories.has(id)) throw new Error(`Factory already registered: ${id}`);
  factories.set(id, factory);
}
export async function initializeGeometry() {
  await init();
  registerFactory("box", box_vertices);
  registerFactory("cylinder", () => cylinder_vertices(48));
  registerFactory("plane", plane_vertices);
}
export function createGeometry(id: FactoryId): BufferGeometry {
  const factory = factories.get(id);
  if (!factory) throw new Error(`Geometry factory unavailable: ${id}`);
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(factory(), 3));
  geometry.computeVertexNormals();
  geometry.computeBoundingSphere();
  return geometry;
}
