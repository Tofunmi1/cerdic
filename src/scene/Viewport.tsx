import { useEffect, useMemo, useRef } from "react";
import type { RefObject } from "react";
import { Canvas } from "@react-three/fiber";
import {
  ContactShadows,
  Edges,
  Grid,
  OrbitControls,
  TransformControls,
} from "@react-three/drei";
import { createXRStore, useXR, XR, XROrigin } from "@react-three/xr";
import { Mesh, DoubleSide } from "three";
import type { SceneCommand, SceneObject, Vec3 } from "../../shared/commands";
import { createGeometry } from "./factories";

export const xrStore = createXRStore({ emulate: false, controller: true });
export type Tool = "translate" | "rotate" | "scale";
interface Props {
  objects: SceneObject[];
  selectedId: string | null;
  tool: Tool;
  onSelect: (id: string | null) => void;
  onCommands: (commands: SceneCommand[]) => void;
}

function RenderObject({
  object,
  selected,
  tool,
  onSelect,
  onCommands,
  dragRef,
}: { object: SceneObject; selected: boolean } & Pick<
  Props,
  "tool" | "onSelect" | "onCommands"
> & { dragRef: RefObject<boolean> }) {
  const ref = useRef<Mesh>(null!);
  const geometry = useMemo(
    () => createGeometry(object.factory),
    [object.factory],
  );
  const inVR = useXR((state) => state.session != null);
  useEffect(() => () => geometry.dispose(), [geometry]);
  const mesh = (
    <mesh
      ref={ref}
      name={object.id}
      geometry={geometry}
      position={object.position}
      rotation={object.rotation}
      scale={object.scale}
      castShadow
      receiveShadow
      onClick={(event) => {
        event.stopPropagation();
        onSelect(object.id);
      }}
    >
      <meshStandardMaterial
        color={object.color}
        roughness={0.33}
        metalness={0.08}
        side={object.factory === "plane" ? DoubleSide : undefined}
        emissive={selected ? "#635b8e" : "#000000"}
        emissiveIntensity={0.035}
      />
      {selected && <Edges threshold={25} color="#5e577e" />}
    </mesh>
  );
  if (!selected || inVR) return mesh;
  return (
    <TransformControls
      object={ref}
      mode={tool}
      size={0.75}
      onMouseDown={() => {
        dragRef.current = true;
      }}
      onMouseUp={() => {
        setTimeout(() => {
          dragRef.current = false;
        }, 0);
        const item = ref.current;
        if (!item) return;
        const patch = {
          position: item.position.toArray() as Vec3,
          rotation: [item.rotation.x, item.rotation.y, item.rotation.z] as Vec3,
          scale: item.scale
            .toArray()
            .map((n) => Math.max(0.02, Math.min(100, n))) as Vec3,
        };
        // Restore the preview before applying the validated document change.
        item.position.fromArray(object.position);
        item.rotation.set(...object.rotation);
        item.scale.fromArray(object.scale);
        onCommands([
          {
            type: "update",
            id: object.id,
            patch,
          },
        ]);
      }}
    >
      {mesh}
    </TransformControls>
  );
}

function Scene(props: Props & { dragRef: RefObject<boolean> }) {
  const inVR = useXR((state) => state.session != null);
  return (
    <>
      <color attach="background" args={["#dde5ed"]} />
      <fog attach="fog" args={["#dde5ed", 22, 55]} />
      <ambientLight intensity={1.1} />
      <hemisphereLight args={["#eff5ff", "#8e8b99", 1.2]} />
      <directionalLight
        position={[3, 8, 5]}
        intensity={2.4}
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-camera-left={-12}
        shadow-camera-right={12}
        shadow-camera-top={12}
        shadow-camera-bottom={-12}
        shadow-normalBias={0.04}
      />
      <mesh
        rotation={[-Math.PI / 2, 0, 0]}
        position={[0, -0.04, 0]}
        receiveShadow
        onClick={() => props.onSelect(null)}
      >
        <planeGeometry args={[200, 200]} />
        <meshStandardMaterial color="#d6dfe8" roughness={1} />
      </mesh>
      <Grid
        position={[0, 0.008, 0]}
        args={[30, 30]}
        cellSize={1}
        sectionSize={5}
        cellColor="#a5b3c2"
        sectionColor="#91a3b5"
        cellThickness={0.45}
        sectionThickness={0.65}
        fadeDistance={24}
        fadeStrength={2}
        infiniteGrid
      />
      {props.objects.map((object) => (
        <RenderObject
          key={object.id}
          object={object}
          selected={object.id === props.selectedId}
          {...props}
        />
      ))}
      <ContactShadows
        position={[0, 0.015, 0]}
        opacity={0.22}
        scale={20}
        blur={2.5}
        far={5}
        resolution={256}
      />
      <XROrigin position={[0, 0, 5]} />
      <OrbitControls
        makeDefault
        enabled={!inVR}
        target={[0, 0.75, 0]}
        minDistance={2}
        maxDistance={35}
        maxPolarAngle={Math.PI / 2 - 0.02}
      />
    </>
  );
}
export default function Viewport(props: Props) {
  const dragRef = useRef(false);
  const select = (id: string | null) => {
    if (!dragRef.current) props.onSelect(id);
  };
  return (
    <Canvas
      shadows
      camera={{ position: [6, 4.8, 8], fov: 40 }}
      dpr={[1, 2]}
      onPointerMissed={() => select(null)}
      fallback={
        <div className="boot-error">
          This browser needs WebGL to render the scene.
        </div>
      }
    >
      <XR store={xrStore}>
        <Scene {...props} onSelect={select} dragRef={dragRef} />
      </XR>
    </Canvas>
  );
}
