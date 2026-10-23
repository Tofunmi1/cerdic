//! Typed operation evaluation for expression calls and operators.
use super::{Bindings, Sample, Type, Value, math, noise, parser::Node};

pub(super) fn binary(code: u8, left: Value, right: Value, ty: Type) -> Value {
    if code >= 5 {
        let a = left.array()[0];
        let b = right.array()[0];
        let result = match code {
            5 => left == right,
            6 => left != right,
            7 => a < b,
            8 => a <= b,
            9 => a > b,
            10 => a >= b,
            11 => matches!(left, Value::Bool(true)) && matches!(right, Value::Bool(true)),
            _ => matches!(left, Value::Bool(true)) || matches!(right, Value::Bool(true)),
        };
        return Value::Bool(result);
    }
    let a = left.array();
    let b = right.array();
    let mut out = [0.0; 4];
    for i in 0..ty.size() {
        let x = if left.ty() == Type::Float { a[0] } else { a[i] };
        let y = if right.ty() == Type::Float {
            b[0]
        } else {
            b[i]
        };
        out[i] = match code {
            0 => x + y,
            1 => x - y,
            2 => x * y,
            3 => {
                if y == 0.0 {
                    0.0
                } else {
                    x / y
                }
            }
            _ => {
                if y == 0.0 {
                    0.0
                } else {
                    x - y * (x / y).floor()
                }
            }
        };
    }
    finite(Value::from_array(ty, out))
}

pub(super) fn call(
    name: &str,
    args: &[Node],
    sample: &Sample<'_>,
    bindings: &Bindings,
    ty: Type,
) -> Value {
    let values: Vec<Value> = args.iter().map(|arg| arg.eval(sample, bindings)).collect();
    if name.starts_with("vec") {
        return construct(&values, ty);
    }
    if name == "ramp" || name == "curve" {
        return ramp(&values, ty);
    }
    if matches!(name, "perlin" | "fbm" | "ridged" | "voronoi") {
        return Value::Float(noise_call(name, &values));
    }
    if name == "select" {
        return if matches!(values.first(), Some(Value::Bool(true))) {
            values.get(1).copied().unwrap_or(Value::Float(0.0))
        } else {
            values.get(2).copied().unwrap_or(Value::Float(0.0))
        };
    }
    if name == "cross" {
        let a = values
            .first()
            .copied()
            .unwrap_or(Value::Vec3([0.0; 3]))
            .array();
        let b = values
            .get(1)
            .copied()
            .unwrap_or(Value::Vec3([0.0; 3]))
            .array();
        return Value::Vec3([
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]);
    }
    if name == "dot" || name == "length" || name == "distance" || name == "normalize" {
        return geometry(name, &values, ty);
    }
    component_call(name, &values, ty)
}

fn construct(values: &[Value], ty: Type) -> Value {
    let size = ty.size();
    let mut out = [0.0; 4];
    if values.len() == 1 && values[0].ty() == Type::Float {
        out[..size].fill(values[0].array()[0]);
        return Value::from_array(ty, out);
    }
    let mut at = 0;
    for value in values {
        let a = value.array();
        for component in a.iter().take(value.ty().size()) {
            if at < size {
                out[at] = *component;
                at += 1;
            }
        }
    }
    Value::from_array(ty, out)
}

fn ramp(values: &[Value], ty: Type) -> Value {
    if values.len() < 5 {
        return Value::Float(0.0);
    }
    let x = values[0].array()[0];
    let first = values[2];
    let stop_count = (values.len() - 1) / 2;
    let last = values[stop_count * 2];
    if x <= values[1].array()[0] {
        return first;
    }
    let last_x = values[(stop_count - 1) * 2 + 1].array()[0];
    if x >= last_x {
        return last;
    }
    for stop in 0..stop_count - 1 {
        let x0 = values[1 + stop * 2].array()[0];
        let x1 = values[3 + stop * 2].array()[0];
        if x <= x1 {
            let a = values[2 + stop * 2].array();
            let b = values[4 + stop * 2].array();
            let t = if x1.to_bits() == x0.to_bits() {
                0.0
            } else {
                (x - x0) / (x1 - x0)
            };
            let mut output = [0.0; 4];
            for i in 0..ty.size() {
                output[i] = a[i] + (b[i] - a[i]) * t;
            }
            return finite(Value::from_array(ty, output));
        }
    }
    last
}

