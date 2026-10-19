use super::{COLOURABLE, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec};
use crate::value::{ParamType, Unit, Value};

pub(super) fn specs() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "behavior.on@1",
        "Makes a shape react in play mode: when it is clicked (`select`) or the viewer comes near it (`near`), it runs its own anim.track clip. \
                     Put it after the anim.track nodes (a switch with a `target` needs no track of its own). `play` runs the clip once and holds the end; `toggle` runs forward, then backward on the next trigger \
                     (a door that opens and closes). The shape no longer follows the shared looping clock; it rests at the clip's start until triggered.",
        vec![single("in", COLOURABLE)],
        OutputSpec::SameAsInput("in"),
        vec![
            ParamSpec {
                name: "trigger",
                ty: ParamType::Choice {
                    options: &[
                        "select", "near", "signal", "flag", "grab", "release", "hold", "timer",
                        "socket", "collide",
                    ],
                },
                default: Some(Value::Str("select".to_owned())),
                doc: "What starts it: select (clicked in play mode), near (the viewer comes within `radius`), signal (only another part's behavior.on that names this part as its `target` starts it), flag (the flag named in `watch` changes), grab (a hand or the mouse picks the part up in play mode; it can then be carried around and is put back when play stops), release (it is let go), hold (both: `sets` apply when picked up and reverse when let go, and the clip runs forward while held and back when released), timer (fires `after` seconds after play mode starts, then every `every` seconds if set) or socket (the part named in `accepts`, which must itself be grabbable with trigger grab, release or hold, snaps onto this part when let go within `radius` of it, and this fires like hold: `sets` as written and the clip forward; picking it out again reverses them) or collide (this part, which needs phys.body, touches another body in play mode: any body, or only the part named in `accepts`; fires when the touching starts, so a ball hitting a bell or a door slamming can light a lamp or set a flag).",
            },
            ParamSpec {
                name: "accepts",
                ty: ParamType::Str { max_len: 64 },
                default: Some(Value::Str(String::new())),
                doc: "With trigger=\"socket\": the output name of the grabbable part that fits in this socket (a key for a lock). With trigger=\"collide\": optional, the output name of the one part whose touch counts (empty means touching anything).",
            },
            ParamSpec {
                name: "clip",
                ty: ParamType::Str { max_len: 24 },
                default: Some(Value::Str(String::new())),
                doc: "Which of this part's own clips (the `clip` of its anim.track nodes) this behaviour runs; empty is the main clip. A part with clips \"wave\" and \"reach\" can have a lever that runs one and a button that runs the other, by naming the clip on the target (target=\"arm:wave\").",
            },
            ParamSpec {
                name: "then",
                ty: ParamType::Str { max_len: 24 },
                default: Some(Value::Str(String::new())),
                doc: "A different clip of this part to start when the clip this behaviour runs reaches its end, for a sequence: \"open\" then \"wave\" then \"close\" by chaining. The chained clip runs in the same direction.",
            },
            ParamSpec {
                name: "after",
                ty: ParamType::Num {
                    unit: Unit::None,
                    min: 0.0,
                    max: 3600.0,
                },
                default: Some(Value::Num(0.0)),
                doc: "With trigger=\"timer\": seconds after play mode starts that it first fires.",
            },
            ParamSpec {
                name: "every",
                ty: ParamType::Num {
                    unit: Unit::None,
                    min: 0.0,
                    max: 3600.0,
                },
                default: Some(Value::Num(0.0)),
                doc: "With trigger=\"timer\": seconds between later firings (0.1 or more); 0 fires only once. A toggle with `every` blinks, opens and closes, or alternates forever.",
            },
            ParamSpec {
                name: "sets",
                ty: ParamType::Str { max_len: 160 },
                default: Some(Value::Str(String::new())),
                doc: "Flags to change when this fires, `name=on`, `name=off` or `name=toggle`, separated by semicolons (a bare name means on). Flags are scene-wide and start off. Names are lowercase letters, digits and underscores, up to 24 long, at most 8 per behaviour.",
            },
            ParamSpec {
                name: "needs",
                ty: ParamType::Str { max_len: 160 },
                default: Some(Value::Str(String::new())),
                doc: "Flags that must be in this state for the trigger to do anything: `key` (on), `!alarm` or `alarm=off` (off), separated by semicolons. A locked door needs the flag its key sets.",
            },
            ParamSpec {
                name: "watch",
                ty: ParamType::Str { max_len: 24 },
                default: Some(Value::Str(String::new())),
                doc: "With trigger=\"flag\": the flag to watch. The behaviour fires every time that flag changes, and cannot set it itself.",
            },
            ParamSpec {
                name: "target",
                ty: ParamType::Str { max_len: 64 },
                default: Some(Value::Str(String::new())),
                doc: "The output name of another part whose clip this behaviour runs (a switch that opens a door or lights a lamp). Empty runs this part's own clip. Write `name:clip` to run one named clip of the target (\"arm:wave\"). The target part needs an anim.track and a behavior.on with trigger=\"signal\".",
            },
            ParamSpec {
                name: "action",
                ty: ParamType::Choice {
                    options: &["play", "toggle", "follow"],
                },
                default: Some(Value::Str("toggle".to_owned())),
                doc: "play runs the clip once and holds the end; toggle alternates forward and backward; follow (only with trigger=\"flag\") runs forward while the watched flag is on and backward while it is off, so a lamp always matches its switch.",
            },
            ParamSpec {
                name: "radius",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.1,
                    max: 100.0,
                },
                default: Some(Value::Num(2.0)),
                doc: "Metres from the shape's centre for a `near` trigger.",
            },
        ],
    )]
}
