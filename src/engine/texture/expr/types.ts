// The syntax tree of a shader expression program, and the typed errors the parser reports.

/** Number of components of a value: 1 is a float, 2 to 4 are vec2 to vec4. */
export type Size = 1 | 2 | 3 | 4;

export type Expr =
  | { kind: 'num'; value: number; size: 1 }
  | { kind: 'name'; name: string; size: Size }
  | { kind: 'unary'; op: '-'; value: Expr; size: Size }
  | { kind: 'binary'; op: string; left: Expr; right: Expr; size: Size }
  | { kind: 'call'; name: string; args: Expr[]; size: Size }
  | { kind: 'swizzle'; value: Expr; field: string; size: Size };

export type Stmt =
  | { kind: 'assign'; name: string; value: Expr }
  | { kind: 'loop'; variable: string; count: number; body: Stmt[] };

export type Program = { statements: Stmt[]; output: Expr };

export type ErrorCode =
  | 'expr.syntax'
  | 'expr.unknown_name'
  | 'expr.type'
  | 'expr.limit'
  | 'expr.no_out'
  | 'expr.loop';

/** An error with the fix to apply, and where it is (1-based). */
export type ParseError = {
  code: ErrorCode;
  message: string;
  suggestion: string;
  line: number;
  column: number;
};

/** Thrown inside the parser; `parseExpr` turns it into a `ParseError`. */
export class Failure extends Error {
  constructor(
    public code: ErrorCode,
    message: string,
    public suggestion: string,
    public pos: number,
  ) {
    super(message);
  }
}
