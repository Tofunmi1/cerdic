//! Evaluates typed field graph nodes, keeping expressions alive until explicit meshing.
use crate::{Fail, Quality, cache::Value, fail};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;

pub(crate) fn execute(
    family: &str,
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
    quality: Quality,
) -> Result<Value, Fail> {
    if family == "field.to_solid" {
        return execute_to_solid(node, inputs, context, quality);
    }
    let field = match family {
        "field.sphere" => ve_geometry::Field::sphere([0.0; 3], number(node, "radius")?),
        "field.ellipsoid" => ve_geometry::Field::ellipsoid(vector(node, "radii")?),
        "field.plane" => {
            ve_geometry::Field::plane(vector(node, "normal")?, number(node, "offset")?)
        }
        "field.box" => {
            ve_geometry::Field::rounded_box(vector(node, "half")?, number(node, "rounding")?)
        }
        "field.torus" => ve_geometry::Field::torus(number(node, "major")?, number(node, "tube")?),
        "field.capsule" => {
            ve_geometry::Field::capsule(number(node, "radius")?, number(node, "half_length")?)
        }
        "field.cylinder" => {
            ve_geometry::Field::cylinder(number(node, "radius")?, number(node, "half_height")?)
        }
        "field.tube" => {
            let radius = number(node, "radius")?;
            // An end radius of zero (the default) keeps the radius constant.
            let end_radius = number(node, "end_radius")
                .ok()
                .filter(|end| *end > 0.0)
                .unwrap_or(radius);
            ve_geometry::Field::tapered_tube(points(node)?, radius, end_radius)
        }
        "field.union" => {
            let mut fields = fields(inputs)?;
            let Some(first) = fields.next() else {
                return Err(fail("graph.missing_input", "field.union needs two fields"));
            };
            Ok(fields.fold(first, ve_geometry::Field::union))
        }
        "field.smooth_union" => {
            let mut fields = fields(inputs)?;
            let a = fields.next().ok_or_else(|| {
                fail("graph.missing_input", "field.smooth_union needs two fields")
            })?;
            let b = fields.next().ok_or_else(|| {
                fail("graph.missing_input", "field.smooth_union needs two fields")
            })?;
            a.smooth_union(b, number(node, "radius")?)
        }
        "field.intersect" | "field.subtract" => {
            let mut fields = fields(inputs)?;
            let a = fields
                .next()
                .ok_or_else(|| fail("graph.missing_input", "field combine needs two fields"))?;
            let b = fields
                .next()
                .ok_or_else(|| fail("graph.missing_input", "field combine needs two fields"))?;
            Ok(if family == "field.intersect" {
                a.intersection(b)
            } else {
                a.difference(b)
            })
        }
        "field.move" => {
            let source = first_field(inputs)?;
            source.moved(vector(node, "by")?)
        }
        "field.scale" => first_field(inputs)?.scaled(number(node, "factor")?),
        "field.mirror" => {
            let axes = vector(node, "axes")?.map(|value| value >= 0.5);
            Ok(first_field(inputs)?.mirrored(axes))
        }
        "field.offset" => first_field(inputs)?.offset(number(node, "amount")?),
        "field.shell" => first_field(inputs)?.shell(number(node, "half_width")?),
        "field.repeat" => {
            first_field(inputs)?.repeated(vector(node, "period")?, repeat_counts(node)?)
        }
        _ => return Err(fail("geometry.unsupported_case", family.to_owned())),
    }
    .map_err(|error| fail("field.invalid_parameter", error.to_string()))?;
    let mut value = Value::nothing();
    value.field = Some(Rc::new(field));
    warn_inexact_distance(family, node, inputs, &mut value);
    Ok(value)
}

fn warn_inexact_distance(family: &str, node: &Node, inputs: &[Rc<Value>], value: &mut Value) {
    if matches!(family, "field.offset" | "field.shell")
        && inputs
            .first()
            .and_then(|input| input.field.as_deref())
            .is_some_and(|source| source.guarantee() != ve_geometry::DistanceGuarantee::Exact)
    {
        value.warnings.push(crate::EvaluationWarning {
            code: "field.distance_inexact".to_owned(),
            node: node.id.to_string(),
            message: format!(
                "{} is applied to a field without an exact Euclidean distance guarantee",
                node.op.family()
            ),
        });
    }
}

