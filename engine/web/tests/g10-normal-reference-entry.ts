// Browser entry for the normal-map reference test: renders a GLB with three.js, once with the file's own
// TANGENT data and once with three.js's derivative-based tangent frame (what it uses when a file has none).
import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';

async function render(bytes: Uint8Array, withTangents: boolean): Promise<string> {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 512;
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, preserveDrawingBuffer: true });
  renderer.toneMapping = THREE.NoToneMapping;
  renderer.setSize(512, 512, false);
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x202020);
  const gltf = await new GLTFLoader().parseAsync(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer, '');
  gltf.scene.traverse(object => {
    const mesh = object as THREE.Mesh;
    if (!mesh.isMesh) return;
    const material = mesh.material as THREE.MeshStandardMaterial;
    if (!withTangents && mesh.geometry.getAttribute('tangent')) {
      // No vertex tangents: three.js builds the frame from derivatives and, for glTF, flips the normal's y.
      mesh.geometry.deleteAttribute('tangent');
      material.normalScale.set(1, -1);
      material.needsUpdate = true;
    }
  });
  scene.add(gltf.scene);
  scene.add(new THREE.AmbientLight(0xffffff, 0.35));
  const sun = new THREE.DirectionalLight(0xffffff, 2.6);
  sun.position.set(1, 1.2, 0.6);
  scene.add(sun);
  const camera = new THREE.PerspectiveCamera(35, 1, 0.1, 50);
  camera.position.set(2.2, 1.7, 2.6);
  camera.lookAt(0, 0, 0);
  renderer.render(scene, camera);
  const url = canvas.toDataURL('image/png');
  renderer.dispose();
  return url;
}

(window as unknown as { renderGlb: typeof render }).renderGlb = render;
