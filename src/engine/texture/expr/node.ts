// The `expr` texture node: a short shader expression program (see `parse.ts`) that draws the texture.
//
//   n = expr(code="a = fbm(p * 6); out = hsv(0.08, 0.6, 0.3 + a * 0.7)")
//
// A graph node of type `expr` carries its code in the `code` parameter. Before the graph is compiled, each one becomes
// a node of its own in the library whose GLSL is the emitted function, so the compiler sees a normal node and it
// composes with every other texture node (feed it to `colorize`, `blend`, a channel, ...).
import { emitGlsl, HELPERS } from './glsl';
import { parseExpr } from './parse';
import type { Library, NodeDef, TexGraph } from '../types';

export const EXPR_TYPE = 'expr';

/** What a texture program sees of the `expr` node: one string parameter and no inputs. */
export const EXPR_NODE: NodeDef = {
  name: EXPR_TYPE,
  shader_model: {
    name: EXPR_TYPE,
    shortdesc: 'Shader expression',
    longdesc: 'A few lines of the shader expression language (p, q, t, vec2-vec4, sin, mix, fbm, snoise, voronoi, stretch, warp, hsv, ... and for i in 0..N loops) that draw the texture; the last line is out = a float, vec3 or vec4.',
    inputs: [],
    outputs: [{ type: 'rgba', rgba: '' }],
    parameters: [{ name: 'code', type: 'string', label: 'Code', default: 'out = vec3(p, 0)' }],
  },
};

/** The source of a shader expression with its errors turned into one readable message. */
function compile(id: string, code: unknown): { functions: string; body: string } {
  if (typeof code !== 'string' || !code.trim()) throw new Error(`expr node "${id}" needs code="...", for example code="out = vec3(p, 0)"`);
  const parsed = parseExpr(code);
  if ('error' in parsed) {
    const { code: kind, message, suggestion, line, column } = parsed.error;
    throw new Error(`expr node "${id}" (${kind}, line ${line}, column ${column}): ${message}. ${suggestion}`);
  }
  return emitGlsl(parsed.program);
}

/**
 * Replaces each `expr` node of the graph with a generated library node. Returns the library and graph to compile,
 * and the helper functions to add once to the shader (empty when the graph has no `expr` node).
 */
export function expandExpressions(library: Library, graph: TexGraph): { library: Library; graph: TexGraph; helpers: string } {
  if (!graph.nodes.some((node) => node.type === EXPR_TYPE)) return { library, graph, helpers: '' };
  const expanded: Library = { ...library };
  const nodes = graph.nodes.map((node) => {
    if (node.type !== EXPR_TYPE) return node;
    const { body } = compile(node.id, node.params?.code);
    const type = `expr_${node.id.replace(/\W/g, '_')}`;
    expanded[type] = {
      name: type,
      shader_model: {
        name: type,
        inputs: [],
        outputs: [{ type: 'rgba', rgba: '$(name)_expr($uv)' }],
        instance: [
          'vec4 $(name)_expr(vec2 uv) {',
          '  vec2 p = uv;',
          '  vec2 q = (uv - 0.5) * 2.0;',
          // The clock only when the expression reads it: a node that does not stays still, so its program is baked once rather than drawn live.
          /\bt\b/.test(body) ? '  float t = uTime;' : '  float t = 0.0;',
          body,
          '  return result;',
          '}',
        ],
      },
    };
    const { code: _code, ...params } = node.params ?? {};
    return { ...node, type, params };
  });
  return { library: expanded, graph: { ...graph, nodes }, helpers: HELPERS };
}
