//! SimL expressions: precedence climbing, swizzles, names, calls; every node is typed as it is built.
use super::{Parser, R};
use crate::sim::ast::{Bin, Builtin, Expr, FUNCS, Kind};
use crate::sim::lexer::Tok;
use crate::sim::typing::{binary_type, closest, func_type, swizzle_indices};
use crate::sim::{Diagnostic, Ty};

impl Parser {
    pub(super) fn expr(&mut self) -> R<Expr> {
        self.binary(0)
    }

    fn peek_binary(&self) -> Option<(Bin, u8)> {
        let Tok::Sym(s) = self.peek() else { return None };
        Some(match *s {
            "||" => (Bin::Or, 1),
            "&&" => (Bin::And, 2),
            "==" => (Bin::Eq, 3),
            "!=" => (Bin::Ne, 3),
            "<" => (Bin::Lt, 4),
            "<=" => (Bin::Le, 4),
            ">" => (Bin::Gt, 4),
            ">=" => (Bin::Ge, 4),
            "+" => (Bin::Add, 5),
            "-" => (Bin::Sub, 5),
            "*" => (Bin::Mul, 6),
            "/" => (Bin::Div, 6),
            "%" => (Bin::Rem, 6),
            _ => return None,
        })
    }

    fn binary(&mut self, min: u8) -> R<Expr> {
        let mut left = self.unary()?;
        while let Some((op, prec)) = self.peek_binary() {
            if prec < min {
                break;
            }
            let line = self.here().0;
            let at_operator = self.err("", "");
            self.at += 1;
            let right = self.binary(prec + 1)?;
            let ty = binary_type(op, left.ty, right.ty).ok_or_else(|| Diagnostic {
                message: format!("cannot combine a {} with a {} here", left.ty.name(), right.ty.name()),
                suggestion: "arithmetic takes equal types or a number with a vector; comparisons take numbers; && and || take bools".to_owned(),
                ..at_operator
            })?;
            left = Expr { kind: Kind::Bin(op, Box::new(left), Box::new(right)), ty, line };
        }
        Ok(left)
    }

    fn unary(&mut self) -> R<Expr> {
        let line = self.here().0;
        if self.eat_sym("-") {
            let inner = self.unary()?;
            if inner.ty == Ty::Bool {
                return Err(self.err("cannot negate a bool", "use `!` for a bool"));
            }
            let ty = inner.ty;
            return Ok(Expr { kind: Kind::Neg(Box::new(inner)), ty, line });
        }
        if self.eat_sym("!") {
            let inner = self.unary()?;
            if inner.ty != Ty::Bool {
                return Err(self.err("`!` takes a bool", "compare first, for example `!(x > 1.0)`"));
            }
            return Ok(Expr { kind: Kind::Not(Box::new(inner)), ty: Ty::Bool, line });
        }
        let mut e = self.primary()?;
        while self.is_sym(".") {
            self.at += 1;
            let swz = self.ident()?;
            let idxs = swizzle_indices(&swz).ok_or_else(|| self.err(format!("`{swz}` is not a swizzle"), "use letters x y z w (or r g b a)"))?;
            if e.ty == Ty::Float || e.ty == Ty::Bool || idxs.iter().any(|&i| i >= e.ty.width()) {
                return Err(self.err(format!("a {} has no `.{swz}`", e.ty.name()), "use components the vector has"));
            }
            let ty = Ty::of_width(idxs.len()).ok_or_else(|| self.err("a swizzle has one to four letters", "shorten it"))?;
            e = Expr { kind: Kind::Swizzle(Box::new(e), idxs), ty, line };
        }
        Ok(e)
    }

    fn primary(&mut self) -> R<Expr> {
        let line = self.here().0;
        match self.peek().clone() {
            Tok::Int(n) => {
                self.at += 1;
                #[expect(clippy::cast_precision_loss, reason = "source integers are small constants")]
                let x = n as f32;
                Ok(Expr { kind: Kind::Num(x), ty: Ty::Float, line })
            }
            Tok::Float(x) => {
                self.at += 1;
                #[expect(clippy::cast_possible_truncation, reason = "source numbers are f32 constants")]
                let x = x as f32;
                Ok(Expr { kind: Kind::Num(x), ty: Ty::Float, line })
            }
            Tok::Sym("(") => {
                self.at += 1;
                let e = self.expr()?;
                self.expect_sym(")")?;
                Ok(e)
            }
            Tok::Ident(name) => {
                self.at += 1;
                self.name(&name, line)
            }
            _ => Err(self.err("expected a value", "write a number, a name, a call or a parenthesised expression")),
        }
    }

