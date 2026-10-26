use super::expand;

fn run(text: &str) -> String {
    expand(text).unwrap().0
}

#[test]
fn the_constants_defined_are_reported() {
    assert_eq!(
        expand("let w = 1\nlet d = 2\nx = f({w})\n").unwrap().1,
        ["d", "w"]
    );
    assert!(expand("x = f(1)\n").unwrap().1.is_empty());
}

#[test]
fn text_without_expressions_is_untouched() {
    let text = "a = solid.box@1(size=[1, 2, 3])\n# {not an expression}\n";
    assert_eq!(run(text), text);
}

#[test]
fn braces_inside_a_string_with_escaped_quotes_are_not_expressions() {
    // A texture program with nested, escaped quotes and a shader loop `for i in 0..6 { w = w * 2 }`.
    let text = "t = solid.texture@1(a, program=\"n = expr(code=\\\"for i in 0..6 { w = w * 2 }\\\")\")\nb = f({1 + 1})\n";
    assert_eq!(
        run(text),
        "t = solid.texture@1(a, program=\"n = expr(code=\\\"for i in 0..6 { w = w * 2 }\\\")\")\nb = f(2)\n"
    );
}

#[test]
fn constants_and_arithmetic_fold_into_numbers() {
    let text = "let w = 1.5\nlet half = w / 2\nb = solid.box@1(size=[{w}, {half}, {w * 2 + 1}])\n";
    assert_eq!(run(text), "\n\nb = solid.box@1(size=[1.5, 0.75, 4])\n");
}

#[test]
fn a_let_may_wrap_its_expression_in_braces() {
    assert_eq!(
        run("let a = 2\nlet b = {a * 3}\nx = f({b}, {a + b})\n"),
        "\n\nx = f(6, 8)\n"
    );
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

#[test]
fn a_text_constant_is_spliced_into_strings() {
    let text =
        "let mirror = \"u = 1.0 - abs(2.0 * p.x - 1.0);\"\nt = f(program=\"${mirror} out = u\")\n";
    assert_eq!(
        run(text),
        "\nt = f(program=\"u = 1.0 - abs(2.0 * p.x - 1.0); out = u\")\n"
    );
}

#[test]
fn a_number_constant_is_spliced_into_strings_too() {
    let text =
        "let grain = 12\nlet half = {grain / 2}\nt = f(program=\"fbm(${grain}, ${half})\")\n";
    assert_eq!(run(text), "\n\nt = f(program=\"fbm(12, 6)\")\n");
}

#[test]
fn text_constants_may_use_earlier_ones_and_carry_escaped_quotes() {
    let text = "let mirror = \"u = p.x;\"\nlet prog = \"n = expr(code=\\\"${mirror} out = u\\\")\"\nt = f(program=\"${prog}; albedo = n\")\n";
    assert_eq!(
        run(text),
        "\n\nt = f(program=\"n = expr(code=\\\"u = p.x; out = u\\\"); albedo = n\")\n"
    );
}

#[test]
fn an_unknown_name_in_a_string_says_which() {
    let error = expand("t = f(program=\"${nope} x\")\n").unwrap_err();
    assert_eq!(error.code, "parse.expression");
    assert!(error.message.contains("nope"), "{}", error.message);
}

#[test]
fn text_constants_are_reported_and_cannot_repeat_a_name() {
    assert_eq!(expand("let a = \"x\"\nlet b = 2\n").unwrap().1, ["a", "b"]);
    assert!(expand("let a = \"x\"\nlet a = 3\n").is_err());
}

#[test]
fn a_dollar_brace_outside_a_string_is_left_alone() {
    assert_eq!(run("let a = 1\nx = f({a}) # ${a}\n"), "\nx = f(1) # ${a}\n");
}
