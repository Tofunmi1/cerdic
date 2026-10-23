//! Additional field-expression contract coverage.
use ve_world::expr::{Bindings, Expr, Sample, Type};

fn sample(index: usize) -> Sample<'static> {
    let h = |key: u32| {
        let index = u32::try_from(index).unwrap_or(0);
        let mut value = index
            .wrapping_mul(2_654_435_761)
            .wrapping_add(key.wrapping_mul(0x9E37_79B9));
        value ^= value >> 15;
        value = value.wrapping_mul(0x2C1B_3C6D);
        value ^= value >> 12;
        f32::from(u16::try_from(value & 0xFFFF).unwrap_or(0)) / 65_535.0 * 40.0 - 20.0
    };
    Sample {
        p: [h(1), h(2), h(3)],
        ..Sample::default()
    }
}

fn rough(octaves: usize) -> f32 {
    let Ok(expr) = Expr::compile(
        &format!("out = fbm(p, {octaves}.0, 1.0)"),
        &Bindings::default(),
    ) else {
        return f32::NAN;
    };
    (0..1500)
        .map(|i| {
            let a = expr.eval(&sample(i)).floats()[0];
            let mut near = sample(i);
            near.p[0] += 0.02;
            (a - expr.eval(&near).floats()[0]).abs()
        })
        .sum::<f32>()
        / 1500.0
}

#[test]
fn detail_roughness_and_additional_contract_cases() {
    println!("roughness one={} six={}", rough(1), rough(6));
    assert!(Expr::compile("out = ramp(p.x, 1.0, 0.0, 0.0, 1.0)", &Bindings::default()).is_err());
    assert!(Expr::compile("out = (((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((p.x))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))", &Bindings::default()).is_err());
    let expr = Expr::compile(
        "out = attr(\"a\") + attr3(\"b\").x + attr2(\"a\").x",
        &Bindings::default(),
    )
    .unwrap();
    assert_eq!(
        expr.attributes(),
        &[("a".to_owned(), Type::Float), ("b".to_owned(), Type::Vec3)]
    );
}

#[test]
fn fixed_arity_functions_reject_wrong_argument_counts() {
    let invalid = [
        "abs()",
        "sign(1.0, 2.0)",
        "floor()",
        "ceil(1.0, 2.0)",
        "fract()",
        "sqrt(1.0, 2.0)",
        "exp()",
        "log(1.0, 2.0)",
        "sin()",
        "cos(1.0, 2.0)",
        "atan2(1.0)",
        "pow(1.0)",
        "min(1.0)",
        "max(1.0, 2.0, 3.0)",
        "clamp(1.0, 2.0)",
        "mix(1.0, 2.0)",
        "step(1.0)",
        "smoothstep(1.0, 2.0)",
        "length()",
        "distance(p)",
        "dot(p)",
        "cross(p)",
        "normalize()",
        "select(true, 1.0)",
        "perlin(p)",
        "fbm(p, 2.0)",
        "ridged(p, 2.0)",
        "voronoi(p)",
        "attr()",
        "attr2(\"x\", \"y\")",
    ];
    for call in invalid {
        let source = format!("out = {call}");
        assert!(
            Expr::compile(&source, &Bindings::default()).is_err(),
            "accepted {call}"
        );
    }
}

#[test]
fn swizzles_enforce_vector_bounds_and_allow_aliases() {
    assert!(Expr::compile("out = p.rbg", &Bindings::default()).is_ok());
    assert!(Expr::compile("out = uv.z", &Bindings::default()).is_err());
    assert!(Expr::compile("out = 2.0.x", &Bindings::default()).is_err());
    assert!(Expr::compile("out = p.xyzw", &Bindings::default()).is_err());
}
