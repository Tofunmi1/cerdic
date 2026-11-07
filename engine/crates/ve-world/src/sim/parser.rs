//! SimL parser: tokens to a typed program whose names are already resolved to parameter, state and local slots.
mod expression;
mod inputs;
mod statements;

use super::ast::{Count, InputKind, Kernel, Output, ParamDecl, Program, StateDecl, Step, Stmt};
use super::lexer::{Tok, Token};
use super::typing::closest;
use super::{Diagnostic, Ty, run};

type R<T> = Result<T, Diagnostic>;

struct Parser {
    toks: Vec<Token>,
    at: usize,
    prog: Program,
    locals: Vec<(String, Ty)>,
    max_locals: usize,
    writes: Vec<bool>,
}

pub(super) fn parse(toks: Vec<Token>) -> Result<Program, Vec<Diagnostic>> {
    let mut p = Parser { toks, at: 0, prog: Program::default(), locals: Vec::new(), max_locals: 0, writes: Vec::new() };
    p.program().map_err(|d| vec![d])?;
    Ok(p.prog)
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.at.min(self.toks.len() - 1)].tok
    }
    fn here(&self) -> (usize, usize) {
        let t = &self.toks[self.at.min(self.toks.len() - 1)];
        (t.line, t.col)
    }
    fn err(&self, message: impl Into<String>, suggestion: impl Into<String>) -> Diagnostic {
        let (line, column) = self.here();
        Diagnostic { line, column, message: message.into(), suggestion: suggestion.into() }
    }
    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Sym(x) if *x == s)
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        let yes = self.is_sym(s);
        if yes {
            self.at += 1;
        }
        yes
    }
    fn expect_sym(&mut self, s: &str) -> R<()> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(self.err(format!("expected `{s}`"), format!("write `{s}` here")))
        }
    }
    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Tok::Ident(x) if x == w)
    }
    fn eat_word(&mut self, w: &str) -> bool {
        let yes = self.is_word(w);
        if yes {
            self.at += 1;
        }
        yes
    }
    fn ident(&mut self) -> R<String> {
        if let Tok::Ident(name) = self.peek().clone() {
            self.at += 1;
            Ok(name)
        } else {
            Err(self.err("expected a name", "write a name made of letters, digits and _"))
        }
    }
    fn int(&mut self) -> R<i64> {
        if let Tok::Int(n) = *self.peek() {
            self.at += 1;
            Ok(n)
        } else {
            Err(self.err("expected a whole number", "write an integer such as 6"))
        }
    }
    fn skip_separators(&mut self) {
        while self.at < self.toks.len() && (matches!(self.peek(), Tok::Newline) || self.is_sym(";")) {
            self.at += 1;
        }
    }
    fn end_of_statement(&mut self) -> R<()> {
        if matches!(self.peek(), Tok::Newline) || self.is_sym(";") || self.is_sym("}") {
            Ok(())
        } else {
            Err(self.err("expected the end of the statement", "put one statement per line, or separate them with `;`"))
        }
    }

    // ---- declarations ----

    fn program(&mut self) -> R<()> {
        loop {
            self.skip_separators();
            if self.at >= self.toks.len() {
                break;
            }
            let word = self.ident().map_err(|_| self.err("expected `sim`, `param`, `state`, `kernel`, `schedule` or `output`", "start a declaration with one of those words"))?;
            match word.as_str() {
                "sim" => self.prog.name = self.ident()?,
                "param" => self.param()?,
                "input" => self.input_decl()?,
                "state" => self.state()?,
                "kernel" => self.kernel()?,
                "schedule" => self.prog.schedule = self.schedule_items(false)?,
                "output" => self.output()?,
                other => {
                    self.at -= 1;
                    return Err(self.err(format!("unknown declaration `{other}`"), "use sim, param, state, kernel, schedule or output"));
                }
            }
            self.end_of_statement()?;
        }
        if self.prog.schedule.is_empty() {
            return Err(self.err("the program has no `schedule`", "add `schedule kernel_name; other_kernel` listing the kernels to run each step"));
        }
        Ok(())
    }

    fn param(&mut self) -> R<()> {
        let name = self.ident()?;
        if self.prog.params.iter().any(|p| p.name == name) {
            return Err(self.err(format!("`{name}` is already a parameter"), "use another name"));
        }
        self.expect_sym("=")?;
        self.locals.clear();
        let value = self.expr()?;
        let known: Vec<_> = self.prog.params.iter().map(|p| p.default).collect();
        let default = run::constant(&value, &known).map_err(|m| self.err(m, "a parameter default is a number or a vector of numbers"))?;
        self.prog.params.push(ParamDecl { name, ty: default.ty, default });
        Ok(())
    }

    fn state(&mut self) -> R<()> {
        let line = self.here().0;
        let name = self.ident()?;
        if self.prog.states.iter().any(|s| s.name == name) {
            return Err(self.err(format!("`{name}` is already a state"), "use another name"));
        }
        self.expect_sym(":")?;
        let ty_name = self.ident()?;
        let ty = match ty_name.as_str() {
            "f32" => Ty::Float,
            "vec2" => Ty::Vec2,
            "vec3" => Ty::Vec3,
            "vec4" => Ty::Vec4,
            "bool" => Ty::Bool,
            _ => return Err(self.err(format!("unknown type `{ty_name}`"), "use f32, vec2, vec3, vec4 or bool")),
        };
        self.expect_sym("[")?;
        let count = if let Tok::Ident(p) = self.peek().clone() {
            self.at += 1;
            if let Some(mesh) = self.prog.inputs.iter().position(|i| i.name == p && i.kind == InputKind::Mesh) {
                Count::Mesh(mesh)
            } else {
                let at = self.prog.params.iter().position(|x| x.name == p).ok_or_else(|| self.err(format!("no parameter or mesh input `{p}`"), "declare it with `param` or `input mesh` above, or write a number"))?;
                Count::Param(at)
            }
        } else {
            let n = self.int()?;
            Count::Lit(usize::try_from(n).map_err(|_| self.err("the count must be positive", "write a count such as 256"))?)
        };
        self.expect_sym("]")?;
        self.prog.states.push(StateDecl { name, ty, count, line });
        Ok(())
    }

    fn output(&mut self) -> R<()> {
        let kind = self.ident()?;
        if !["particles", "values", "mesh", "push"].contains(&kind.as_str()) {
            return Err(self.err(format!("unknown output kind `{kind}`"), "use `particles` (a vec3 array drawn as tetrahedra), `mesh` (a vec3 array with one point per vertex of the input mesh), `push BODY array` (a vec3 array whose sum changes the speed of the physics body BODY every step) or `values`"));
        }
        let target = if kind == "push" { Some(self.ident()?) } else { None };
        let name = self.ident()?;
        let state = self.state_index(&name)?;
        if kind == "mesh" && !(self.prog.states[state].ty == Ty::Vec3 && matches!(self.prog.states[state].count, Count::Mesh(_))) {
            return Err(self.err("a mesh output is a vec3 array sized by a mesh input", "declare `input mesh m` and `state pos: vec3[m]`"));
        }
        if kind == "particles" && self.prog.states[state].ty != Ty::Vec3 {
            return Err(self.err("particles need a vec3 array", "declare the positions as `state pos: vec3[N]`"));
        }
        if kind == "push" && self.prog.states[state].ty != Ty::Vec3 {
            return Err(self.err("a push output is a vec3 array", "declare `state push: vec3[1]` and write the speed change into it"));
        }
        self.prog.outputs.push(Output { kind, state, target });
        Ok(())
    }

    fn kernel(&mut self) -> R<()> {
        let name = self.ident()?;
        if self.prog.kernels.iter().any(|k| k.name == name) {
            return Err(self.err(format!("`{name}` is already a kernel"), "use another name"));
        }
        self.locals.clear();
        self.max_locals = 0;
        self.writes = vec![false; self.prog.states.len()];
        let over = if self.eat_word("once") {
            None
        } else if self.eat_word("each") {
            let var = self.ident()?;
            if !self.eat_word("in") {
                return Err(self.err("expected `in`", "write `each i in pos`"));
            }
            let arr_name = self.ident()?;
            let arr = self.state_index(&arr_name)?;
            self.push_local(var, Ty::Float);
            Some(arr)
        } else {
            return Err(self.err("expected `each` or `once`", "write `kernel name each i in pos { ... }` or `kernel name once { ... }`"));
        };
        let body = self.block()?;
        if name == "init" {
            self.prog.init = Some(self.prog.kernels.len());
        }
        self.prog.kernels.push(Kernel { name, over, body, locals: self.max_locals, writes: std::mem::take(&mut self.writes) });
        Ok(())
    }

    fn state_index(&self, name: &str) -> R<usize> {
        self.prog.states.iter().position(|s| s.name == name).ok_or_else(|| {
            let hint = closest(name, self.prog.states.iter().map(|s| s.name.as_str()));
            self.err(format!("no state `{name}`"), hint.map_or_else(|| "declare it with `state name: vec3[N]` above".to_owned(), |h| format!("did you mean `{h}`?")))
        })
    }

    fn schedule_items(&mut self, nested: bool) -> R<Vec<Step>> {
        let mut steps = Vec::new();
        loop {
            if self.eat_word("repeat") {
                let n = self.int()?;
                let n = u32::try_from(n).ok().filter(|n| (1..=1000).contains(n)).ok_or_else(|| self.err("repeat count must be 1 to 1000", "write `repeat 6 { ... }`"))?;
                self.expect_sym("{")?;
                let inner = self.schedule_items(true)?;
                self.expect_sym("}")?;
                steps.push(Step::Repeat(n, inner));
            } else {
                let name = self.ident()?;
                let at = self.prog.kernels.iter().position(|k| k.name == name).ok_or_else(|| {
                    let hint = closest(&name, self.prog.kernels.iter().map(|k| k.name.as_str()));
                    self.err(format!("no kernel `{name}`"), hint.map_or_else(|| "declare the kernel above the schedule".to_owned(), |h| format!("did you mean `{h}`?")))
                })?;
                steps.push(Step::Run(at));
            }
            if self.eat_sym(";") {
                if !nested && matches!(self.peek(), Tok::Newline) {
                    break;
                }
            } else if !(nested && matches!(self.peek(), Tok::Newline)) {
                break;
            }
            if nested {
                self.skip_separators();
                if self.is_sym("}") {
                    break;
                }
            }
        }
        Ok(steps)
    }

    // ---- statements ----

    fn push_local(&mut self, name: String, ty: Ty) -> usize {
        self.locals.push((name, ty));
        self.max_locals = self.max_locals.max(self.locals.len());
        self.locals.len() - 1
    }

    fn block(&mut self) -> R<Vec<Stmt>> {
        self.expect_sym("{")?;
        let saved = self.locals.len();
        let mut out = Vec::new();
        self.skip_separators();
        while !self.is_sym("}") {
            if self.at >= self.toks.len() {
                return Err(self.err("the block is not closed", "add a `}`"));
            }
            out.push(self.statement()?);
            self.end_of_statement()?;
            self.skip_separators();
        }
        self.at += 1;
        self.locals.truncate(saved);
        Ok(out)
    }

    fn statement(&mut self) -> R<Stmt> {
        let line = self.here().0;
        if self.eat_word("let") {
            let name = self.ident()?;
            self.expect_sym("=")?;
            let value = self.expr()?;
            let slot = self.push_local(name, value.ty);
            return Ok(Stmt::Let(slot, value));
        }
        if self.eat_word("if") {
            return self.if_statement();
        }
        if self.eat_word("for") {
            return self.for_loop(line);
        }
        let name = self.ident()?;
        if self.is_sym("[") {
            return self.set_element(&name, line);
        }
        let slot = self.locals.iter().rposition(|(n, _)| *n == name).ok_or_else(|| self.err(format!("`{name}` is not a local variable"), "declare it with `let`; state is written as `name[i] = ...`"))?;
        let op = self.assign_op()?;
        let value = self.expr()?;
        let ty = self.locals[slot].1;
        if value.ty != ty && !(op.is_some() && value.ty == Ty::Float) {
            return Err(self.err(format!("`{name}` is a {} but the value is a {}", ty.name(), value.ty.name()), "make the types match"));
        }
        Ok(Stmt::SetLocal { slot, op, value })
    }

}
