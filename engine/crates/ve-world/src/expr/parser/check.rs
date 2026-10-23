use super::{Binary, Diagnostic, ExprKind, Node, Span, Type, Unary, Value};

pub(super) fn check_call(
    name: &str,
    args: &[Node],
    span: Span,
    errors: &mut Vec<Diagnostic>,
) -> Type {
    check_arity(name, args.len(), span, errors);
    if let Some(vector_size) = vector_size(name) {
        return check_constructor(name, vector_size, args, span, errors);
    }
    match name {
        "length" | "distance" | "dot" => Type::Float,
        "normalize" => args.first().map_or(Type::Float, |arg| arg.ty),
        "cross" => check_cross(args, span, errors),
        "select" => check_select(args, span, errors),
        "ramp" | "curve" => check_ramp(name, args, span, errors),
        "perlin" | "fbm" | "ridged" | "voronoi" => {
            check_noise(name, args, span, errors);
            Type::Float
        }
        _ => common_type(
            args.first().map(|arg| arg.ty),
            args.get(1).map(|arg| arg.ty),
        ),
    }
}

fn check_constructor(
    name: &str,
    size: usize,
    args: &[Node],
    span: Span,
    errors: &mut Vec<Diagnostic>,
) -> Type {
    let components: usize = args.iter().map(|arg| arg.ty.size()).sum();
    if !(args.len() == 1 && args[0].ty == Type::Float) && components != size {
        errors.push(Diagnostic::new(
            span,
            format!("{name} arguments provide {components} components, expected {size}"),
            format!("use {name}(x) to broadcast or provide {size} components"),
        ));
    }
    match size {
        2 => Type::Vec2,
        3 => Type::Vec3,
        _ => Type::Vec4,
    }
}

fn check_cross(args: &[Node], span: Span, errors: &mut Vec<Diagnostic>) -> Type {
    if args.iter().any(|arg| arg.ty != Type::Vec3) {
        errors.push(Diagnostic::new(
            span,
            "cross requires two vec3 arguments",
            "pass two vec3 values",
        ));
    }
    Type::Vec3
}

fn check_select(args: &[Node], span: Span, errors: &mut Vec<Diagnostic>) -> Type {
    if args.first().is_some_and(|arg| arg.ty != Type::Bool) {
        errors.push(Diagnostic::new(
            span,
            "select condition must be bool",
            "compare values to produce a bool",
        ));
    }
    let first = args.get(1).map(|arg| arg.ty);
    if first != args.get(2).map(|arg| arg.ty) {
        errors.push(Diagnostic::new(
            span,
            "select values must have the same type",
            "use matching value types",
        ));
    }
    first.unwrap_or(Type::Float)
}

fn check_noise(name: &str, args: &[Node], span: Span, errors: &mut Vec<Diagnostic>) {
    if args.first().is_some_and(|arg| arg.ty != Type::Vec3) {
        errors.push(Diagnostic::new(
            span,
            format!("{name} position must be vec3"),
            "pass p or another vec3",
        ));
    }
}

fn check_arity(name: &str, count: usize, span: Span, errors: &mut Vec<Diagnostic>) {
    let wanted = match name {
        "abs" | "sign" | "floor" | "ceil" | "fract" | "sqrt" | "exp" | "log" | "sin" | "cos"
        | "normalize" | "length" | "attr" | "attr2" | "attr3" | "attr4" => Some(1),
        "atan2" | "pow" | "min" | "max" | "step" | "distance" | "dot" | "cross" | "perlin"
        | "voronoi" => Some(2),
        "clamp" | "mix" | "smoothstep" | "select" | "fbm" | "ridged" => Some(3),
        _ => None,
    };
    if let Some(expected) = wanted.filter(|expected| *expected != count) {
        errors.push(Diagnostic::new(
            span,
            format!("{name} expects {expected} arguments, got {count}"),
            "check the function argument count",
        ));
    }
    if !is_known(name) {
        let suggestion = nearest(
            name,
            &[
                "smoothstep",
                "normalize",
                "perlin",
                "voronoi",
                "mix",
                "sin",
                "vec3",
            ],
        )
        .map_or("check the function name".to_owned(), |value| {
            format!("did you mean `{value}`?")
        });
        errors.push(Diagnostic::new(
            span,
            format!("unknown function `{name}`"),
            suggestion,
        ));
    }
}

