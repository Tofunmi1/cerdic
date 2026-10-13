//! The two operators groups use. Neither is made with `add_node` freely: `group.use@1` only through
//! `add_instance` (its group, version and overrides live on the node, and its inputs are the group's),
//! and `group.input@1` only inside a group.

use super::spec;
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::ParamType;

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "group.use@1",
            "An instance of a group: the group's result, with the parameters it exposes.",
            vec![],
            OutputSpec::Fixed(PortType::Solid),
            vec![],
        ),
        spec(
            "group.input@1",
            "A solid that an instance of the group supplies, by this name.",
            vec![],
            OutputSpec::Fixed(PortType::Solid),
            vec![ParamSpec {
                name: "name",
                ty: ParamType::Str { max_len: 64 },
                default: None,
                doc: "The input's name; an instance links to it by this name.",
            }],
        ),
    ]
}
