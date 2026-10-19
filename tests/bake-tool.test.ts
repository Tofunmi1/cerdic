import { expect, test } from 'bun:test';
import { bakeParts, type McpHost } from '../src/engine/mcp-bridge';
import type { MeshPart } from '../src/engine/worker-types';

type Call = { low: Float32Array; li: Uint32Array; uv: Float32Array; high: Float32Array; hi: Uint32Array; size: number; maps: string; cage: number; distance: number; rays: number; bias: number };
const rgbaFor = (size: number) => { let raw=''; const bytes=new Uint8Array(size*size*4).fill(255); for(let i=0;i<bytes.length;i+=0x8000) raw+=String.fromCharCode(...bytes.subarray(i,i+0x8000)); return btoa(raw); };
function rig() {
  const calls: Call[] = [];
  const runtime = { bake_maps(...args: [Float32Array, Uint32Array, Float32Array, Float32Array, Uint32Array, number, string, number, number, number, number]) {
    const [low, li, uv, high, hi, size, maps, cage, distance, rays, bias] = args;
    calls.push({ low, li, uv, high, hi, size, maps, cage, distance, rays, bias });
    const item = { covered: size * size, rgbaBase64: rgbaFor(size) };
    return JSON.stringify({ ok: true, size, normal: maps !== 'ao' ? item : null, ao: maps !== 'normal' ? item : null });
  } };
  const quad = (name: string, z=0): MeshPart => ({ node: name, name, volume: 0, positions: new Float32Array([0,0,z, 3,0,z, 3,4,z, 0,4,z]), indices: new Uint32Array([0,1,2,0,2,3]), uvs: new Float32Array([0,0,1,0,1,1,0,0,1,1,0,1]), occlusion: new Float32Array(0) });
  const host = { ready: true, doc: {}, evaluation: { parts: [quad('low'), quad('high', .1)], state: {} }, runtime } as unknown as McpHost;
  return { host, calls };
}
test('defaults scale from diagonal and select default work settings', async () => {
  const { host, calls } = rig(); await bakeParts(host, { low: 'low', high: 'high' }); const d=5;
  expect(calls[0]).toMatchObject({ cage: .05*d, distance: .1*d, bias: .001*d, rays: 64, size: 512 });
});
test('missing part reports names on screen', async () => { const { host }=rig(); expect(await bakeParts(host,{low:'missing',high:'high'})).toMatchObject({ok:false,error: expect.stringContaining('low, high')}); });
test('missing low UVs asks for unwrap', async () => { const {host}=rig(); host.evaluation!.parts[0].uvs=undefined; expect(await bakeParts(host,{low:'low',high:'high'})).toMatchObject({ok:false,error:expect.stringContaining('poly.unwrap_uv')}); });
test('PNG signatures and dimensions are valid; ao selection returns only ao', async () => {
 const {host}=rig(); const result=await bakeParts(host,{low:'low',high:'high',size:64,maps:['ao']}) as { maps: Record<string,string> };
 expect(Object.keys(result.maps)).toEqual(['ao']); const bytes=Uint8Array.from(atob(result.maps.ao),c=>c.charCodeAt(0));
 expect([...bytes.slice(0,8)]).toEqual([137,80,78,71,13,10,26,10]); const view=new DataView(bytes.buffer); expect(view.getUint32(16)).toBe(64); expect(view.getUint32(20)).toBe(64);
});
test('the returned maps are compressed: a flat 64 x 64 map is far smaller than its raw size', async () => {
 const {host}=rig(); const result=await bakeParts(host,{low:'low',high:'high',size:64,maps:['normal']}) as { maps: Record<string,string> };
 const bytes=Uint8Array.from(atob(result.maps.normal),c=>c.charCodeAt(0));
 expect(bytes.length).toBeLessThan(64*64*4*0.1);
});
test('out of range size and rays name their allowed ranges', async () => {
 const {host}=rig(); expect(await bakeParts(host,{low:'low',high:'high',size:8})).toMatchObject({error:expect.stringContaining('64 to 2048')});
 expect(await bakeParts(host,{low:'low',high:'high',rays:500})).toMatchObject({error:expect.stringContaining('1 to 256')});
});