fn check_ramp(name: &str, args: &[Node], span: Span, errors: &mut Vec<Diagnostic>) -> Type {
    if args.len() < 5 || args.len().is_multiple_of(2) {
        errors.push(Diagnostic::new(
            span,
            format!("{name} needs x and at least two position/value stops"),
            "provide x followed by position, value pairs",
        ));
    }
    if args.len() < 5 {
        return Type::Float;
    }
    let value_type = args[2].ty;
    if value_type == Type::Bool
        || args[1..]
            .chunks_exact(2)
            .any(|pair| pair[0].ty != Type::Float || pair[1].ty != value_type)
    {
        errors.push(Diagnostic::new(
            span,
            format!("{name} stop positions and values have incompatible types"),
            "use increasing float positions and values of one type",
        ));
    }
    let positions: Vec<f32> = (1..args.len())
        .step_by(2)
        .filter_map(|index| constant(&args[index]))
        .collect();
    if positions.len() == (args.len() - 1) / 2
        && positions.windows(2).any(|pair| pair[0] >= pair[1])
    {
        errors.push(Diagnostic::new(
            span,
            format!("{name} stop positions must be increasing"),
            "sort the stops by increasing position",
        ));
    }
    value_type
}

fn vector_size(name: &str) -> Option<usize> {
    match name {
        "vec2" => Some(2),
        "vec3" => Some(3),
        "vec4" => Some(4),
        _ => None,
    }
}
fn is_known(name: &str) -> bool {
    matches!(
        name,
        "abs"
            | "sign"
            | "floor"
            | "ceil"
            | "fract"
            | "sqrt"
            | "exp"
            | "log"
            | "sin"
            | "cos"
            | "atan2"
            | "min"
            | "max"
            | "pow"
            | "clamp"
            | "mix"
            | "step"
            | "smoothstep"
            | "length"
            | "distance"
            | "dot"
            | "cross"
            | "normalize"
            | "select"
            | "ramp"
            | "curve"
            | "perlin"
            | "fbm"
            | "ridged"
            | "voronoi"
            | "vec2"
            | "vec3"
            | "vec4"
            | "attr"
            | "attr2"
            | "attr3"
            | "attr4"
    )
}
fn common_type(first: Option<Type>, second: Option<Type>) -> Type {
    match (first, second) {
        (Some(Type::Float), Some(other)) | (Some(other), Some(Type::Float)) => other,
        (Some(first), _) => first,
        _ => Type::Float,
    }
}
fn constant(node: &Node) -> Option<f32> {
    match &node.kind {
        ExprKind::Literal(Value::Float(value)) => Some(*value),
        ExprKind::Unary(Unary::Negate, child) => Some(-constant(child)?),
        ExprKind::Binary(op, left, right) => {
            let first = constant(left)?;
            let second = constant(right)?;
            match op {
                Binary::Add => Some(first + second),
                Binary::Sub => Some(first - second),
                Binary::Mul => Some(first * second),
                Binary::Div if second != 0.0 => Some(first / second),
                Binary::Mod if second != 0.0 => Some(first - second * (first / second).floor()),
                _ => None,
            }
        }
        _ => None,
    }
}
pub(super) fn nearest<'a>(word: &str, options: &'a [&'a str]) -> Option<&'a str> {
    options
        .iter()
        .copied()
        .min_by_key(|candidate| edit_distance(word, candidate))
        .filter(|candidate| edit_distance(word, candidate) <= 3)
}
fn edit_distance(first: &str, second: &str) -> usize {
    let mut row: Vec<usize> = (0..=second.len()).collect();
    for (i, first_char) in first.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, second_char) in second.chars().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(previous + usize::from(first_char != second_char));
            previous = old;
        }
    }
    row[second.len()]
}
