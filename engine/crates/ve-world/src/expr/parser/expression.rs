use super::{ExprKind, Input, Kind, Node, Op, Parser, Span, Type, Unary, Value, check, literal};

impl Parser<'_> {
    pub(super) fn expression(&mut self, min_bp: u8) -> Node {
        self.depth += 1;
        if self.depth > 64 {
            let span = self.peek().span;
            self.error(
                span,
                "expression nesting exceeds 64",
                "reduce nested parentheses or operators",
            );
            self.depth -= 1;
            return literal(0.0, span);
        }
        let mut left = self.prefix();
        loop {
            if matches!(self.peek().kind, Kind::Dot) && 13 >= min_bp {
                left = self.swizzle(left);
                continue;
            }
            let Some((op, left_bp, right_bp)) = infix(&self.peek().kind) else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            let span = self.take().span;
            let right = self.expression(right_bp);
            left = self.binary(op, left, right, span);
        }
        self.depth -= 1;
        left
    }

    fn prefix(&mut self) -> Node {
        let token = self.take();
        match token.kind {
            Kind::Number(number) => Node {
                kind: ExprKind::Literal(Value::Float(number)),
                ty: Type::Float,
                span: token.span,
            },
            Kind::Name(name) if name == "true" || name == "false" => Node {
                kind: ExprKind::Literal(Value::Bool(name == "true")),
                ty: Type::Bool,
                span: token.span,
            },
            Kind::Name(name) if matches!(self.peek().kind, Kind::Left) => {
                self.call(name, token.span)
            }
            Kind::Name(name) => self.name(name, token.span),
            Kind::Minus | Kind::Bang => {
                let op = if matches!(token.kind, Kind::Minus) {
                    Unary::Negate
                } else {
                    Unary::Not
                };
                let child = self.expression(13);
                let ty = child.ty;
                if (op == Unary::Not) != (ty == Type::Bool) {
                    self.error(
                        token.span,
                        "unary operator has an incompatible type",
                        "use `!` with bool and `-` with numbers",
                    );
                }
                self.operations += 1;
                Node {
                    kind: ExprKind::Unary(op, Box::new(child)),
                    ty,
                    span: token.span,
                }
            }
            Kind::Left => {
                let node = self.expression(0);
                if matches!(self.peek().kind, Kind::Right) {
                    self.cursor += 1;
                } else {
                    self.error(
                        self.peek().span,
                        "expected `)`",
                        "close the parenthesized expression",
                    );
                }
                node
            }
            _ => {
                self.error(
                    token.span,
                    "expected an expression",
                    "write a number, variable, or function call",
                );
                literal(0.0, token.span)
            }
        }
    }

    fn name(&mut self, name: String, span: Span) -> Node {
        if let Some(node) = self.vars.get(&name) {
            return node.clone();
        }
        let input = match name.as_str() {
            "p" => Some((Input::Position, Type::Vec3)),
            "n" => Some((Input::Normal, Type::Vec3)),
            "uv" => Some((Input::Uv, Type::Vec2)),
            "pi" => Some((Input::Pi, Type::Float)),
            "tau" => Some((Input::Tau, Type::Float)),
            _ => None,
        };
        if let Some((input, ty)) = input {
            return Node {
                kind: ExprKind::Input(input),
                ty,
                span,
            };
        }
        if let Some(index) = self
            .bindings
            .fields
            .iter()
            .position(|(field, _)| field == &name)
        {
            return Node {
                kind: ExprKind::Input(Input::Bound(index)),
                ty: self.bindings.fields[index].1,
                span,
            };
        }
        let hint = check::nearest(
            &name,
            &[
                "p",
                "n",
                "uv",
                "pi",
                "tau",
                "out",
                "abs",
                "sin",
                "cos",
                "mix",
                "length",
                "smoothstep",
            ],
        );
        self.error(
            span,
            format!("unknown name `{name}`"),
            hint.map_or_else(
                || "define this name before using it".to_owned(),
                |value| format!("did you mean `{value}`?"),
            ),
        );
        literal(0.0, span)
    }

    fn binary(&mut self, op: Op, left: Node, right: Node, span: Span) -> Node {
        let left_type = left.ty;
        let right_type = right.ty;
        let bool_op = matches!(op, Op::Eq | Op::Ne)
            && left_type == right_type
            && matches!(left_type, Type::Bool | Type::Float)
            || matches!(op, Op::Lt | Op::Le | Op::Gt | Op::Ge)
                && left_type == Type::Float
                && right_type == Type::Float
            || matches!(op, Op::And | Op::Or)
                && left_type == Type::Bool
                && right_type == Type::Bool;
        let math_op = matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod)
            && (left_type == right_type || left_type == Type::Float || right_type == Type::Float)
            && left_type != Type::Bool
            && right_type != Type::Bool;
        let ty = if bool_op {
            Type::Bool
        } else if math_op {
            if left_type == Type::Float {
                right_type
            } else {
                left_type
            }
        } else {
            self.error(
                span,
                format!(
                    "incompatible operand types {} and {}",
                    left_type.name(),
                    right_type.name()
                ),
                "use matching types or a float with a vector",
            );
            left_type
        };
        if left_type.size() > 1 && right_type.size() > 1 && left_type != right_type {
            self.error(
                span,
                format!(
                    "cannot combine {} and {}",
                    left_type.name(),
                    right_type.name()
                ),
                "use vectors with matching sizes",
            );
        }
        self.operations += 1;
        Node {
            kind: ExprKind::Binary(op.into(), Box::new(left), Box::new(right)),
            ty,
            span,
        }
    }

    fn swizzle(&mut self, child: Node) -> Node {
        let span = self.take().span;
        let token = self.take();
        let Kind::Name(chars) = token.kind else {
            self.error(
                token.span,
                "expected swizzle after `.`",
                "use x, y, z, or w",
            );
            return child;
        };
        let mut indices = Vec::new();
        for character in chars.chars() {
            let index = match character {
                'x' | 'r' => 0,
                'y' | 'g' => 1,
                'z' | 'b' => 2,
                'w' | 'a' => 3,
                _ => 8,
            };
            if index >= child.ty.size() || index > 3 {
                self.error(
                    token.span,
                    format!("{} has no swizzle component `{character}`", child.ty.name()),
                    "use a component within the vector size",
                );
            }
            indices.push(index.min(3));
        }
        if indices.is_empty() || indices.len() > 4 {
            self.error(
                token.span,
                "swizzle must have one to four components",
                "choose one to four components",
            );
        }
        let ty = match indices.len() {
            1 => Type::Float,
            2 => Type::Vec2,
            3 => Type::Vec3,
            _ => Type::Vec4,
        };
        self.operations += 1;
        Node {
            kind: ExprKind::Swizzle(Box::new(child), indices),
            ty,
            span,
        }
    }
}

fn infix(kind: &Kind) -> Option<(Op, u8, u8)> {
    Some(match kind {
        Kind::Or => (Op::Or, 1, 2),
        Kind::And => (Op::And, 3, 4),
        Kind::EqEq => (Op::Eq, 5, 6),
        Kind::Ne => (Op::Ne, 5, 6),
        Kind::Lt => (Op::Lt, 7, 8),
        Kind::Le => (Op::Le, 7, 8),
        Kind::Gt => (Op::Gt, 7, 8),
        Kind::Ge => (Op::Ge, 7, 8),
        Kind::Plus => (Op::Add, 9, 10),
        Kind::Minus => (Op::Sub, 9, 10),
        Kind::Star => (Op::Mul, 11, 12),
        Kind::Slash => (Op::Div, 11, 12),
        Kind::Percent => (Op::Mod, 11, 12),
        _ => return None,
    })
}
