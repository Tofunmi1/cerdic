// Parses and type-checks a shader expression program (see engine/docs/handoff-codex-shader-expr.md for the language).
import { MAX_IDENTIFIER, MAX_LOOP_BOUND, MAX_LOOP_DEPTH, MAX_LOOP_PRODUCT, MAX_NODES, MAX_SOURCE } from './limits';
import { FUNCTIONS, nearest } from './signatures';
import { RESERVED, tokenize, type Tok } from './tokens';
import { Failure, type Expr, type ParseError, type Program, type Size, type Stmt } from './types';

export type { Expr, ParseError, Program, Size, Stmt } from './types';

const BUILTINS: [string, Size][] = [['p', 2], ['q', 2], ['t', 1], ['PI', 1], ['TAU', 1]];
const NAME = /^[a-z][a-z0-9_]*$/;
const SWIZZLE_SETS = ['xyzw', 'rgba'];

/** Operator precedence; anything else ends an expression. */
const PRECEDENCE: Record<string, number> = { '+': 10, '-': 10, '*': 20, '/': 20, '%': 20 };
const UNARY_PRECEDENCE = 30;

class Parser {
  private tokens: Tok[];
  private at = 0;
  private nodes = 0;
  private variables = new Map<string, Size>(BUILTINS);
  private output: Expr | undefined;

  constructor(private source: string) {
    this.tokens = tokenize(source);
  }

  program(): Program {
    const statements = this.block(0, 1, false);
    if (!this.output) {
      throw this.fail('expr.no_out', 'The program never assigns out', 'End the program with a line such as: out = vec3(p, 0)', this.source.length);
    }
    return { statements, output: this.output };
  }

  // ---- statements

  /** Statements until the closing brace of a loop (inside one) or the end of the source. */
  private block(depth: number, product: number, inLoop: boolean): Stmt[] {
    const statements: Stmt[] = [];
    while (this.at < this.tokens.length && !(inLoop && this.peek('}'))) {
      if (this.peek(';')) {
        this.take();
        continue;
      }
      const statement = this.peek('for') ? this.loop(depth, product) : this.assignment(inLoop);
      if (statement) statements.push(statement);
      if (this.at < this.tokens.length && !this.peek(';') && !this.peek('}')) {
        throw this.fail('expr.syntax', 'Expected the end of the statement', 'Put each statement on its own line, or separate them with ;');
      }
    }
    return statements;
  }

  /** `name = expr`; the `out = expr` line is recorded as the program's output and returns nothing. */
  private assignment(inLoop: boolean): Stmt | undefined {
    const name = this.take();
    if (RESERVED.has(name.value)) {
      throw this.fail('expr.syntax', `'${name.value}' is not part of this language`, 'There is no if, while, or declarations: write name = expression, or for i in 0..N { }.', name.pos);
    }
    this.checkName(name);
    this.expect('=', 'Write name = expression.');
    const value = this.expression(0);
    this.node();
    if (name.value === 'out') return this.setOutput(name, value, inLoop);
    if (this.output) throw this.fail('expr.no_out', 'out must be the last statement', 'Move the out line to the end.', name.pos);
    if (BUILTINS.some(([builtin]) => builtin === name.value)) {
      throw this.fail('expr.syntax', `'${name.value}' is built in and cannot be assigned`, 'Pick another variable name.', name.pos);
    }
    const known = this.variables.get(name.value);
    if (known !== undefined && known !== value.size) {
      throw this.fail('expr.type', `'${name.value}' was a ${label(known)} and cannot become a ${label(value.size)}`, 'Use a new variable name for a value of another size.', name.pos);
    }
    this.variables.set(name.value, value.size);
    return { kind: 'assign', name: name.value, value };
  }

  private setOutput(name: Tok, value: Expr, inLoop: boolean): undefined {
    if (inLoop) throw this.fail('expr.no_out', 'out cannot be assigned inside a loop', 'Assign out once, after the loops.', name.pos);
    if (this.output) throw this.fail('expr.no_out', 'out is assigned twice', 'Keep a single out line, at the end.', name.pos);
    if (value.size === 2) throw this.fail('expr.type', 'out must be a float, vec3 or vec4', 'Use vec3(p, 0) or vec4(p, 0, 1) for a vec2.', name.pos);
    this.output = value;
    return undefined;
  }

