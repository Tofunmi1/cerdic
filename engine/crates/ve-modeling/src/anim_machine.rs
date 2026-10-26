//! State machine graph node: validated machine data rides on its input shape.
use crate::{Fail, cache::Value, fail};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};

pub(crate) fn machine(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "anim.machine needs a shape"))?;
    let Some(Param::Str(program)) = node.params.get("program") else {
        return Err(fail("param.type", "anim.machine needs a program string"));
    };
    if program.len() > 4096 {
        return Err(fail(
            "param.range",
            "anim.machine program exceeds 4096 bytes",
        ));
    }
    let normalized = program.replace(';', "\n");
    let machine = ve_animstate::StateMachine::parse(&normalized).map_err(|error| {
        Box::new(
            ve_graph::Diagnostic::error("anim.machine", error.to_string())
                .suggest("write state NAME clip=CLIP and transition A -> B when ...")
                .at_node(&node.id),
        )
    })?;
    let mut value = (**input).clone();
    value.state_machine = Some(machine);
    value.state_machine_node = Some(node.id.clone());
    Ok(value)
}
