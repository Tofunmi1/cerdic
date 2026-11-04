// Baking: runs a compiled texture shader in WebGL2 and reads the picture back.
//
// A 2D shader is drawn over the unit square, so the picture is the texture in UV space. A 3D shader reads an
// object-space position per texel (`uPosition`), which `positionMap` makes by drawing a mesh's triangles in
// UV space with their positions as colour; the result is a texture that wraps the mesh with no seams in the
// pattern, whatever the unwrap looks like.
import type { Compiled } from './compile';
import { decodedPicture, rawPicture } from './outfit-images';

const VERTEX = `#version 300 es
out vec2 vUv;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  vUv = p;
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const MESH_VERTEX = `#version 300 es
layout(location = 0) in vec2 aUv;
layout(location = 1) in vec3 aPosition;
out vec3 vPosition;
void main() {
  vPosition = aPosition;
  gl_Position = vec4(aUv * 2.0 - 1.0, 0.0, 1.0);
}`;
const MESH_FRAGMENT = `#version 300 es
precision highp float;
in vec3 vPosition;
out vec4 fragColor;
void main() { fragColor = vec4(vPosition, 1.0); }`;

/** Fills texels next to the drawn ones with their neighbour's position, so filtering across a seam stays inside the pattern. */
const DILATE = `#version 300 es
precision highp float;
uniform sampler2D uSource;
in vec2 vUv;
out vec4 fragColor;
void main() {
  ivec2 c = ivec2(gl_FragCoord.xy);
  vec4 here = texelFetch(uSource, c, 0);
  if (here.a > 0.0) { fragColor = here; return; }
  ivec2 size = textureSize(uSource, 0);
  for (int dy = -1; dy <= 1; dy++) for (int dx = -1; dx <= 1; dx++) {
    ivec2 q = c + ivec2(dx, dy);
    if (q.x < 0 || q.y < 0 || q.x >= size.x || q.y >= size.y) continue;
    vec4 n = texelFetch(uSource, q, 0);
    if (n.a > 0.0) { fragColor = n; return; }
  }
  fragColor = vec4(0.0);
}`;

/**
 * Combines the position, smooth-normal and occlusion maps into the surface signal map, per texel: curvature (the mean
 * curvature, from how the normal changes across the surface), occlusion and slope. Where a texel's neighbours are not
 * the next bit of the same surface (a chart edge, a gap) the curvature is 0 rather than a jump.
 */
const SURFACE = `#version 300 es
precision highp float;
uniform sampler2D uP;
uniform sampler2D uN;
uniform sampler2D uA;
out vec4 fragColor;
void main() {
  ivec2 c = ivec2(gl_FragCoord.xy);
  ivec2 size = textureSize(uP, 0);
  vec4 p0 = texelFetch(uP, c, 0);
  if (p0.a <= 0.0) { fragColor = vec4(0.0); return; }
  vec3 n0 = texelFetch(uN, c, 0).xyz;
  float ao = texelFetch(uA, c, 0).x;
  float curvature = 0.0;
  ivec2 hi = size - 1;
  ivec2 ua = clamp(c + ivec2(1, 0), ivec2(0), hi), ub = clamp(c - ivec2(1, 0), ivec2(0), hi);
  ivec2 va = clamp(c + ivec2(0, 1), ivec2(0), hi), vb = clamp(c - ivec2(0, 1), ivec2(0), hi);
  vec4 pua = texelFetch(uP, ua, 0), pub = texelFetch(uP, ub, 0), pva = texelFetch(uP, va, 0), pvb = texelFetch(uP, vb, 0);
  if (pua.a > 0.0 && pub.a > 0.0 && pva.a > 0.0 && pvb.a > 0.0) {
    float la = length(pua.xyz - p0.xyz), lb = length(p0.xyz - pub.xyz), lc = length(pva.xyz - p0.xyz), ld = length(p0.xyz - pvb.xyz);
    bool joined = la > 1e-7 && lb > 1e-7 && lc > 1e-7 && ld > 1e-7 && max(la, lb) < 2.5 * min(la, lb) && max(lc, ld) < 2.5 * min(lc, ld);
    if (joined) {
      vec3 pu = (pua.xyz - pub.xyz) * 0.5, pv = (pva.xyz - pvb.xyz) * 0.5;
      vec3 nu = (texelFetch(uN, ua, 0).xyz - texelFetch(uN, ub, 0).xyz) * 0.5;
      vec3 nv = (texelFetch(uN, va, 0).xyz - texelFetch(uN, vb, 0).xyz) * 0.5;
      float g11 = dot(pu, pu), g22 = dot(pv, pv), g12 = dot(pu, pv);
      float det = g11 * g22 - g12 * g12;
      if (det > 1e-16) {
        // The shape operator S satisfies N_u = S P_u and N_v = S P_v; its trace is the sum over the inverse metric.
        float trace = (g22 * dot(nu, pu) - g12 * (dot(nu, pv) + dot(nv, pu)) + g11 * dot(nv, pv)) / det;
        curvature = 0.5 * trace;
      }
    }
  }
  fragColor = vec4(clamp(curvature * 0.05, -1.0, 1.0), ao, normalize(n0).y, 1.0);
}`;

/** How a baked colour map is finished for a style (see `solid.style`). */
export type FinishKind = 'painterly' | 'retro' | 'ink';

const FINISH_HEAD = `#version 300 es
precision highp float;
uniform sampler2D uSource;
out vec4 fragColor;
`;

/**
 * Painterly: a Kuwahara filter (radius 4). Each pixel takes the average colour of whichever of the four quadrants around it
 * varies least, which flattens texture into brush-sized patches and keeps the edges between them sharp.
 */
const PAINTERLY = FINISH_HEAD + `
const int R = 4;
void main() {
  ivec2 c = ivec2(gl_FragCoord.xy);
  ivec2 hi = textureSize(uSource, 0) - 1;
  vec3 best = vec3(0.0);
  float bestVariance = 1e9;
  for (int k = 0; k < 4; k++) {
    ivec2 low = ivec2((k & 1) == 0 ? -R : 0, (k & 2) == 0 ? -R : 0);
    vec3 sum = vec3(0.0), sum2 = vec3(0.0);
    for (int dy = 0; dy <= R; dy++) for (int dx = 0; dx <= R; dx++) {
      vec3 s = texelFetch(uSource, clamp(c + low + ivec2(dx, dy), ivec2(0), hi), 0).rgb;
      sum += s; sum2 += s * s;
    }
    float n = float((R + 1) * (R + 1));
    vec3 mean = sum / n;
    vec3 v = sum2 / n - mean * mean;
    float variance = v.r + v.g + v.b;
    if (variance < bestVariance) { bestVariance = variance; best = mean; }
  }
  fragColor = vec4(best, 1.0);
}`;

/** Retro: blocks of 1/128 of the map, each in a handful of colour levels per channel. */
const RETRO = FINISH_HEAD + `
void main() {
  ivec2 c = ivec2(gl_FragCoord.xy);
  ivec2 size = textureSize(uSource, 0);
  int block = max(size.x / 128, 1);
  vec3 s = texelFetch(uSource, clamp((c / block) * block + block / 2, ivec2(0), size - 1), 0).rgb;
  fragColor = vec4(floor(s * 4.0 + 0.5) / 4.0, 1.0);
}`;

/** Ink: the colour's brightness in three greys. */
const INK = FINISH_HEAD + `
void main() {
  vec3 s = texelFetch(uSource, ivec2(gl_FragCoord.xy), 0).rgb;
  float l = dot(s, vec3(0.299, 0.587, 0.114));
  float level = (floor(clamp(l, 0.0, 0.999) * 3.0) + 0.5) / 3.0;
  fragColor = vec4(vec3(level), 1.0);
}`;

const FINISH: Record<FinishKind, string> = { painterly: PAINTERLY, retro: RETRO, ink: INK };

export type PositionMap = { texture: WebGLTexture; size: number };

/** A texture shader kept compiled for drawing every frame (see `Baker.live`). */
export type LiveBake = {
  /** Draws the picture at `seconds` and returns its RGBA8 pixels (the same array every call). */
  frame(seconds: number): Uint8Array;
  /** Frees the GPU objects. */
  dispose(): void;
};

/** A WebGL2 context that bakes textures. Create one and reuse it; call `dispose` when done. */
export class Baker {
  readonly gl: WebGL2RenderingContext;

  constructor(canvas: OffscreenCanvas | HTMLCanvasElement) {
    const gl = canvas.getContext('webgl2', { preserveDrawingBuffer: true, antialias: false, alpha: true }) as WebGL2RenderingContext | null;
    if (!gl) throw new Error('baking needs WebGL2');
    if (!gl.getExtension('EXT_color_buffer_float')) throw new Error('baking needs the EXT_color_buffer_float extension');
    this.gl = gl;
  }

  private program(vertex: string, fragment: string): WebGLProgram {
    const { gl } = this;
    const compile = (type: number, source: string) => {
      const shader = gl.createShader(type) as WebGLShader;
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        const log = gl.getShaderInfoLog(shader) ?? '';
        throw new Error(`texture shader failed to compile: ${log.split('\n').find((l) => l.includes('ERROR')) ?? log}`);
      }
      return shader;
    };
    const vertexShader = compile(gl.VERTEX_SHADER, vertex);
    let fragmentShader: WebGLShader;
    try {
      fragmentShader = compile(gl.FRAGMENT_SHADER, fragment);
    } catch (error) {
      gl.deleteShader(vertexShader);
      throw error;
    }
    const program = gl.createProgram() as WebGLProgram;
    gl.attachShader(program, vertexShader);
    gl.attachShader(program, fragmentShader);
    gl.linkProgram(program);
    gl.detachShader(program, vertexShader);
    gl.detachShader(program, fragmentShader);
    gl.deleteShader(vertexShader);
    gl.deleteShader(fragmentShader);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      const error = gl.getProgramInfoLog(program);
      gl.deleteProgram(program);
      throw new Error(`texture shader failed to link: ${error}`);
    }
    return program;
  }

  private target(size: number, internal: number, format: number, type: number): { texture: WebGLTexture; framebuffer: WebGLFramebuffer } {
    const { gl } = this;
    const texture = gl.createTexture() as WebGLTexture;
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, internal, size, size, 0, format, type, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    const framebuffer = gl.createFramebuffer() as WebGLFramebuffer;
    gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texture, 0);
    return { texture, framebuffer };
  }

  /** The object-space position of every texel a triangle covers; `uvs` has two floats and `positions` three per corner. */
  positionMap(uvs: Float32Array, positions: Float32Array, size: number, padding = 8): PositionMap {
    const { gl } = this;
    const corners = uvs.length / 2;
    const data = new Float32Array(corners * 5);
    for (let i = 0; i < corners; i++) data.set([uvs[i * 2], uvs[i * 2 + 1], positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]], i * 5);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 20, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 20, 8);
    let a = this.target(size, gl.RGBA32F, gl.RGBA, gl.FLOAT);
    gl.viewport(0, 0, size, size);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.disable(gl.CULL_FACE);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.BLEND);
    const meshProgram = this.program(MESH_VERTEX, MESH_FRAGMENT);
    gl.useProgram(meshProgram);
    gl.drawArrays(gl.TRIANGLES, 0, corners);
    gl.deleteProgram(meshProgram);
    gl.deleteVertexArray(vao);
    gl.deleteBuffer(buffer);
    if (padding > 0) {
      const dilate = this.program(VERTEX, DILATE);
      gl.useProgram(dilate);
      gl.uniform1i(gl.getUniformLocation(dilate, 'uSource'), 0);
      for (let i = 0; i < padding; i++) {
        const b = this.target(size, gl.RGBA32F, gl.RGBA, gl.FLOAT);
        gl.bindFramebuffer(gl.FRAMEBUFFER, b.framebuffer);
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, a.texture);
        gl.drawArrays(gl.TRIANGLES, 0, 3);
        gl.deleteFramebuffer(a.framebuffer);
        gl.deleteTexture(a.texture);
        a = b;
      }
      gl.deleteProgram(dilate);
    }
    gl.deleteFramebuffer(a.framebuffer);
    return { texture: a.texture, size };
  }

  /** Finishes a baked colour map for a style: painted over, pixelated or inked. Returns RGBA8 pixels in the same layout. */
  finish(pixels: Uint8Array, size: number, kind: FinishKind): Uint8Array {
    const { gl } = this;
    const source = gl.createTexture() as WebGLTexture;
    gl.bindTexture(gl.TEXTURE_2D, source);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, size, size, 0, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    const target = this.target(size, gl.RGBA8, gl.RGBA, gl.UNSIGNED_BYTE);
    gl.viewport(0, 0, size, size);
    gl.disable(gl.BLEND);
    const program = this.program(VERTEX, FINISH[kind]);
    gl.useProgram(program);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, source);
    gl.uniform1i(gl.getUniformLocation(program, 'uSource'), 0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    const out = new Uint8Array(size * size * 4);
    gl.readPixels(0, 0, size, size, gl.RGBA, gl.UNSIGNED_BYTE, out);
    gl.deleteFramebuffer(target.framebuffer);
    gl.deleteTexture(target.texture);
    gl.deleteTexture(source);
    gl.deleteProgram(program);
    return out;
  }

  /** The surface signal map (curvature, occlusion, slope per texel) from the position, normal and occlusion maps of one mesh. */
  surfaceMap(position: PositionMap, normal: PositionMap, occlusion: PositionMap, size: number): PositionMap {
    const { gl } = this;
    const target = this.target(size, gl.RGBA32F, gl.RGBA, gl.FLOAT);
    gl.viewport(0, 0, size, size);
    gl.disable(gl.BLEND);
    const program = this.program(VERTEX, SURFACE);
    gl.useProgram(program);
    [[position, 'uP'], [normal, 'uN'], [occlusion, 'uA']].forEach(([map, name], unit) => {
      gl.activeTexture(gl.TEXTURE0 + unit);
      gl.bindTexture(gl.TEXTURE_2D, (map as PositionMap).texture);
      gl.uniform1i(gl.getUniformLocation(program, name as string), unit);
    });
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.activeTexture(gl.TEXTURE0);
    gl.deleteFramebuffer(target.framebuffer);
    gl.deleteProgram(program);
    return { texture: target.texture, size };
  }

  /**
   * Runs a compiled shader and returns RGBA8 pixels, row 0 at v = 0. A shader that reads the mesh needs its position map
   * (3D textures and `surface_height`) and its surface signal map (`surface_convex`, `surface_cavity`, ...).
   */
  bake(compiled: Compiled, size: number, positions?: PositionMap, surface?: PositionMap): Uint8Array {
    const { gl } = this;
    const needsPosition = compiled.needs?.position ?? compiled.space === '3d';
    const needsSurface = compiled.needs?.surface ?? false;
    if (needsPosition && !positions) throw new Error('this texture reads the mesh (3D texture or surface node): it needs the mesh to be baked onto');
    if (needsSurface && !surface) throw new Error('this texture uses surface nodes: it needs the mesh\'s surface signals');
    const program = this.program(VERTEX, compiled.source);
    const target = this.target(size, gl.RGBA8, gl.RGBA, gl.UNSIGNED_BYTE);
    gl.viewport(0, 0, size, size);
    gl.disable(gl.BLEND);
    gl.useProgram(program);
    if (needsPosition && positions) {
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, positions.texture);
      gl.uniform1i(gl.getUniformLocation(program, 'uPosition'), 0);
    }
    if (needsSurface && surface) {
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, surface.texture);
      gl.uniform1i(gl.getUniformLocation(program, 'uSurface'), 1);
      gl.activeTexture(gl.TEXTURE0);
    }
    // Pictures (`image` nodes) take the texture units after the mesh maps.
    (compiled.needs?.images ?? []).forEach((name, i) => {
      const texture = this.picture(name); // first: uploading a picture changes the active texture unit
      gl.activeTexture(gl.TEXTURE2 + i);
      gl.bindTexture(gl.TEXTURE_2D, texture);
      gl.uniform1i(gl.getUniformLocation(program, `uImg_${name}`), 2 + i);
    });
    gl.activeTexture(gl.TEXTURE0);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    const pixels = new Uint8Array(size * size * 4);
    gl.readPixels(0, 0, size, size, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    gl.deleteFramebuffer(target.framebuffer);
    gl.deleteTexture(target.texture);
    gl.deleteProgram(program);
    return pixels;
  }

  /**
   * A shader kept compiled and drawn again on demand, for textures that move: compiled once, then each `frame(seconds)` sets the clock (`uTime`), draws on the GPU and reads the
   * picture back as RGBA8 (row 0 at v = 0). The returned array is reused by the next frame, so copy it if it must outlive that.
   */
  live(compiled: Compiled, size: number, positions?: PositionMap, surface?: PositionMap): LiveBake {
    const { gl } = this;
    const needsPosition = compiled.needs?.position ?? compiled.space === '3d';
    const needsSurface = compiled.needs?.surface ?? false;
    if (needsPosition && !positions) throw new Error('this texture reads the mesh (3D texture or surface node): it needs the mesh to be baked onto');
    if (needsSurface && !surface) throw new Error('this texture uses surface nodes: it needs the mesh\'s surface signals');
    const program = this.program(VERTEX, compiled.source);
    const target = this.target(size, gl.RGBA8, gl.RGBA, gl.UNSIGNED_BYTE);
    const images = (compiled.needs?.images ?? []).map((name) => ({ name, texture: this.picture(name) }));
    const out = new Uint8Array(size * size * 4);
    const frame = (seconds: number): Uint8Array => {
      gl.bindFramebuffer(gl.FRAMEBUFFER, target.framebuffer);
      gl.viewport(0, 0, size, size);
      gl.disable(gl.BLEND);
      gl.useProgram(program);
      gl.uniform1f(gl.getUniformLocation(program, 'uTime'), seconds);
      if (needsPosition && positions) {
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, positions.texture);
        gl.uniform1i(gl.getUniformLocation(program, 'uPosition'), 0);
      }
      if (needsSurface && surface) {
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, surface.texture);
        gl.uniform1i(gl.getUniformLocation(program, 'uSurface'), 1);
      }
      images.forEach(({ name, texture }, i) => {
        gl.activeTexture(gl.TEXTURE2 + i);
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.uniform1i(gl.getUniformLocation(program, `uImg_${name}`), 2 + i);
      });
      gl.activeTexture(gl.TEXTURE0);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      gl.readPixels(0, 0, size, size, gl.RGBA, gl.UNSIGNED_BYTE, out);
      return out;
    };
    const dispose = () => {
      gl.deleteFramebuffer(target.framebuffer);
      gl.deleteTexture(target.texture);
      gl.deleteProgram(program);
    };
    return { frame, dispose };
  }

  disposePositionMap(map: PositionMap) {
    this.gl.deleteTexture(map.texture);
  }

  private pictures = new Map<string, WebGLTexture>();

  /** The decoded picture `name` as a repeating, mipmapped texture (uploaded once per baker); row 0 of the picture is at v = 1, as UVs have it. */
  private picture(name: string): WebGLTexture {
    const cached = this.pictures.get(name);
    if (cached) return cached;
    const { gl } = this;
    const texture = gl.createTexture() as WebGLTexture;
    gl.activeTexture(gl.TEXTURE0 + 15);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    const made = rawPicture(name);
    if (made) {
      // A picture the engine made (a simulation): bytes with row 0 at v = 0 already, so no flip.
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, made.width, made.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, made.pixels);
    } else {
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, gl.RGBA, gl.UNSIGNED_BYTE, decodedPicture(name));
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    }
    gl.generateMipmap(gl.TEXTURE_2D);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.REPEAT);
    this.pictures.set(name, texture);
    return texture;
  }

  /** Releases the offscreen WebGL context when a one-shot export baker is no longer needed. */
  dispose() {
    for (const texture of this.pictures.values()) this.gl.deleteTexture(texture);
    this.pictures.clear();
    this.gl.getExtension('WEBGL_lose_context')?.loseContext();
  }
}

/** Encodes RGBA8 pixels as a PNG (browser). */
export async function encodePng(pixels: Uint8Array, size: number): Promise<Uint8Array> {
  const canvas = new OffscreenCanvas(size, size);
  const context = canvas.getContext('2d');
  if (!context) throw new Error('no 2d canvas to encode a PNG');
  // putImageData is not premultiplied; textures are opaque, so the colours survive.
  context.putImageData(new ImageData(new Uint8ClampedArray(pixels), size, size), 0, 0);
  const blob = await canvas.convertToBlob({ type: 'image/png' });
  return new Uint8Array(await blob.arrayBuffer());
}

/**
 * A tangent-space normal map from a height map (the red channel). `strength` scales the slope; the map wraps,
 * since procedural textures tile. Row order matches `pixels`.
 */
export function normalFromHeight(pixels: Uint8Array, size: number, strength: number): Uint8Array {
  const out = new Uint8Array(size * size * 4);
  const h = (x: number, y: number) => pixels[((y + size) % size * size + ((x + size) % size)) * 4] / 255;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const dx = (h(x + 1, y) - h(x - 1, y)) * strength * size * 0.5;
      const dy = (h(x, y + 1) - h(x, y - 1)) * strength * size * 0.5;
      const length = Math.hypot(dx, dy, 1);
      const o = (y * size + x) * 4;
      // glTF normal maps are +Y up in texture space, and up is the direction of falling v (row 0 is the top).
      // A height that grows with the row index therefore tilts the normal up: green is +dy, red is -dx.
      out[o] = Math.round((-dx / length * 0.5 + 0.5) * 255);
      out[o + 1] = Math.round((dy / length * 0.5 + 0.5) * 255);
      out[o + 2] = Math.round((1 / length * 0.5 + 0.5) * 255);
      out[o + 3] = 255;
    }
  }
  return out;
}