  /** `for i in 0..N { ... }` with a literal bound. Names first assigned inside are local to the loop. */
  private loop(depth: number, product: number): Stmt {
    const start = this.take();
    if (depth >= MAX_LOOP_DEPTH) {
      throw this.fail('expr.loop', 'Loops nest too deeply', `Nest at most ${MAX_LOOP_DEPTH} loops; merge the inner work into one loop.`, start.pos);
    }
    const variable = this.take();
    if (!variable || !NAME.test(variable.value) || this.variables.has(variable.value)) {
      throw this.fail('expr.loop', 'The loop needs a new lowercase variable name', 'Write for i in 0..8 { ... } with a name not used before.', variable?.pos);
    }
    this.expectLoop('in');
    this.expectLoop('0');
    this.expectLoop('..');
    const bound = this.take();
    const count = Number(bound?.value);
    if (!bound || !/^\d+$/.test(bound.value) || count < 1 || count > MAX_LOOP_BOUND || product * count > MAX_LOOP_PRODUCT) {
      throw this.fail('expr.loop', 'The loop bound is out of range', `Use a whole number from 1 to ${MAX_LOOP_BOUND}; nested loops may run at most ${MAX_LOOP_PRODUCT} times in total.`, bound?.pos);
    }
    this.expectLoop('{');
    const before = new Set(this.variables.keys());
    this.variables.set(variable.value, 1);
    const body = this.block(depth + 1, product * count, true);
    this.expectLoop('}');
    for (const name of [...this.variables.keys()]) if (!before.has(name)) this.variables.delete(name);
    return { kind: 'loop', variable: variable.value, count, body };
  }

  // ---- expressions

  private expression(minimum: number): Expr {
    let left = this.primary();
    for (;;) {
      if (this.peek('.')) {
        left = this.swizzle(left);
        continue;
      }
      const operator = this.tokens[this.at]?.value;
      const precedence = operator === undefined ? undefined : PRECEDENCE[operator];
      if (precedence === undefined || precedence < minimum) return left;
      this.take();
      const right = this.expression(precedence + 1);
      this.node();
      left = { kind: 'binary', op: operator!, left, right, size: this.joined(left.size, right.size) };
    }
  }

  private primary(): Expr {
    const token = this.take();
    if (!token) throw this.fail('expr.syntax', 'The expression ends too early', 'Finish the expression.', this.source.length);
    this.node();
    if (token.value === '-') {
      const value = this.expression(UNARY_PRECEDENCE);
      return { kind: 'unary', op: '-', value, size: value.size };
    }
    if (token.value === '(') {
      const inner = this.expression(0);
      this.expect(')', 'Close the parenthesis.');
      return inner;
    }
    if (/^[0-9.]/.test(token.value)) return { kind: 'num', value: Number(token.value), size: 1 };
    if (/^[A-Za-z_]/.test(token.value)) return this.peek('(') ? this.call(token) : this.name(token);
    throw this.fail('expr.syntax', `Unexpected '${token.value}'`, 'Expected a number, a variable, a function call or ( ).', token.pos);
  }

  private name(token: Tok): Expr {
    if (RESERVED.has(token.value)) {
      throw this.fail('expr.syntax', `'${token.value}' is not part of this language`, 'There is no if, while, or declarations: write name = expression, or for i in 0..N { }.', token.pos);
    }
    const size = this.variables.get(token.value);
    if (size === undefined) throw this.fail('expr.unknown_name', `Unknown name '${token.value}'`, 'Assign the variable on an earlier line, or use p, q, t, PI or TAU.', token.pos);
    return { kind: 'name', name: token.value, size };
  }

