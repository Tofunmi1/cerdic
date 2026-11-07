//! Typed API declarations generated from the operator registry (G3): one TypeScript declaration file that says, for every ready operator, what it is called, which inputs it takes and what
//! kinds of values they accept, which parameters it has with their types, choices and ranges, and what it returns. A script, a tool schema or an editor completes and checks calls against it, and it
//! cannot drift from the engine because it is made from the same registry the evaluator validates against.
//!
//! The shape: `ve.solid.box({ size: [1, 1, 1] })` makes a node; an operator with one input takes it first (`ve.xform.move(wall, { by: [0, 1, 0] })`), one with several takes an object of ports
//! (`ve.scatter.place({ in: tree, surface: land }, { radius: 3 })`). A number parameter also accepts a `Number` node (a `value.*` result). A node's kind is in its type, so a polygon mesh
//! cannot go where only a solid is accepted.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::spec::{
    Arity, OperatorRegistry as Registry, OperatorSpec, OutputSpec, PortSpec, PortType,
};
use crate::value::{ParamType, Unit, Value};

fn kind(port: PortType) -> String {
    format!("{port:?}")
}

fn kinds(accepts: &[PortType]) -> String {
    let all: Vec<String> = accepts.iter().map(|p| format!("'{}'", kind(*p))).collect();
    format!("Node<{}>", all.join(" | "))
}

fn comment(text: &str, indent: &str) -> String {
    let single = text.replace("*/", "* /").replace('\n', " ");
    format!("{indent}/** {} */\n", single.trim())
}

fn first_sentences(doc: &str) -> String {
    // The whole documentation is long (it is what a model reads); the declaration keeps the first two sentences.
    let mut taken = String::new();
    for (i, sentence) in doc.split_inclusive(". ").enumerate() {
        if i == 2 {
            break;
        }
        taken.push_str(sentence);
    }
    taken.trim().to_owned()
}

fn unit_word(unit: Unit) -> &'static str {
    match unit {
        Unit::None => "",
        Unit::Length => "metres",
        Unit::Angle => "an angle",
        Unit::Ratio => "a ratio",
    }
}

fn number(v: f64) -> String {
    if (v - v.round()).abs() < 1.0e-9 && v.abs() < 1.0e15 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// The TypeScript type of a parameter, and a note on its range.
fn param_type(ty: &ParamType) -> (String, String) {
    match ty {
        ParamType::Num { unit, min, max } => {
            let unit = unit_word(*unit);
            (
                "number | Node<'Number'>".to_owned(),
                format!(
                    "{}{}..{}",
                    if unit.is_empty() {
                        String::new()
                    } else {
                        format!("{unit}, ")
                    },
                    number(*min),
                    number(*max)
                ),
            )
        }
        ParamType::Int { min, max } => (
            "number | Node<'Number'>".to_owned(),
            format!("whole number {min}..{max}"),
        ),
        ParamType::Bool => ("boolean".to_owned(), String::new()),
        ParamType::Str { max_len } => ("string".to_owned(), format!("text up to {max_len} bytes")),
        ParamType::StrList { max_items, max_len } => (
            "string[]".to_owned(),
            format!("up to {max_items} texts of {max_len} bytes"),
        ),
        ParamType::Choice { options } => (
            options
                .iter()
                .map(|o| format!("'{}'", o.replace('\'', "\\'")))
                .collect::<Vec<_>>()
                .join(" | "),
            String::new(),
        ),
        ParamType::Vec2 { unit, min, max } => (
            "Vec2".to_owned(),
            format!(
                "{}{}..{} each",
                {
                    let u = unit_word(*unit);
                    if u.is_empty() {
                        String::new()
                    } else {
                        format!("{u}, ")
                    }
                },
                number(*min),
                number(*max)
            ),
        ),
        ParamType::Vec3 { unit, min, max } => (
            "Vec3".to_owned(),
            format!(
                "{}{}..{} each",
                {
                    let u = unit_word(*unit);
                    if u.is_empty() {
                        String::new()
                    } else {
                        format!("{u}, ")
                    }
                },
                number(*min),
                number(*max)
            ),
        ),
        ParamType::Points2 {
            min_points,
            max_points,
            ..
        } => (
            "Vec2[]".to_owned(),
            format!("{min_points} to {max_points} points"),
        ),
        ParamType::Points3 {
            min_points,
            max_points,
            ..
        } => (
            "Vec3[]".to_owned(),
            format!("{min_points} to {max_points} points"),
        ),
    }
}

fn default_note(default: Option<&Value>) -> String {
    match default {
        Some(Value::Num(n)) => format!("default {}", number(*n)),
        Some(Value::Int(n)) => format!("default {n}"),
        Some(Value::Bool(b)) => format!("default {b}"),
        Some(Value::Str(s)) if s.len() <= 24 => format!("default \"{s}\""),
        Some(Value::List(items)) if items.len() <= 3 => {
            let parts: Vec<String> = items
                .iter()
                .map(|v| match v {
                    Value::Num(n) => number(*n),
                    Value::Int(n) => n.to_string(),
                    _ => "..".to_owned(),
                })
                .collect();
            format!("default [{}]", parts.join(", "))
        }
        Some(_) => "has a default".to_owned(),
        None => "required".to_owned(),
    }
}

fn pascal(id: &str) -> String {
    id.split(['.', '_', '@'])
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            c.next().map_or_else(String::new, |f| {
                f.to_uppercase().collect::<String>() + c.as_str()
            })
        })
        .collect()
}

