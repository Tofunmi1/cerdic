//! `poly.skin`: a skeleton of named joints with radii, skinned into one closed surface (Blender's Skin modifier).

use super::spec;
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "poly.skin@1",
        "Skins a skeleton into ONE closed surface of mostly quads (Blender's Skin modifier, the B-Mesh method): give the joints of a body, an animal or a machine frame, each with a position and \
         two radii, and the bones between them, and get a smooth, clean base mesh with proper junctions at every hip, shoulder and branch. Subdivide it with poly.subdivide and shape it with \
         poly.program or poly.sculpt. `joints` is text: one joint per `;` as `name x y z rx rz` (metres; rx and rz are the half extents of the cross-section across and along the bone), with the \
         word `root` after the numbers on one joint of every connected piece (it orients the bone frames; use the pelvis). `bones` is `a-b; b-c; ...` by joint name. Example, a simple figure: \
         joints=\"pelvis 0 0.95 0 0.16 0.1 root; chest 0 1.3 0 0.16 0.1; head 0 1.68 0 0.09 0.1; hip 0.1 0.9 0 0.09 0.09; knee 0.1 0.5 0 0.06 0.06; foot 0.1 0.05 0.05 0.04 0.04\", \
         bones=\"pelvis-chest; chest-head; pelvis-hip; hip-knee; knee-foot\", symmetry=\"x\". Rules learned the hard way: the surface STOPS at an end joint (put the head joint at the top of the head, the foot \
         joint at the sole); several thin bones leaving ONE joint collide, so hang fingers off a chain of knuckles, not a single palm joint; two branch joints need a bone between them. The result has a \
         per-vertex integer attribute `joint` (the index of the joint, in the order written, each vertex was made from) for weights and regions. Errors name the fix.",
        vec![],
        OutputSpec::Fixed(PortType::Poly),
        vec![
            ParamSpec {
                name: "joints",
                ty: ParamType::Str { max_len: 16384 },
                default: Some(Value::Str(
                    "root 0 0 0 0.1 0.1 root; tip 0 1 0 0.1 0.1".to_owned(),
                )),
                doc: "The joints: `name x y z rx rz` separated by `;`, plus the word `root` on one joint of each connected piece. At most 4,096 joints; names are letters, digits and underscores.",
            },
            ParamSpec {
                name: "bones",
                ty: ParamType::Str { max_len: 16384 },
                default: Some(Value::Str("root-tip".to_owned())),
                doc: "The bones: `a-b` by joint name, separated by `;`.",
            },
            ParamSpec {
                name: "branch_smoothing",
                ty: ParamType::Num { unit: Unit::Ratio, min: 0.0, max: 1.0 },
                default: Some(Value::Num(0.0)),
                doc: "Relaxes the surface around branches (0 none, 1 full). Rounds hips and shoulders; costs a little detail.",
            },
            ParamSpec {
                name: "symmetry",
                ty: ParamType::Choice { options: &["none", "x", "y", "z", "xy", "xz", "yz", "xyz"] },
                default: Some(Value::Str("none".to_owned())),
                doc: "Axes of mirror symmetry in the skeleton; keeps quads from crossing the mirror plane so a symmetric figure skins symmetrically. Use `x` for a body.",
            },
            ParamSpec {
                name: "subdivide",
                ty: ParamType::Bool,
                default: Some(Value::Bool(true)),
                doc: "Add joints along long bones so the cross-section rings stay about one radius apart (Blender's behaviour). Turn off to get exactly one ring per joint.",
            },
        ],
    )]
}