    fn name(&mut self, name: &str, line: usize) -> R<Expr> {
        if name == "true" || name == "false" {
            return Ok(Expr { kind: Kind::Bool(name == "true"), ty: Ty::Bool, line });
        }
        if self.is_sym("(") {
            return self.call(name, line);
        }
        if self.is_sym("[") {
            let arr = self.state_index(name)?;
            self.at += 1;
            let idx = self.expr()?;
            self.expect_sym("]")?;
            let ty = self.prog.states[arr].ty;
            return Ok(Expr { kind: Kind::Elem { arr, idx: Box::new(idx) }, ty, line });
        }
        if let Some(slot) = self.locals.iter().rposition(|(n, _)| n == name) {
            return Ok(Expr { kind: Kind::Local(slot), ty: self.locals[slot].1, line });
        }
        if let Some(at) = self.prog.params.iter().position(|p| p.name == name) {
            return Ok(Expr { kind: Kind::Param(at), ty: self.prog.params[at].ty, line });
        }
        let builtin = match name {
            "dt" => Some(Builtin::Dt),
            "t" => Some(Builtin::T),
            "step" => Some(Builtin::Step),
            "seed" => Some(Builtin::Seed),
            "n" => Some(Builtin::N),
            _ => None,
        };
        if let Some(b) = builtin {
            return Ok(Expr { kind: Kind::Builtin(b), ty: Ty::Float, line });
        }
        let known = self.locals.iter().map(|(n, _)| n.as_str()).chain(self.prog.params.iter().map(|p| p.name.as_str()));
        let hint = closest(name, known);
        Err(Diagnostic {
            message: format!("`{name}` is not defined here"),
            suggestion: hint.map_or_else(|| "declare it with `let`, `param`, or index a state as `name[i]`".to_owned(), |h| format!("did you mean `{h}`?")),
            ..self.err("", "")
        })
    }

    fn call(&mut self, name: &str, line: usize) -> R<Expr> {
        if matches!(name, "bone_point" | "bone_vector" | "rest") {
            return self.input_call(name, line);
        }
        self.expect_sym("(")?;
        let mut args = Vec::new();
        if !self.is_sym(")") {
            loop {
                args.push(self.expr()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
        }
        self.expect_sym(")")?;
        let vec_width = match name {
            "vec2" => Some(2),
            "vec3" => Some(3),
            "vec4" => Some(4),
            _ => None,
        };
        if let Some(width) = vec_width {
            if args.iter().any(|a| a.ty == Ty::Bool) {
                return Err(self.err("a vector is made of numbers", "pass numbers or smaller vectors"));
            }
            let total: usize = args.iter().map(|a| a.ty.width()).sum();
            let broadcast = args.len() == 1 && args[0].ty == Ty::Float;
            if total != width && !broadcast {
                return Err(self.err(format!("{name} needs {width} numbers, got {total}"), format!("pass {width} numbers, or one number for all")));
            }
            let ty = Ty::of_width(width).unwrap_or(Ty::Vec3);
            return Ok(Expr { kind: Kind::Vector(args), ty, line });
        }
        let func = FUNCS.iter().find(|(n, _)| *n == name).map(|(_, f)| *f).ok_or_else(|| {
            let hint = closest(name, FUNCS.iter().map(|(n, _)| *n));
            self.err(format!("unknown function `{name}`"), hint.map_or_else(|| "see the SimL function list in the guide".to_owned(), |h| format!("did you mean `{h}`?")))
        })?;
        let ty = func_type(func, &args).map_err(|m| self.err(format!("{name}: {m}"), "check the number and types of the arguments"))?;
        Ok(Expr { kind: Kind::Call(func, args), ty, line })
    }
}
