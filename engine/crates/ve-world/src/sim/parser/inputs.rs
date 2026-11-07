//! SimL inputs: `input bone NAME`, `input mesh NAME`, the calls that read them and the `adjacent` loop.
use super::{Parser, R};
use crate::sim::Ty;
use crate::sim::ast::{Expr, InputDecl, InputKind, Kind, Stmt};
use crate::sim::typing::closest;

impl Parser {
    pub(super) fn input_decl(&mut self) -> R<()> {
        let word = self.ident()?;
        let kind = match word.as_str() {
            "bone" => InputKind::Bone,
            "mesh" => InputKind::Mesh,
            other => return Err(self.err(format!("unknown input kind `{other}`"), "use `input bone NAME` (a bone's pose) or `input mesh NAME` (points and edges)")),
        };
        let name = self.ident()?;
        if self.prog.inputs.iter().any(|i| i.name == name) {
            return Err(self.err(format!("`{name}` is already an input"), "use another name"));
        }
        self.prog.inputs.push(InputDecl { name, kind });
        Ok(())
    }

    pub(super) fn input_index(&self, name: &str, kind: InputKind) -> R<usize> {
        let found = self.prog.inputs.iter().position(|i| i.name == name && i.kind == kind);
        found.ok_or_else(|| {
            let same = self.prog.inputs.iter().filter(|i| i.kind == kind).map(|i| i.name.as_str());
            let word = if kind == InputKind::Bone { "bone" } else { "mesh" };
            let hint = closest(name, same);
            self.err(format!("no {word} input `{name}`"), hint.map_or_else(|| format!("declare it with `input {word} {name}` above"), |h| format!("did you mean `{h}`?")))
        })
    }

    /// `bone_point(bone, v)`, `bone_vector(bone, v)` and `rest(mesh, i)`: the first argument is an input's name.
    pub(super) fn input_call(&mut self, name: &str, line: usize) -> R<Expr> {
        self.expect_sym("(")?;
        let target = self.ident()?;
        self.expect_sym(",")?;
        let arg = self.expr()?;
        self.expect_sym(")")?;
        if name == "rest" {
            let input = self.input_index(&target, InputKind::Mesh)?;
            if arg.ty != Ty::Float {
                return Err(self.err("rest(mesh, i) takes a point number", "pass the element number, for example `rest(cape, i)`"));
            }
            return Ok(Expr { kind: Kind::Rest { input, idx: Box::new(arg) }, ty: Ty::Vec3, line });
        }
        let input = self.input_index(&target, InputKind::Bone)?;
        if arg.ty != Ty::Vec3 {
            return Err(self.err(format!("{name}(bone, v) takes a vec3"), "pass a position or direction in the bone's own space"));
        }
        Ok(Expr { kind: Kind::Bone { input, vector: name == "bone_vector", v: Box::new(arg) }, ty: Ty::Vec3, line })
    }

    /// `for j, len in adjacent(mesh, v) { ... }` after the first loop name was read.
    pub(super) fn adjacent_loop(&mut self, first: String, line: usize) -> R<Stmt> {
        let second = self.ident()?;
        if !self.eat_word("in") || !self.eat_word("adjacent") {
            return Err(self.err("two loop names go with `adjacent`", "write `for j, len in adjacent(mesh, i) { ... }`"));
        }
        self.expect_sym("(")?;
        let mesh = self.ident()?;
        let input = self.input_index(&mesh, InputKind::Mesh)?;
        self.expect_sym(",")?;
        let of = self.expr()?;
        self.expect_sym(")")?;
        if of.ty != Ty::Float {
            return Err(self.err("adjacent(mesh, v) takes a point number", "pass the element number"));
        }
        let saved = self.locals.len();
        let vertex = self.push_local(first, Ty::Float);
        let length = self.push_local(second, Ty::Float);
        let body = self.block()?;
        self.locals.truncate(saved);
        Ok(Stmt::Adjacent { vertex, length, input, of, body, line })
    }
}
