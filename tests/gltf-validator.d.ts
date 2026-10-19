// The Khronos glTF validator ships no types; this declares the part the export tests use.
declare module 'gltf-validator' {
  export type ValidationReport = {
    issues: { numErrors: number; numWarnings: number; numInfos: number; numHints: number; messages: { code: string; message: string; pointer?: string; severity: number }[] };
    info: { totalVertexCount: number; totalTriangleCount: number; materialCount: number; meshCount?: number; drawCallCount: number };
  };
  const validator: { validateBytes(data: Uint8Array): Promise<ValidationReport> };
  export default validator;
}
