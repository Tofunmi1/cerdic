// Types of the texture generator: Material Maker node definitions (the vendored `.mmg` JSON, MIT) and the
// graph of nodes the engine compiles into one shader.

/** A parameter of a node definition: what it is, its limits and its default. */
export type ParamDef = {
  name: string;
  type: 'float' | 'enum' | 'boolean' | 'color' | 'size' | 'gradient' | 'curve' | string;
  label?: string;
  default?: unknown;
  min?: number;
  max?: number;
  step?: number;
  values?: { name: string; value: string }[];
};

export type InputDef = { name: string; type: string; default: string; label?: string; function?: boolean };

/** An output: its `type` key names the expression, for example `{ rgba: "...", type: "rgba" }`. */
export type OutputDef = { type?: string; [typeKey: string]: string | undefined };

export type ShaderModel = {
  name?: string;
  code?: string[];
  global?: string[] | string;
  instance?: string[] | string;
  includes?: string[];
  inputs?: InputDef[];
  outputs?: OutputDef[];
  parameters?: ParamDef[];
  shortdesc?: string;
  longdesc?: string;
};

/** One `.mmg` file: a node's name, its default parameter values and its shader model. */
export type NodeDef = { name: string; parameters?: Record<string, unknown>; shader_model: ShaderModel; type?: string };

/** The nodes available to a compile, by name (the file name without `.mmg`). */
export type Library = Record<string, NodeDef>;

/** A parameter value in a graph: a number, flag, string, colour, gradient or curve. */
export type ParamValue = number | boolean | string | { r: number; g: number; b: number; a: number } | Record<string, unknown>;

/** A node of a texture graph and what feeds its inputs. */
export type TexNode = {
  id: string;
  /** The library name of the node (`bricks`, `fbm`, `colorize`, ...). */
  type: string;
  params?: Record<string, ParamValue>;
  /** Input name to the node and output index that feeds it. */
  inputs?: Record<string, { node: string; output?: number }>;
};

export type TexGraph = { nodes: TexNode[]; output: { node: string; output?: number } };