fn execute_to_solid(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
    quality: Quality,
) -> Result<Value, Fail> {
    let source = inputs
        .first()
        .and_then(|value| value.field.as_deref())
        .ok_or_else(|| fail("graph.missing_input", "field.to_solid needs a field"))?;
    let bounds = [vector(node, "min")?, vector(node, "max")?];
    let edge = number(node, "edge_length")? * quality.field_edge_scale();
    let solid = ve_geometry::field::mesh(source, bounds, edge, context)
        .map_err(|error| fail(field_code(&error), error.to_string()))?;
    let mesh = solid
        .to_mesh()
        .map_err(|error| fail("kernel.failed", error.to_string()))?;
    Ok(Value::solid(mesh, solid.volume()))
}

fn first_field(inputs: &[Rc<Value>]) -> Result<ve_geometry::Field, Fail> {
    inputs
        .first()
        .and_then(|value| value.field.as_deref())
        .cloned()
        .ok_or_else(|| fail("graph.missing_input", "field operator needs a field"))
}

fn fields(inputs: &[Rc<Value>]) -> Result<impl Iterator<Item = ve_geometry::Field> + '_, Fail> {
    inputs
        .iter()
        .map(|value| {
            value
                .field
                .as_deref()
                .cloned()
                .ok_or_else(|| fail("graph.type_mismatch", "expected a field input"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(std::iter::IntoIterator::into_iter)
}

fn number(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .ok_or_else(|| fail("field.invalid_parameter", format!("missing {name}")))
}

fn vector(node: &Node, name: &str) -> Result<[f64; 3], Fail> {
    let Some(Param::List(values)) = node.params.get(name) else {
        return Err(fail("field.invalid_parameter", format!("missing {name}")));
    };
    if values.len() != 3 {
        return Err(fail(
            "field.invalid_parameter",
            format!("{name} needs three values"),
        ));
    }
    let components = values.iter().map(Param::as_f64).collect::<Option<Vec<_>>>();
    let Some(components) = components else {
        return Err(fail("field.invalid_parameter", format!("invalid {name}")));
    };
    Ok([components[0], components[1], components[2]])
}

fn points(node: &Node) -> Result<Vec<[f64; 3]>, Fail> {
    let Some(Param::List(points)) = node.params.get("points") else {
        return Err(fail("field.invalid_parameter", "missing tube points"));
    };
    points
        .iter()
        .map(|point| {
            let Param::List(components) = point else {
                return Err(fail("field.invalid_parameter", "tube point must be 3D"));
            };
            let values = components
                .iter()
                .map(Param::as_f64)
                .collect::<Option<Vec<_>>>();
            let Some(values) = values.filter(|values| values.len() == 3) else {
                return Err(fail("field.invalid_parameter", "tube point must be 3D"));
            };
            Ok([values[0], values[1], values[2]])
        })
        .collect()
}

fn repeat_counts(node: &Node) -> Result<[u32; 3], Fail> {
    let counts = vector(node, "count")?;
    if counts
        .iter()
        .any(|value| *value < 0.0 || *value > 16.0 || value.fract().to_bits() != 0.0_f64.to_bits())
    {
        return Err(fail(
            "field.invalid_parameter",
            "repeat counts must be integers from 0 to 16",
        ));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "validated repeat counts are integral and bounded by 16"
    )]
    let output = [counts[0] as u32, counts[1] as u32, counts[2] as u32];
    Ok(output)
}

fn field_code(error: &ve_geometry::GeometryError) -> &'static str {
    match error {
        ve_geometry::GeometryError::FieldKernel(ve_kernel::FieldError::Clipped { .. }) => {
            "field.clipped"
        }
        ve_geometry::GeometryError::FieldKernel(ve_kernel::FieldError::Empty) => "field.empty",
        ve_geometry::GeometryError::FieldKernel(ve_kernel::FieldError::Budget) => {
            "budget.field_ops"
        }
        ve_geometry::GeometryError::FieldKernel(ve_kernel::FieldError::Kernel(
            ve_kernel::KernelError::Cancelled,
        ))
        | ve_geometry::GeometryError::Cancelled => "eval.cancelled",
        _ => "field.invalid_parameter",
    }
}
