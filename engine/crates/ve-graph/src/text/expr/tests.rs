use super::expand;

fn run(text: &str) -> String {
    expand(text).unwrap()
}

#[test]
fn text_without_expressions_is_untouched() {
    let text = "a = solid.box@1(size=[1, 2, 3])\n# {not an expression}\n";
    assert_eq!(run(text), text);
}

#[test]
fn constants_and_arithmetic_fold_into_numbers() {
    let text = "let w = 1.5\nlet half = w / 2\nb = solid.box@1(size=[{w}, {half}, {w * 2 + 1}])\n";
    assert_eq!(run(text), "\n\nb = solid.box@1(size=[1.5, 0.75, 4])\n");
}

#[test]
fn units_follow_the_closing_brace() {
    assert_eq!(
        run("let t = 2\na = f(x={t * 5}mm, y={90 / 2}deg)\n"),
        "\na = f(x=10mm, y=45deg)\n"
    );
}

#[test]
fn precedence_brackets_and_functions() {
    assert_eq!(
        run(
            "a = f({2 + 3 * 4}, {(2 + 3) * 4}, {-2 * -3}, {max(1, 5, 3)}, {clamp(9, 0, 4)}, {sqrt(16)})\n"
        ),
        "a = f(14, 20, 6, 5, 4, 4)\n"
    );
    assert_eq!(
        run("a = f({round(pi * 100) / 100}, {radians(180) - pi})\n"),
        "a = f(3.14, 0)\n"
    );
}

#[test]
fn braces_inside_strings_and_comments_are_left_alone() {
    assert_eq!(
        run("a = f(name=\"{x}\") # {1 + 1}\n"),
        "a = f(name=\"{x}\") # {1 + 1}\n"
    );
}

#[test]
fn a_let_line_keeps_the_line_numbers() {
    assert_eq!(
        run("let a = 1\nlet b = 2\nx = f({a + b})\n")
            .matches('\n')
            .count(),
        3
    );
}

#[test]
fn mistakes_name_the_problem_and_the_fix() {
    let cases = [
        ("a = f({w})\n", "w is not defined"),
        ("a = f({1 / 0})\n", "division by zero"),
        ("a = f({2 +})\n", "a value was expected"),
        ("a = f({2 3})\n", "left over"),
        ("a = f({1 + 2)\n", "no closing"),
        ("a = f({5mm})\n", "unit"),
        ("a = f({bogus(1)})\n", "not a function"),
        ("let pi = 3\n", "cannot name a constant"),
        ("let a = 1\nlet a = 2\n", "already defined"),
        ("let\n", "let name = expression"),
    ];
    for (source, wanted) in cases {
        let failure = expand(source).unwrap_err();
        assert_eq!(failure.code, "parse.expression", "{source}");
        assert!(
            failure.message.contains(wanted),
            "{source}: {}",
            failure.message
        );
        assert!(failure.suggestion.is_some(), "{source}");
    }
}

#[test]
fn errors_point_at_the_expression() {
    let failure = expand("ok = 1\nb = f({w * 2})\n").unwrap_err();
    let span = failure.span.unwrap();
    assert_eq!((span.line, span.col, span.len), (2, 7, 7));
}