/// `solid.box@1` as ("solid", "box", 1).
fn split_id(id: &str) -> (&str, &str, u32) {
    let (name, version) = id.split_once('@').unwrap_or((id, "1"));
    let (family, op) = name.split_once('.').unwrap_or(("", name));
    (family, op, version.parse().unwrap_or(1))
}

fn port_type(port: &PortSpec) -> String {
    let base = kinds(port.accepts);
    match port.arity {
        Arity::Variadic { .. } => format!("{base}[]"),
        _ => base,
    }
}

fn output_type(spec: &OperatorSpec, generic: Option<&str>) -> String {
    match spec.output {
        OutputSpec::Fixed(p) => format!("Node<'{}'>", kind(p)),
        OutputSpec::SameAsInput(port) => {
            match (generic, spec.inputs.iter().find(|i| i.name == port)) {
                (Some(g), _) => g.to_owned(),
                (None, Some(p)) => kinds(p.accepts),
                (None, None) => "Node".to_owned(),
            }
        }
    }
}

/// The declaration file for every ready operator of `registry`.
#[must_use]
pub fn operators_d_ts(registry: &Registry) -> String {
    let mut out = String::new();
    out.push_str(
        "// Generated from the operator registry (ve-graph). Run bun run schema:generate.\n",
    );
    out.push_str("// Typed calls for every ready operator: `ve.solid.box({ size: [1, 1, 1] })` makes a node; the kinds of value are in the types.\n\n");
    out.push_str(
        "export type Vec2 = [number, number];\nexport type Vec3 = [number, number, number];\n\n",
    );
    let all_kinds: Vec<String> = [
        PortType::Solid,
        PortType::Profile,
        PortType::Curve,
        PortType::Sdf,
        PortType::Field,
        PortType::Inst,
        PortType::Light,
        PortType::Environment,
        PortType::Check,
        PortType::Poly,
        PortType::Selection,
        PortType::Number,
        PortType::Pattern,
        PortType::Game,
        PortType::Volume,
    ]
    .iter()
    .map(|p| format!("'{}'", kind(*p)))
    .collect();
    let _ = writeln!(
        out,
        "/** The kinds of value a node can make. */\nexport type Kind = {};\n",
        all_kinds.join(" | ")
    );
    let mut ids: Vec<String> = registry
        .ready()
        .map(|spec| format!("'{}'", spec.id))
        .collect();
    ids.sort();
    let _ = writeln!(
        out,
        "/** The id of every ready operator, `family.name@version`. */\nexport type OperatorId = {};\n",
        ids.join("\n  | ")
    );
    out.push_str("declare const kindOf: unique symbol;\n/** A node of a program; `K` is the kind (or kinds) of value it makes. */\nexport interface Node<K extends Kind = Kind> {\n  readonly [kindOf]?: K;\n  readonly id: string;\n  readonly label: string;\n}\n\n");
    // Group by family; a name used by two versions gets a version suffix on all but the first.
    let mut families: BTreeMap<String, Vec<(&OperatorSpec, String)>> = BTreeMap::new();
    let mut specs: Vec<&OperatorSpec> = registry.ready().collect();
    specs.sort_by_key(|spec| spec.id.to_string());
    for spec in specs {
        let id = spec.id.to_string();
        let (family, name, version) = split_id(&id);
        let entry = families.entry(family.to_owned()).or_default();
        let key = if entry.iter().any(|(_, k)| k == name) {
            format!("{name}_v{version}")
        } else {
            name.to_owned()
        };
        entry.push((spec, key));
    }
    let mut body = String::new();
    for (family, ops) in &families {
        let _ = writeln!(body, "  {family}: {{");
        for (spec, key) in ops {
            let id = spec.id.to_string();
            let (_, name, version) = split_id(&id);
            let params_name = format!("{}Params", pascal(&format!("{family}.{name}_v{version}")));
            // Parameters interface.
            let _ = writeln!(
                out,
                "{}export interface {params_name} {{",
                comment(&first_sentences(spec.doc), "")
            );
            for param in &spec.params {
                let (ty, range) = param_type(&param.ty);
                let note = [default_note(param.default.as_ref()), range]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("; ");
                out.push_str(&comment(
                    &format!("{} ({note})", param.doc.trim().trim_end_matches('.')),
                    "  ",
                ));
                let optional = if param.default.is_some() { "?" } else { "" };
                let _ = writeln!(out, "  {}{optional}: {ty};", property(param.name));
            }
            out.push_str("}\n\n");
            // The call.
            let params_optional = spec.params.iter().all(|p| p.default.is_some());
            let params_arg = if spec.params.is_empty() {
                String::new()
            } else {
                format!(
                    "params{}: {params_name}",
                    if params_optional { "?" } else { "" }
                )
            };
            let ports = &spec.inputs;
            let (inputs_arg, generic) = match ports.as_slice() {
                [] => (String::new(), None),
                [one] => {
                    let ty = port_type(one);
                    let optional = matches!(one.arity, Arity::Optional);
                    // An optional argument cannot come before a required one, so an optional input in front of required parameters is `| undefined` instead.
                    let leave_out = optional && (params_optional || spec.params.is_empty());
                    let name = if leave_out { "input?" } else { "input" };
                    let ty_or_none = |t: String| {
                        if optional && !leave_out {
                            format!("{t} | undefined")
                        } else {
                            t
                        }
                    };
                    if matches!(spec.output, OutputSpec::SameAsInput(p) if p == one.name)
                        && !matches!(one.arity, Arity::Variadic { .. })
                    {
                        (
                            format!("{name}: {}", ty_or_none("T".to_owned())),
                            Some(("T", ty)),
                        )
                    } else {
                        (format!("{name}: {}", ty_or_none(ty)), None)
                    }
                }
                many => {
                    let fields: Vec<String> = many
                        .iter()
                        .map(|p| {
                            format!(
                                "{}{}: {}",
                                property(p.name),
                                if matches!(p.arity, Arity::Optional) {
                                    "?"
                                } else {
                                    ""
                                },
                                port_type(p)
                            )
                        })
                        .collect();
                    (format!("inputs: {{ {} }}", fields.join("; ")), None)
                }
            };
            let signature = [inputs_arg, params_arg]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            let ret = output_type(spec, generic.as_ref().map(|g| g.0));
            let type_params = generic.as_ref().map_or_else(String::new, |(name, bound)| {
                format!("<{name} extends {bound}>")
            });
            body.push_str(&comment(&first_sentences(spec.doc), "    "));
            let _ = writeln!(
                body,
                "    {}{type_params}({signature}): {ret};",
                property(key)
            );
        }
        body.push_str("  };\n");
    }
    let _ = write!(
        out,
        "/** Every ready operator, by family and name: `ve.poly.select(wall, {{ element: 'face' }})`. */\nexport interface Ve {{\n{body}}}\n\ndeclare const ve: Ve;\nexport default ve;\n"
    );
    out
}

/// A property name, quoted only when it is not a plain identifier.
fn property(name: &str) -> String {
    if name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        name.to_owned()
    } else {
        format!("'{name}'")
    }
}