  private call(token: Tok): Expr {
    this.take();
    const args: Expr[] = [];
    while (!this.peek(')')) {
      args.push(this.expression(0));
      if (this.peek(',')) this.take();
      else break;
    }
    this.expect(')', 'Close the call with ).');
    const rule = Object.hasOwn(FUNCTIONS, token.value) ? FUNCTIONS[token.value] : undefined;
    if (!rule) {
      const near = nearest(token.value);
      throw this.fail('expr.unknown_name', `Unknown function '${token.value}'`, near ? `Did you mean ${near}()?` : 'Use a GLSL math function (sin, mix, smoothstep, length, ...) or hash, noise, snoise, fbm, hsv, rot.', token.pos);
    }
    try {
      return { kind: 'call', name: token.value, args, size: rule(args.map((arg) => arg.size)) };
    } catch (error) {
      throw this.fail('expr.type', `${token.value}(): ${(error as Error).message}`, 'Check the number and sizes of the arguments.', token.pos);
    }
  }

  private swizzle(value: Expr): Expr {
    this.take();
    const field = this.take();
    const set = SWIZZLE_SETS.find((letters) => field && [...field.value].every((c) => letters.includes(c)));
    if (!field || !set || field.value.length > 4) {
      throw this.fail('expr.syntax', 'Invalid component selection', 'Use 1 to 4 letters from xyzw or from rgba, such as .xy or .rgb.', field?.pos);
    }
    if ([...field.value].some((c) => set.indexOf(c) >= value.size)) {
      throw this.fail('expr.type', `A ${label(value.size)} has no component ${field.value}`, 'Select only components the value has.', field.pos);
    }
    this.node();
    return { kind: 'swizzle', value, field: field.value, size: field.value.length as Size };
  }

  /** The size of `a op b`: equal sizes, or a float with a vector. */
  private joined(a: Size, b: Size): Size {
    if (a === b || b === 1) return a;
    if (a === 1) return b;
    throw this.fail('expr.type', `Cannot combine a ${label(a)} with a ${label(b)}`, 'Use equal sizes, or a float with a vector. Swizzle (.xy) to match sizes.');
  }

  // ---- tokens and errors

  private peek(value: string): boolean {
    return this.tokens[this.at]?.value === value;
  }

  private take(): Tok {
    return this.tokens[this.at++];
  }

  private expect(value: string, suggestion: string): void {
    if (!this.peek(value)) throw this.fail('expr.syntax', `Expected '${value}'`, suggestion);
    this.at++;
  }

  private expectLoop(value: string): void {
    if (!this.peek(value)) throw this.fail('expr.loop', `Expected '${value}' in the loop`, `Write the loop as for i in 0..${MAX_LOOP_BOUND} { ... } with a literal bound.`);
    this.at++;
  }

  private checkName(token: Tok): void {
    if (!NAME.test(token.value)) throw this.fail('expr.syntax', 'Expected a variable name', 'Start a line with a lowercase name, then =, as in a = p.x * 2.', token.pos);
    if (token.value.length > MAX_IDENTIFIER) throw this.fail('expr.limit', 'The name is too long', `Use at most ${MAX_IDENTIFIER} characters.`, token.pos);
  }

  private node(): void {
    if (++this.nodes > MAX_NODES) throw this.fail('expr.limit', 'The program is too large', `Use at most ${MAX_NODES} operations; share repeated work in variables.`);
  }

  private fail(code: Failure['code'], message: string, suggestion: string, pos = this.tokens[this.at]?.pos ?? this.source.length): Failure {
    return new Failure(code, message, suggestion, pos);
  }
}

const label = (size: Size): string => (size === 1 ? 'float' : `vec${size}`);

/** Parses a program, or returns the first error with the fix to apply. */
export function parseExpr(source: string): { program: Program } | { error: ParseError } {
  try {
    if (source.length > MAX_SOURCE) throw new Failure('expr.limit', 'The source is too long', `Use at most ${MAX_SOURCE} characters.`, MAX_SOURCE);
    return { program: new Parser(source).program() };
  } catch (error) {
    if (!(error instanceof Failure)) throw error;
    const before = source.slice(0, error.pos);
    return {
      error: {
        code: error.code,
        message: error.message,
        suggestion: error.suggestion,
        line: before.split('\n').length,
        column: error.pos - (before.lastIndexOf('\n') + 1) + 1,
      },
    };
  }
}