fn noise_call(name: &str, values: &[Value]) -> f32 {
    let p = values
        .first()
        .copied()
        .unwrap_or(Value::Vec3([0.0; 3]))
        .array();
    let point = [p[0], p[1], p[2]];
    let seed_value = if name == "fbm" || name == "ridged" {
        values.get(2).copied().unwrap_or(Value::Float(0.0)).array()[0]
    } else {
        values.get(1).copied().unwrap_or(Value::Float(0.0)).array()[0]
    };
    let seed = noise::seed(seed_value);
    match name {
        "perlin" => noise::perlin(point, seed),
        "fbm" => noise::fbm(
            point,
            noise::octaves(values.get(1).copied().unwrap_or(Value::Float(1.0)).array()[0]),
            seed,
            false,
        ),
        "ridged" => noise::fbm(
            point,
            noise::octaves(values.get(1).copied().unwrap_or(Value::Float(1.0)).array()[0]),
            seed,
            true,
        ),
        _ => noise::voronoi(point, seed),
    }
}

fn geometry(name: &str, values: &[Value], ty: Type) -> Value {
    let a = values.first().copied().unwrap_or(Value::Float(0.0)).array();
    let b = values.get(1).copied().unwrap_or(Value::Float(0.0)).array();
    let size = values.first().map_or(1, |value| value.ty().size());
    let mut sum = 0.0;
    for i in 0..size {
        let d = if name == "distance" {
            a[i] - b[i]
        } else {
            a[i]
        };
        sum += d * d;
    }
    if name == "dot" {
        return Value::Float((0..size).map(|i| a[i] * b[i]).sum());
    }
    if name == "length" || name == "distance" {
        return Value::Float(sum.sqrt());
    }
    let length = sum.sqrt();
    if length == 0.0 {
        return Value::from_array(ty, [0.0; 4]);
    }
    let mut out = a;
    for item in &mut out[..ty.size()] {
        *item /= length;
    }
    Value::from_array(ty, out)
}

fn component_call(name: &str, values: &[Value], ty: Type) -> Value {
    let first = values.first().copied().unwrap_or(Value::Float(0.0));
    let second = values.get(1).copied().unwrap_or(Value::Float(0.0));
    let third = values.get(2).copied().unwrap_or(Value::Float(0.0));
    let first_array = first.array();
    let second_array = second.array();
    let third_array = third.array();
    let mut out = [0.0; 4];
    for i in 0..ty.size() {
        let first_component = if first.ty() == Type::Float {
            first_array[0]
        } else {
            first_array[i]
        };
        let second_component = if second.ty() == Type::Float {
            second_array[0]
        } else {
            second_array[i]
        };
        let third_component = if third.ty() == Type::Float {
            third_array[0]
        } else {
            third_array[i]
        };
        out[i] = match name {
            "abs" => first_component.abs(),
            "sign" => first_component.signum(),
            "floor" => first_component.floor(),
            "ceil" => first_component.ceil(),
            "fract" => first_component - first_component.floor(),
            "sqrt" => {
                if first_component < 0.0 {
                    0.0
                } else {
                    first_component.sqrt()
                }
            }
            "exp" => math::exp(first_component.clamp(-100.0, 88.0)),
            "log" => math::log(first_component),
            "pow" => math::pow(first_component, second_component),
            "sin" => math::sin(first_component),
            "cos" => math::cos(first_component),
            "atan2" => math::atan2(first_component, second_component),
            "min" => first_component.min(second_component),
            "max" => first_component.max(second_component),
            "clamp" => first_component.clamp(
                second_component.min(third_component),
                second_component.max(third_component),
            ),
            "mix" => first_component + (second_component - first_component) * third_component,
            "step" => {
                if second_component >= first_component {
                    1.0
                } else {
                    0.0
                }
            }
            "smoothstep" => {
                let t = if second_component.to_bits() == first_component.to_bits() {
                    0.0
                } else {
                    ((third_component - first_component) / (second_component - first_component))
                        .clamp(0.0, 1.0)
                };
                t * t * (3.0 - 2.0 * t)
            }
            _ => 0.0,
        };
    }
    finite(Value::from_array(ty, out))
}

fn finite(value: Value) -> Value {
    let mut array = value.array();
    for x in &mut array {
        if !x.is_finite() {
            *x = if x.is_sign_positive() {
                f32::MAX
            } else {
                -f32::MAX
            };
        }
    }
    Value::from_array(value.ty(), array)
}
