import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import type { MeshPart } from '../worker-types';
import { cornerNormals } from '../mesh-upload';
import { Baker } from './bake';
import { bakeMaterialMaps } from './apply';
import { loadBrowserTextureLibrary } from './browser-library';

const MAX_PREVIEW_TRIANGLES = 200_000;
let activeClose: (() => void) | undefined;

function texture(pixels: Uint8Array | undefined, size: number, color: boolean) {
  if (!pixels) return undefined;
  const map = new THREE.DataTexture(pixels, size, size, THREE.RGBAFormat, THREE.UnsignedByteType);
  map.colorSpace = color ? THREE.SRGBColorSpace : THREE.NoColorSpace;
  map.magFilter = THREE.LinearFilter;
  map.minFilter = THREE.LinearMipmapLinearFilter;
  map.generateMipmaps = true;
  map.needsUpdate = true;
  return map;
}

/** Opens a disposable, orbitable 3D preview of a baked material on its actual evaluated mesh. */
export async function openTexturePreview(part: MeshPart, size = 512) {
  if (!part.material?.texture_program) throw new Error('select a part with a solid.texture program first');
  if (part.indices.length / 3 > MAX_PREVIEW_TRIANGLES) throw new Error(`preview is capped at ${MAX_PREVIEW_TRIANGLES.toLocaleString()} triangles`);
  activeClose?.();
  const { library, helpers } = await loadBrowserTextureLibrary();
  const baker = new Baker(new OffscreenCanvas(1, 1));
  let baked;
  try {
    baked = bakeMaterialMaps(library, helpers, part, baker, size);
  } finally {
    baker.dispose();
  }

  const positions = new Float32Array(part.indices.length * 3);
  const normals = cornerNormals(part.positions, part.indices, 40);
  for (let corner = 0; corner < part.indices.length; corner++) {
    const source = part.indices[corner] * 3;
    positions.set(part.positions.subarray(source, source + 3), corner * 3);
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
  geometry.setAttribute('uv', new THREE.BufferAttribute(baked.uvs, 2));
  geometry.computeBoundingBox();
  const box = geometry.boundingBox!;
  const center = box.getCenter(new THREE.Vector3());
  const radius = Math.max(box.getBoundingSphere(new THREE.Sphere()).radius, 0.05);
  geometry.translate(-center.x, -center.y, -center.z);

  const map = texture(baked.albedo, size, true);
  const packed = texture(baked.metallicRoughness, size, false);
  const normalMap = texture(baked.normal, size, false);
  const emissiveMap = texture(baked.emissive, size, true);
  const color = part.material.color;
  const material = new THREE.MeshStandardMaterial({
    color: new THREE.Color().setRGB(color[0], color[1], color[2], THREE.SRGBColorSpace),
    map,
    metalnessMap: packed,
    roughnessMap: packed,
    normalMap,
    // The maps follow glTF (row 0 is v = 0, +Y is up); three.js's derivative tangent frame wants y flipped
    // for that, as GLTFLoader does for files without tangents.
    normalScale: new THREE.Vector2(1, -1),
    emissiveMap,
    emissive: emissiveMap ? new THREE.Color(1, 1, 1) : new THREE.Color(...part.material.emissive),
    metalness: part.material.metallic,
    roughness: part.material.roughness,
  });

  const overlay = document.createElement('div');
  overlay.id = 'texture-preview';
  const safeName = part.name.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');
  overlay.innerHTML = `<style>
    #texture-preview { position:fixed; inset:0; z-index:10000; display:grid; place-items:center; padding:24px; background:rgba(5,8,14,.78); backdrop-filter:blur(12px); color:#fff; font:14px/1.45 system-ui,sans-serif; }
    #texture-preview .card { position:relative; width:min(900px,95vw); height:min(680px,88vh); overflow:hidden; border:1px solid rgba(255,255,255,.24); border-radius:20px; background:rgba(22,27,39,.88); box-shadow:0 24px 90px rgba(0,0,0,.48); }
    #texture-preview header { position:absolute; z-index:1; inset:14px 16px auto; display:flex; justify-content:space-between; align-items:center; pointer-events:none; }
    #texture-preview header button { pointer-events:auto; border:1px solid rgba(255,255,255,.28); border-radius:10px; padding:7px 12px; color:#fff; background:rgba(255,255,255,.12); font:inherit; cursor:pointer; }
    #texture-preview canvas { display:block; width:100%; height:100%; }
    #texture-preview .hint { position:absolute; inset:auto 16px 12px; opacity:.7; pointer-events:none; }
  </style><section class="card"><header><strong>3D texture preview · ${safeName}</strong><button type="button">Close</button></header><canvas></canvas><small class="hint">Drag to orbit · scroll to zoom</small></section>`;
  document.body.append(overlay);
  const canvas = overlay.querySelector('canvas')!;
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: 'low-power' });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  const scene = new THREE.Scene();
  scene.background = new THREE.Color('#171c29');
  scene.add(new THREE.HemisphereLight(0xcfe0ff, 0x30271f, 2.0));
  const key = new THREE.DirectionalLight(0xffe5c2, 3.0);
  key.position.set(-2, 4, 3);
  scene.add(key);
  scene.add(new THREE.Mesh(geometry, material));
  const camera = new THREE.PerspectiveCamera(40, 1, Math.max(radius / 1000, 0.001), radius * 100);
  camera.position.set(radius * 2.7, radius * 1.8, radius * 3.2);
  const controls = new OrbitControls(camera, canvas);
  controls.target.set(0, 0, 0);
  controls.update();
  const resize = () => {
    const bounds = canvas.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    renderer.setSize(bounds.width, bounds.height, false);
    camera.aspect = bounds.width / bounds.height;
    camera.updateProjectionMatrix();
    renderer.render(scene, camera);
  };
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  controls.addEventListener('change', () => renderer.render(scene, camera));
  resize();
  let close = () => {};
  const onKeyDown = (event: KeyboardEvent) => { if (event.key === 'Escape') close(); };
  close = () => {
    if (!overlay.isConnected) return;
    window.removeEventListener('keydown', onKeyDown);
    observer.disconnect(); controls.dispose(); renderer.dispose(); geometry.dispose(); material.dispose();
    map?.dispose(); packed?.dispose(); normalMap?.dispose(); emissiveMap?.dispose(); overlay.remove();
    if (activeClose === close) activeClose = undefined;
  };
  activeClose = close;
  window.addEventListener('keydown', onKeyDown);
  overlay.querySelector('button')!.addEventListener('click', close, { once: true });
  overlay.addEventListener('click', event => { if (event.target === overlay) close(); });
  return close;
}
