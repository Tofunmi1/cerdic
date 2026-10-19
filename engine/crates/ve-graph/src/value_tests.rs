use super::*;

const LEN: ParamType = ParamType::Num {
    unit: Unit::Length,
    min: 0.0,
    max: 10.0,
};

#[test]
fn numbers_are_canonicalised_and_bounded() {
    assert_eq!(LEN.check(&Value::Int(3)), Ok(Value::Num(3.0)));
    assert_eq!(LEN.check(&Value::Num(3.0)), Ok(Value::Num(3.0)));
    assert_eq!(
        LEN.check(&Value::Num(-1.0)).unwrap_err().code,
        "param.range"
    );
    assert_eq!(
        LEN.check(&Value::Num(f64::NAN)).unwrap_err().code,
        "param.non_finite"
    );
    assert_eq!(
        LEN.check(&Value::Num(f64::INFINITY)).unwrap_err().code,
        "param.non_finite"
    );
    assert_eq!(
        LEN.check(&Value::Str("1".into())).unwrap_err().code,
        "param.type"
    );
}

#[test]
fn integers_do_not_accept_fractions() {
    let t = ParamType::Int { min: 3, max: 512 };
    assert_eq!(t.check(&Value::Int(24)), Ok(Value::Int(24)));
    assert_eq!(t.check(&Value::Int(2)).unwrap_err().code, "param.range");
    assert_eq!(t.check(&Value::Num(24.5)).unwrap_err().code, "param.type");
}

#[test]
fn vectors_and_point_lists_are_checked_per_component() {
    let v3 = ParamType::Vec3 {
        unit: Unit::Length,
        min: -1.0,
        max: 1.0,
    };
    let ok = Value::List(vec![Value::Int(0), Value::Num(0.5), Value::Num(-1.0)]);
    assert!(v3.check(&ok).is_ok());
    let short = Value::List(vec![Value::Int(0)]);
    assert_eq!(v3.check(&short).unwrap_err().code, "param.type");
    let big = Value::List(vec![Value::Int(0), Value::Int(0), Value::Int(9)]);
    assert_eq!(v3.check(&big).unwrap_err().code, "param.range");

    let pts = ParamType::Points2 {
        min_points: 3,
        max_points: 4,
        min: -1.0,
        max: 1.0,
    };
    let pt = |x: f64, y: f64| Value::List(vec![Value::Num(x), Value::Num(y)]);
    assert!(
        pts.check(&Value::List(vec![pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)]))
            .is_ok()
    );
    assert_eq!(
        pts.check(&Value::List(vec![pt(0.0, 0.0)]))
            .unwrap_err()
            .code,
        "param.range"
    );
    let five = Value::List((0..5).map(|_| pt(0.0, 0.0)).collect());
    assert_eq!(pts.check(&five).unwrap_err().code, "param.range");
}

#[test]
fn text_is_bounded_in_bytes() {
    let t = ParamType::Str { max_len: 4 };
    assert!(t.check(&Value::Str("abcd".into())).is_ok());
    assert_eq!(
        t.check(&Value::Str("abcde".into())).unwrap_err().code,
        "param.range"
    );
}

#[test]
fn points3_checks_every_coordinate_and_rejects_two_dimensions() {
    let points = ParamType::Points3 {
        min_points: 2,
        max_points: 3,
        min: -2.0,
        max: 2.0,
    };
    let point = |x, y, z| Value::List(vec![Value::Num(x), Value::Num(y), Value::Num(z)]);
    assert!(
        points
            .check(&Value::List(vec![
                point(0.0, 1.0, 2.0),
                point(-1.0, 0.0, 1.0)
            ]))
            .is_ok()
    );
    assert_eq!(
        points
            .check(&Value::List(vec![
                Value::List(vec![
                    Value::Num(0.0),
                    Value::Num(0.0)
                ]);
                2
            ]))
            .unwrap_err()
            .code,
        "param.type"
    );
    assert_eq!(
        points
            .check(&Value::List(vec![point(0.0, 0.0, 3.0); 2]))
            .unwrap_err()
            .code,
        "param.range"
    );
}
