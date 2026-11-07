//! SimL control flow and writes: `if`, element assignment, and the `for` loops (constant range, `neighbors`).
use super::{Parser, R};
use crate::sim::Ty;
use crate::sim::ast::{Bin, Stmt};
use crate::sim::typing::swizzle_indices;

impl Parser {
    pub(super) fn if_statement(&mut self) -> R<Stmt> {
        let cond = self.expr()?;
        if cond.ty != Ty::Bool {
            return Err(self.err(format!("the condition is a {}, not a bool", cond.ty.name()), "compare with `<`, `>` or `==`"));
        }
        let then = self.block()?;
        let save = self.at;
        self.skip_separators();
        let other = if self.eat_word("else") {
            if self.eat_word("if") { vec![self.if_statement()?] } else { self.block()? }
        } else {
            self.at = save;
            Vec::new()
        };
        Ok(Stmt::If(cond, then, other))
    }

    pub(super) fn set_element(&mut self, name: &str, line: usize) -> R<Stmt> {
        let arr = self.state_index(name)?;
        self.at += 1;
        let idx = self.expr()?;
        self.expect_sym("]")?;
        let comp = if self.eat_sym(".") {
            let swz = self.ident()?;
            let idxs = swizzle_indices(&swz).filter(|v| v.len() == 1).ok_or_else(|| self.err("write one component", "write `.x`, `.y`, `.z` or `.w`"))?;
            Some(idxs[0])
        } else {
            None
        };
        let op = self.assign_op()?;
        let value = self.expr()?;
        let target = self.prog.states[arr].ty;
        let wanted = if comp.is_some() { Ty::Float } else { target };
        let scaled = op.is_some() && value.ty == Ty::Float && target != Ty::Bool;
        if value.ty != wanted && !scaled {
            return Err(self.err(format!("`{name}` takes a {} but the value is a {}", wanted.name(), value.ty.name()), "make the types match"));
        }
        if let Some(c) = comp
            && c >= target.width()
        {
            return Err(self.err(format!("`{name}` has no such component"), "use a component the type has"));
        }
        self.writes[arr] = true;
        Ok(Stmt::SetElem { arr, idx, comp, op, value, line })
    }

    pub(super) fn assign_op(&mut self) -> R<Option<Bin>> {
        for (sym, op) in [("=", None), ("+=", Some(Bin::Add)), ("-=", Some(Bin::Sub)), ("*=", Some(Bin::Mul)), ("/=", Some(Bin::Div))] {
            if self.eat_sym(sym) {
                return Ok(op);
            }
        }
        Err(self.err("expected `=`, `+=`, `-=`, `*=` or `/=`", "assign a value to the variable"))
    }

    pub(super) fn for_loop(&mut self, line: usize) -> R<Stmt> {
        let var = self.ident()?;
        if self.eat_sym(",") {
            return self.adjacent_loop(var, line);
        }
        if !self.eat_word("in") {
            return Err(self.err("expected `in`", "write `for k in 0..4 { ... }`"));
        }
        let saved = self.locals.len();
        if self.eat_word("neighbors") {
            self.expect_sym("(")?;
            let arr_name = self.ident()?;
            let arr = self.state_index(&arr_name)?;
            if self.prog.states[arr].ty != Ty::Vec3 {
                return Err(self.err("neighbors search a vec3 array of positions", "name the positions array"));
            }
            self.expect_sym(",")?;
            let centre = self.expr()?;
            self.expect_sym(",")?;
            let radius = self.expr()?;
            self.expect_sym(")")?;
            if centre.ty != Ty::Vec3 || radius.ty != Ty::Float {
                return Err(self.err("neighbors(array, centre: vec3, radius: f32)", "pass a vec3 position and a number"));
            }
            if !self.eat_word("cap") {
                return Err(self.err("a neighbors loop needs a cap", "add `cap 24` after the parentheses: the most neighbours one element visits"));
            }
            let cap = u32::try_from(self.int()?).ok().filter(|c| (1..=4096).contains(c)).ok_or_else(|| self.err("cap must be 1 to 4096", "write `cap 24`"))?;
            let slot = self.push_local(var, Ty::Float);
            let body = self.block()?;
            self.locals.truncate(saved);
            return Ok(Stmt::Neighbors { slot, arr, centre, radius, cap, body, line });
        }
        let from = self.signed_int()?;
        self.expect_sym("..")?;
        let to = self.signed_int()?;
        if to - from > 4096 {
            return Err(self.err("a loop runs at most 4096 turns", "use a smaller range or split the work"));
        }
        let slot = self.push_local(var, Ty::Float);
        let body = self.block()?;
        self.locals.truncate(saved);
        Ok(Stmt::Repeat { slot, from, to, body })
    }

    fn signed_int(&mut self) -> R<i32> {
        let negative = self.eat_sym("-");
        let n = i32::try_from(self.int().map_err(|_| self.err("loop bounds are whole-number constants", "write `for k in 0..4`"))?).map_err(|_| self.err("bound too large", "use a smaller bound"))?;
        Ok(if negative { -n } else { n })
    }
}
