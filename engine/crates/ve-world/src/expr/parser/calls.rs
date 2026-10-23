use super::{ExprKind, Kind, Node, Parser, Span, Type, Value, check_call};

impl Parser<'_> {
    pub(super) fn call(&mut self, name: String, span: Span) -> Node {
        self.cursor += 1;
        if matches!(name.as_str(), "attr" | "attr2" | "attr3" | "attr4")
            && matches!(self.peek().kind, Kind::String(_))
        {
            let token = self.take();
            let Kind::String(attribute) = token.kind else {
                return literal(0.0, span);
            };
            if matches!(self.peek().kind, Kind::Right) {
                self.cursor += 1;
            } else {
                self.error(
                    self.peek().span,
                    "attribute call expects one string literal",
                    "write attr(\"name\")",
                );
            }
            let ty = match name.as_str() {
                "attr" => Type::Float,
                "attr2" => Type::Vec2,
                "attr3" => Type::Vec3,
                _ => Type::Vec4,
            };
            if !self.attrs.iter().any(|(found, _)| found == &attribute) {
                self.attrs.push((attribute.clone(), ty));
            }
            self.operations += 1;
            return Node {
                kind: ExprKind::Attribute(attribute),
                ty,
                span,
            };
        }
        let mut args = Vec::new();
        while !matches!(self.peek().kind, Kind::Right | Kind::End) {
            args.push(self.expression(0));
            if matches!(self.peek().kind, Kind::Comma) {
                self.cursor += 1;
            } else {
                break;
            }
        }
        if matches!(self.peek().kind, Kind::Right) {
            self.cursor += 1;
        } else {
            self.error(
                self.peek().span,
                "expected `)` after arguments",
                "close the function call",
            );
        }
        let ty = check_call(&name, &args, span, &mut self.errors);
        self.operations += 1;
        Node {
            kind: ExprKind::Call(name, args),
            ty,
            span,
        }
    }
}

fn literal(value: f32, span: Span) -> Node {
    Node {
        kind: ExprKind::Literal(Value::Float(value)),
        ty: Type::Float,
        span,
    }
}
