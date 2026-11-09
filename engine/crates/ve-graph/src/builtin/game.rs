//! Game definition fragments. `game.collect` joins them for the evaluator.
use super::spec;
use crate::spec::{Arity, OperatorSpec, OutputSpec, ParamSpec, PortSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const GAME: &[PortType] = &[PortType::Game];
mod kind;
mod navigation;
const NUMBER: ParamType = ParamType::Num {
    unit: Unit::None,
    min: -1.0e9,
    max: 1.0e9,
};
const TEXT: ParamType = ParamType::Str { max_len: 4096 };

fn param(
    name: &'static str,
    ty: ParamType,
    default: Option<Value>,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty,
        default,
        doc,
    }
}

fn game_node(id: &str, doc: &'static str, params: Vec<ParamSpec>) -> Option<OperatorSpec> {
    spec(id, doc, vec![], OutputSpec::Fixed(PortType::Game), params)
}

fn var() -> Option<OperatorSpec> {
    game_node(
        "game.var@1",
        "Declare a numeric game variable. Example: `score = game.var@1(name=\"score\", value=0)`.",
        vec![
            param("name", TEXT, None, "Variable name."),
            param("value", NUMBER, None, "Initial numeric value."),
        ],
    )
}

fn spawn() -> Option<OperatorSpec> {
    spec(
        "game.spawn@1",
        "Place entities of a declared kind, at a point or one per placement of a set (scatter.place, inst.array ...), standing exactly where each placement is, so pickups sit on hilly ground. heading sets the initial facing in degrees. Examples: `coins = game.spawn@1(kind=\"coin\", at=[3, 0, 2], count=8, spread=6)` and `coins = game.spawn@1(scattered, kind=\"coin\")`.",
        vec![PortSpec {
            name: "places",
            accepts: &[PortType::Inst],
            arity: Arity::Optional,
        }],
        OutputSpec::Fixed(PortType::Game),
        vec![
            param("kind", TEXT, None, "Declared kind to spawn."),
            param(
                "at",
                ParamType::Vec3 {
                    unit: Unit::Length,
                    min: -1.0e6,
                    max: 1.0e6,
                },
                Some(Value::List(vec![Value::Num(0.0); 3])),
                "Spawn position in metres (ignored when a set is linked).",
            ),
            param(
                "count",
                ParamType::Int { min: 1, max: 2048 },
                Some(Value::Int(1)),
                "Number of entities.",
            ),
            param(
                "spread",
                ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 1.0e6,
                },
                Some(Value::Num(0.0)),
                "Scatter square width in metres.",
            ),
            param(
                "heading",
                ParamType::Num {
                    unit: Unit::None,
                    min: -360.0,
                    max: 360.0,
                },
                Some(Value::Num(0.0)),
                "Initial facing direction in degrees.",
            ),
            param("level", TEXT, Some(Value::Str(String::new())), "Belongs to this level (declared with game.level@1): it appears when the level loads and goes when another loads. Empty spawns at the start and stays."),
        ],
    )
}

fn health() -> Option<OperatorSpec> {
    let num = |min: f64, max: f64| ParamType::Num { unit: Unit::None, min, max };
    game_node(
        "game.health@1",
        "Give a kind hit points. Its entities get the props health and max_health (bind a bar to them with game.ui@1: value=\"hero.health\", max_var=\"hero.max_health\"). Actions on any rule: hurt:SELECTOR:AMOUNT (hurt:b:10, hurt:tag:enemy:5@kind:hero names the source), heal:SELECTOR:AMOUNT. A hurt raises the event hurt (a = the victim, b = the source), reaching zero raises killed and then removes the entity. iframes makes it untouchable for that many seconds after a hurt; knock pushes it away from the source. Example: `hp = game.health@1(kind=\"hero\", hp=100, iframes=0.6, knock=4)` with `die = game.rule@1(on=\"killed\", a=\"tag:hero\", do=\"lose\")`.",
        vec![
            param("kind", TEXT, None, "The kind that has health."),
            param("hp", num(1.0, 1.0e6), None, "Starting and maximum hit points."),
            param("iframes", num(0.0, 60.0), Some(Value::Num(0.0)), "Seconds untouchable after being hurt."),
            param("knock", num(0.0, 100.0), Some(Value::Num(0.0)), "Metres per second it is pushed away from the source of a hurt."),
            param("height", num(-1.0, 100.0), Some(Value::Num(-1.0)), "Height of the shape shots hit, in metres (-1 uses twice the kind's size)."),
        ],
    )
}

fn projectile() -> Option<OperatorSpec> {
    let num = |min: f64, max: f64| ParamType::Num { unit: Unit::None, min, max };
    game_node(
        "game.projectile@1",
        "Make a kind a shot: its entities fly, hurt the first victim (an entity with health) they meet, are used up, and stop at walls (kinds with solid=...) or when their life ends (raising impact). pierce lets it pass through that many extra victims; splash hurts neighbours within that radius (less with distance); homing turns it toward the nearest victim by that many degrees a second; gravity bends the flight. A weapon (game.weapon@1) spawns it. Example: `arrow = game.projectile@1(kind=\"arrow\", speed=30, life=3, damage=20, gravity=9.8)`.",
        vec![
            param("kind", TEXT, None, "The kind of the shot (declare it with game.kind@1; give it a part to draw it)."),
            param("speed", num(0.1, 1000.0), Some(Value::Num(20.0)), "Metres per second."),
            param("life", num(0.05, 600.0), Some(Value::Num(2.0)), "Seconds before it expires."),
            param("damage", num(0.0, 1.0e6), Some(Value::Num(10.0)), "Damage on a direct hit."),
            param("gravity", num(-100.0, 100.0), Some(Value::Num(0.0)), "Downward acceleration in metres per second squared."),
            param("pierce", ParamType::Int { min: 0, max: 100 }, Some(Value::Int(0)), "Extra victims it passes through."),
            param("splash", num(0.0, 1000.0), Some(Value::Num(0.0)), "Blast radius in metres on impact (0 for none)."),
            param("homing", num(0.0, 3600.0), Some(Value::Num(0.0)), "Degrees a second it may turn toward the nearest victim (0 for none)."),
        ],
    )
}

fn weapon() -> Option<OperatorSpec> {
    let num = |min: f64, max: f64| ParamType::Num { unit: Unit::None, min, max };
    game_node(
        "game.weapon@1",
        "A weapon carried by the entities matching owner. It fires when its input (game.input@1 with key=\"press\") is pressed, held when auto is on, or when a rule runs fire:NAME (an enemy: `game.rule@1(on=\"sight\", a=\"kind:turret\", do=\"fire:spit@a\")`), and keeps a magazine in the owner's prop ammo_NAME. mode projectile spawns the shot kind (game.projectile@1); hitscan hurts the first victim along a ray of range metres at once. aim: heading (the way the owner faces), camera (through the crosshair of the game camera) or a selector such as tag:hero (toward the nearest). Events: fired, empty (dry trigger), reloaded, hit, impact. Example: `rifle = game.weapon@1(name=\"rifle\", mode=\"projectile\", shot=\"bullet\", input=\"fire\", reload_input=\"reload\", owner=\"tag:hero\", ammo=12, reload=1.4, cooldown=0.12, spread=1.5, aim=\"camera\", auto=true)`.",
        vec![
            param("name", TEXT, None, "Unique name; fire:NAME and the prop ammo_NAME use it."),
            param("mode", ParamType::Choice { options: &["projectile", "hitscan", "melee"] }, Some(Value::Str("projectile".to_owned())), "A flying shot, an instant ray, or a melee swing that hurts everything within range inside the arc."),
            param("foes", TEXT, Some(Value::Str(String::new())), "Who it can hurt, as a selector such as tag:enemy (empty: any entity with health but the owner). Name the enemy for a party's weapons so allies never hurt each other."),
            param("arc", ParamType::Num { unit: Unit::None, min: 1.0, max: 360.0 }, Some(Value::Num(120.0)), "Melee: the width of the swing in degrees, centred on the aim."),
            param("owner", TEXT, None, "Who carries it: a selector such as tag:hero or kind:turret."),
            param("input", TEXT, Some(Value::Str("none".to_owned())), "The key input that fires it (none: only rules fire it)."),
            param("reload_input", TEXT, Some(Value::Str(String::new())), "The key input that reloads it."),
            param("shot", TEXT, Some(Value::Str(String::new())), "Projectile mode: the kind of the shot."),
            param("ammo", ParamType::Int { min: 0, max: 10000 }, Some(Value::Int(0)), "Rounds per magazine (0 is endless)."),
            param("reload", num(0.0, 60.0), Some(Value::Num(1.0)), "Seconds to reload."),
            param("cooldown", num(0.0, 60.0), Some(Value::Num(0.2)), "Seconds between shots."),
            param("spread", num(0.0, 90.0), Some(Value::Num(0.0)), "Scatter in degrees either side of the aim."),
            param("aim", TEXT, Some(Value::Str("heading".to_owned())), "heading, camera, or a selector to aim at."),
            param("muzzle", num(0.0, 10.0), Some(Value::Num(1.0)), "Height of the muzzle above the owner's feet, in metres."),
            param("range", num(0.0, 10000.0), Some(Value::Num(0.0)), "Hitscan: how far the ray reaches, in metres."),
            param("damage", num(0.0, 1.0e6), Some(Value::Num(10.0)), "Hitscan: damage of a hit."),
            param("auto", ParamType::Bool, Some(Value::Bool(false)), "Keep firing while the input is held."),
        ],
    )
}

fn brain() -> Option<OperatorSpec> {
    let num = |min: f64, max: f64| ParamType::Num { unit: Unit::None, min, max };
    game_node(
        "game.brain@1",
        "Make a kind move and fight on its own: companions and enemies. With a foe (an entity matching foes that has health) within engage metres it closes to its weapon's reach (keep metres for a ranged weapon) and uses the weapon (game.weapon@1, aim at the foes); with none it follows its leader to within follow metres. Walls (solid kinds) stop it and it slides along them. Give the kind health (game.health@1) so it can be hurt and a weapon with foes set so allies never hurt each other. Example: `pal = game.brain@1(kind=\"companion\", foes=\"tag:enemy\", leader=\"tag:hero\", speed=4, follow=3, weapon=\"sword\")`.",
        vec![
            param("kind", TEXT, None, "The kind it drives."),
            param("foes", TEXT, None, "Who it fights: a selector such as tag:enemy."),
            param("leader", TEXT, Some(Value::Str(String::new())), "Whom it follows when it has nothing to fight, such as tag:hero (empty: it stays put)."),
            param("speed", num(0.1, 100.0), Some(Value::Num(3.0)), "Metres per second."),
            param("follow", num(0.0, 100.0), Some(Value::Num(3.0)), "How close it stays to its leader."),
            param("engage", num(0.0, 1000.0), Some(Value::Num(15.0)), "How far away it notices a foe."),
            param("weapon", TEXT, Some(Value::Str(String::new())), "The weapon it fights with."),
            param("keep", num(-1.0, 1000.0), Some(Value::Num(-1.0)), "How far from a foe a ranged fighter stands (-1 uses 6)."),
        ],
    )
}

fn targeting() -> Option<OperatorSpec> {
    let num = |min: f64, max: f64| ParamType::Num { unit: Unit::None, min, max };
    game_node(
        "game.targeting@1",
        "Choose the leader's target. The leader's prop target is the id of the foe it faces (the nearest inside the cone, else the nearest of all), kept until that foe dies or leaves range; the foe's prop targeted is 1, so the UI can show it (text=\"{targeted.health}\", a bar with value=\"targeted.health\"). The action target:next or target:prev cycles through the foes by distance. Abilities with needs_target use it. Example: `lock = game.targeting@1(leader=\"tag:hero\", foes=\"tag:enemy\", range=25)`.",
        vec![
            param("leader", TEXT, None, "Whose target it is: a selector such as tag:hero."),
            param("foes", TEXT, None, "What can be targeted: a selector such as tag:enemy."),
            param("range", num(0.0, 1000.0), Some(Value::Num(25.0)), "How far away a foe can be targeted."),
            param("cone", num(0.0, 360.0), Some(Value::Num(80.0)), "Foes within this many degrees of where the leader faces (or the camera looks) are preferred."),
        ],
    )
}

fn ability() -> Option<OperatorSpec> {
    let num = |min: f64, max: f64| ParamType::Num { unit: Unit::None, min, max };
    game_node(
        "game.ability@1",
        "A button-triggered action with a cost and a cooldown, for the owner's ability bar. Pressing its input (game.input@1 with key=\"press\") runs the do actions with a = the owner and b = its target (game.targeting@1), pays cost from the owner's prop cost_prop, and starts the cooldown; the owner's props cd_NAME (seconds left) and cdmax_NAME drive a slot on screen (game.ui@1 type=\"slot\"). A press that cannot work (cooling down, too little of the cost prop, no target in range when needs_target is on) raises refused:NAME instead; a use raises ability:NAME. Actions to use: hurt:b:30, heal:tag:party:40, blast:a:5:30@tag:enemy (hurt every foe within 5 m of the owner), dash:a:12:0.25, iframes:a:0.4, effect:particles:explode@b. Example: `smash = game.ability@1(name=\"smash\", owner=\"tag:hero\", input=\"power1\", cooldown=8, cost=20, range=6, needs_target=true, do=\"hurt:b:60;effect:camera_shake:hit@a\")`.",
        vec![
            param("name", TEXT, None, "Unique name."),
            param("owner", TEXT, None, "Who has it: a selector such as tag:hero."),
            param("input", TEXT, None, "The key input that triggers it."),
            param("cooldown", num(0.0, 3600.0), Some(Value::Num(1.0)), "Seconds before it can be used again."),
            param("cost", num(0.0, 1.0e6), Some(Value::Num(0.0)), "Taken from the owner's prop cost_prop when used."),
            param("cost_prop", TEXT, Some(Value::Str("focus".to_owned())), "The owner's prop that pays the cost."),
            param("range", num(0.0, 1000.0), Some(Value::Num(0.0)), "The farthest target it works on in metres (0 for any)."),
            param("needs_target", ParamType::Bool, Some(Value::Bool(false)), "Whether it needs a target in range."),
            param("do", TEXT, Some(Value::Str(String::new())), "Semicolon-separated actions, as in game.rule@1."),
        ],
    )
}

fn ui() -> Option<OperatorSpec> {
    let text = |name: &'static str, help: &'static str| param(name, TEXT, Some(Value::Str(String::new())), help);
    let amount = |name: &'static str, help: &'static str| param(name, ParamType::Num { unit: Unit::None, min: -1.0, max: 1000.0 }, Some(Value::Num(-1.0)), help);
    game_node(
        "game.ui@1",
        "One element of the game's screen UI (title, pause menu, HUD, prompts, minimap, dialogue choices). Drawn over the 3D view in fractions of the screen height (x runs from 0 to the aspect ratio) with a procedural font, so it needs no assets. Types: text (words, with {score} for a variable and {key:jump} for the key the player uses), bar (value and max are a variable or a number), panel, button (raises the event ui:NAME and runs `do` when accepted: Enter, A, a click), stack (a column or row that places its children: parent=NAME), grid, minimap (dots for entities with the given tags around the followed one). show=\"title,paused\" limits it to flow states (game.state@1), so a menu appears only where it belongs; arrows/Tab/d-pad move the focus between buttons. Example: `menu = game.ui@1(name=\"menu\", type=\"stack\", anchor=\"c\", fill=\"rgba(0,0,0,0.6)\", show=\"title\")` then `play = game.ui@1(name=\"play\", type=\"button\", parent=\"menu\", text=\"Play\", do=\"go:playing\")`. Colors are written rgba(r,g,b,a) or quoted \"#rrggbb\".",
        vec![
            param("name", TEXT, None, "Unique id; a button raises ui:NAME; parent= refers to it."),
            param("type", ParamType::Choice { options: &["text", "bar", "panel", "button", "stack", "grid", "minimap", "slot", "portrait", "floaters"] }, None, "What the element is. slot: an ability square with an icon, a key label and a cooldown overlay (value=\"hero.cd_smash\", max=\"hero.cdmax_smash\" from game.ability@1). portrait: a round badge with an initial (text=\"V\", fill=colour). floaters: draws the damage numbers, +healing and Dodge! that float up from where things were hurt (one per game, no size)."),
            text("when", "Show the element only while this expression is true, for example targeted.health>0 (names are variables or KIND.prop, and targeted.PROP is the leader's current target): a target frame that appears only while something is targeted."),
            text("icon", "A slot's picture, drawn from strokes: sword, shield, bolt, heal, dash, star, flame or skull."),
            text("key", "A slot's key label, usually text=\"{key:ACTION}\" so it follows the player's device."),
            text("parent", "A stack or grid declared earlier that places this element; empty places it by anchor."),
            text("show", "Flow states (comma separated) in which it is visible; empty means always."),
            text("hide", "Flow states in which it is hidden."),
            text("text", "Words for text, button and the like; {name} inserts a variable, {key:ACTION} the key or button the player uses for an input."),
            text("do", "Actions a button runs when accepted, as in game.rule@1: go:playing, restart, level:cave, add:score:1 ..."),
            text("tip", "A button's tooltip, shown under it while it has the focus."),
            text("anchor", "Where on the screen: tl, tc, tr, cl, c, cr, bl, bc, br (default tl)."),
            text("at", "Offset from the anchor in screen heights, as x,y (for example 0.02,0.03)."),
            text("size", "Width,height in screen heights for bar, panel, button and minimap, written as text such as \"0.3,0.03\"."),
            amount("letter", "The letter height of a text or a button's words, in screen heights (-1 uses 0.03; 0.06 is a heading)."),
            text("value", "A bar's value: the name of a variable."),
            amount("max", "A bar's full value as a number (-1 uses 1)."),
            text("max_var", "A bar's full value as the name of a variable (used instead of max)."),
            text("label", "Words drawn on a bar."),
            text("dir", "A stack's direction: down or right."),
            amount("gap", "Space between a container's children (-1 uses the default)."),
            amount("pad", "Space inside a container's edge (-1 uses the default)."),
            text("align", "A stack's alignment across its direction: start, center or end."),
            amount("cols", "A grid's columns (-1 uses 3)."),
            text("cell", "A grid's cell width,height."),
            amount("radius", "A minimap's reach in metres from the centre to the edge; for other elements the corner rounding."),
            text("follow", "A minimap's centre: a selector such as tag:hero."),
            text("tags", "A minimap shows entities of kinds with these tags (comma separated)."),
            text("kinds", "A minimap shows entities of these kinds (comma separated)."),
            text("fill", "Background colour of a panel, stack, grid or button."),
            text("border", "Border colour."),
            text("color", "Text or bar colour."),
            text("font", "Font preset: sans, serif, mono, hand ..."),
            text("plate", "true puts a dark plate behind a text."),
        ],
    )
}

fn state() -> Option<OperatorSpec> {
    game_node(
        "game.state@1",
        "Declare a state of the game's flow, such as title, playing, paused, game_over or win. The first one declared is where the game starts. A frozen state stops the simulation (nothing moves, no timers, no rules on tick); only inputs declared with game.input key=\"press\" are heard, so a press can leave it. Entering a state raises the event state:NAME. Winning or losing enters the states named win and game_over when they exist. Example: `title = game.state@1(name=\"title\", frozen=true)` with `go = game.rule@1(on=\"input:start\", state=\"title\", do=\"go:playing\")`.",
        vec![
            param("name", TEXT, None, "Unique name; rules use it in go:NAME, state=NAME and on=state:NAME."),
            param("frozen", ParamType::Bool, Some(Value::Bool(false)), "Whether the simulation stands still in this state."),
        ],
    )
}

fn level() -> Option<OperatorSpec> {
    game_node(
        "game.level@1",
        "Declare a level: spawns with level=NAME belong to it. The first one declared loads at the start; the action level:NAME replaces the loaded level's entities with the named level's and puts every hero (tag:hero) at its start. Loading raises the event level:NAME. Example: `two = game.level@1(name=\"cave\", start=[0, 0, 0])`.",
        vec![
            param("name", TEXT, None, "Unique name."),
            param("start", ParamType::Vec3 { unit: Unit::Length, min: -1.0e6, max: 1.0e6 }, Some(Value::List(vec![Value::Num(0.0); 3])), "Where heroes are put when the level loads, in metres (used when place_heroes is on)."),
            param("place_heroes", ParamType::Bool, Some(Value::Bool(false)), "Whether loading the level puts every hero at start."),
        ],
    )
}

fn input() -> Option<OperatorSpec> {
    game_node(
        "game.input@1",
        "Declare a player axis or press input. Example: `move = game.input@1(name=\"move\", axis=\"x,z\", target=\"tag:hero\")`.",
        vec![
            param("name", TEXT, None, "Input name."),
            param(
                "axis",
                TEXT,
                Some(Value::Str(String::new())),
                "Comma-separated axes, such as x,z.",
            ),
            param(
                "key",
                ParamType::Choice {
                    options: &["", "press"],
                },
                Some(Value::Str(String::new())),
                "Button kind, currently press.",
            ),
            param("target", TEXT, None, "Target selector, such as tag:hero."),
            param(
                "follow",
                ParamType::Choice {
                    options: &["", "viewer", "left_hand", "right_hand"],
                },
                Some(Value::Str(String::new())),
                "Move matching entities to the viewer position (viewer) or to a tracked controller (left_hand, right_hand) each step. A hand that is not tracked leaves its entity where it was.",
            ),
            param(
                "keys",
                TEXT,
                Some(Value::Str(String::new())),
                "Keyboard keys that drive it, as KeyboardEvent codes separated by commas (Space, KeyC, ArrowUp, Mouse0). An axis needs a positive and a negative key per axis, in order: axis=\"x,z\" keys=\"KeyD,KeyA,KeyW,KeyS\". Empty uses the page's defaults. The player can rebind them.",
            ),
            param(
                "pad",
                TEXT,
                Some(Value::Str(String::new())),
                "Gamepad controls: button<N> for a press (button0 is A), stick:left or stick:right for a two-axis input, axis<N>+ and axis<N>- for one axis. Empty uses the page's defaults.",
            ),
            param("label", TEXT, Some(Value::Str(String::new())), "What a prompt and the controls screen call this action (default: the name)."),
        ],
    )
}

fn rule() -> Option<OperatorSpec> {
    game_node(
        "game.rule@1",
        "Declare an event rule and semicolon-separated actions. Example: `pick = game.rule@1(on=\"touch\", a=\"tag:hero\", b=\"tag:coin\", do=\"add:score:1;destroy:b\")`.",
        vec![
            param(
                "on",
                TEXT,
                None,
                "Event name, such as touch, tick or var:score.",
            ),
            param(
                "a",
                TEXT,
                Some(Value::Str(String::new())),
                "Optional first entity selector.",
            ),
            param(
                "b",
                TEXT,
                Some(Value::Str(String::new())),
                "Optional second entity selector.",
            ),
            param(
                "when",
                TEXT,
                Some(Value::Str(String::new())),
                "Optional condition expression.",
            ),
            param(
                "every",
                ParamType::Num {
                    unit: Unit::None,
                    min: 0.0,
                    max: 1.0e6,
                },
                Some(Value::Num(0.0)),
                "Optional tick interval in seconds.",
            ),
            param(
                "state",
                TEXT,
                Some(Value::Str(String::new())),
                "Limit the rule to one flow state (declared with game.state@1), such as playing or paused. Every rule on an event sees the state the event began in, so one pause key can have a rule in playing and another in paused.",
            ),
            param("do", TEXT, None, "Semicolon-separated action list. Besides add, set, destroy, spawn, timer, emit, effect, win and lose, flow actions: go:STATE (enter a state), restart or restart:STATE (rebuild the game from its start, then enter the state), level:NAME (load a level), checkpoint and respawn (store the hero's place and return to it; an optional :SELECTOR picks other entities)."),
        ],
    )
}

fn script() -> Option<OperatorSpec> {
    game_node(
        "game.script@1",
        "A JavaScript behaviour that runs inside the game's fixed step, for anything the typed game nodes do not say. The source runs once when the game starts and registers handlers: `on(event, fn)`, `on(event, {a: \"tag:hero\", b: \"tag:coin\"}, fn)`, `every(seconds, fn)`, `after(seconds, fn)`. The handler gets `{name, a, b, tick, dt}`, with `a` and `b` the event entities or null. World: `find(\"kind:x\" | \"tag:x\")`, `entity(id)`, `spawn(kind, [x, y, z], heading)`; entities have `id`, `kind`, `tags`, `pos`, `vel`, `heading`, `get/set/add(prop)`, `has(tag)`, `destroy()`. Game: `game.get/set/add(variable)`, `game.emit(name, a, b)`, `game.effect(kind, name, target, value)`, `game.win/lose`, `game.tick/time`. Keep saveable data in global `state` (plain JSON); `Math.random` uses the seeded stream; `log(...)` writes to the script log. `inventory` must be loaded first with `inv = game.script@1(lib=\"inventory\")`; it provides `define`, `create`, `add`, `remove`, `count`, `has`, `weight`, and `list` and saves state under `state.inventory`. Example after declaring the library: `pick = game.script@1(name=\"pickups\", source=\"on('start', () => inventory.define('herb', {max_stack: 8, weight: 0.2})); on('touch', {a: 'tag:hero', b: 'tag:herb'}, (e) => { inventory.create(e.a, {slots: 12, max_weight: 20}); const result = inventory.add(e.a, 'herb', 1); if (result.added) e.b.destroy(); });\")`.",
        vec![
            param(
                "name",
                TEXT,
                Some(Value::Str(String::new())),
                "Unique lowercase name (letters, digits, underscores); empty uses the node label or id.",
            ),
            param(
                "source",
                ParamType::Str { max_len: 65536 },
                Some(Value::Str(String::new())),
                "JavaScript source, up to 64 KB (not needed with lib).",
            ),
            param(
                "lib",
                ParamType::Choice {
                    options: &["", "rpg", "combat", "inventory"],
                },
                Some(Value::Str(String::new())),
                "Load a standard library instead of source; declare it before the scripts that use it. rpg: attributes with modifiers, tags, effects (duration, period, stacking), abilities (cost, cooldown, requirements) and a damage pipeline (resistances, armour, block, poise and stagger, death) in global `rpg`. combat (needs rpg first): timed melee attacks, combos, hitstop, dodge, block and parry. inventory: deterministic item definitions and capacity-limited stacks in global `inventory`; add, remove, count and query items. RPG, combat and inventory state is saved in the game snapshot; see each library header for calls.",
            ),
        ],
    )
}

fn goal() -> Option<OperatorSpec> {
    game_node(
        "game.goal@1",
        "Declare a terminal condition. Example: `won = game.goal@1(outcome=\"win\", when=\"score>=8\")`.",
        vec![
            param(
                "outcome",
                ParamType::Choice {
                    options: &["win", "lose"],
                },
                None,
                "Whether this condition wins or loses.",
            ),
            param("when", TEXT, None, "Condition expression."),
        ],
    )
}

fn hud() -> Option<OperatorSpec> {
    game_node(
        "game.hud@1",
        "Show text bound to game variables as a procedural pixel HUD. Example: `panel = game.hud@1(text=\"Score {score}\", anchor=\"gaze\", size=0.005)`. Anchors: gaze, left, right, top, bottom.",
        vec![
            param(
                "text",
                TEXT,
                None,
                "HUD text; `{name}` inserts a game variable.",
            ),
            param(
                "anchor",
                ParamType::Choice {
                    options: &["gaze", "left", "right", "top", "bottom"],
                },
                Some(Value::Str("gaze".to_owned())),
                "HUD position anchor.",
            ),
            param(
                "size",
                ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 1.0,
                },
                Some(Value::Num(0.005)),
                "Pixel edge length in metres (a letter is 7 pixels tall: 0.005 gives 3.5 cm letters, a 20-character line about 0.6 m wide, readable at arm's length).",
            ),
        ],
    )
}

fn collect() -> Option<OperatorSpec> {
    spec(
        "game.collect@1",
        "Join game fragments in link order. Example: `all = game.collect@1(score, hero, spawn, rule, goal, name=\"coins\")`.",
        vec![PortSpec {
            name: "in",
            accepts: GAME,
            arity: Arity::Variadic { min: 1 },
        }],
        OutputSpec::Fixed(PortType::Game),
        vec![param(
            "name",
            TEXT,
            Some(Value::Str("game".to_owned())),
            "Definition name; the last collect name wins.",
        )],
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        var(),
        kind::kind(),
        spawn(),
        input(),
        rule(),
        script(),
        goal(),
        hud(),
        collect(),
        agent(),
        controller(),
        camera(),
        state(),
        level(),
        ui(),
        health(),
        projectile(),
        weapon(),
        brain(),
        targeting(),
        ability(),
        navigation::nav(),
        game_node(
            "game.ground@1",
            "Set the ground plane height. Example: `ground = game.ground@1(y=0.25)`.",
            vec![param(
                "y",
                NUMBER,
                Some(Value::Num(0.0)),
                "Ground height in metres.",
            )],
        ),
        game_node(
            "game.terrain@1",
            "Declare ground sampling for rolling things on a field. Example: `terrain = game.terrain@1(parts=[\"field\", \"path\"], cell=0.25)`; parts are the output names of authored parts.",
            vec![
                param(
                    "parts",
                    ParamType::StrList {
                        max_items: 256,
                        max_len: 128,
                    },
                    None,
                    "Output names of authored parts covered by this terrain.",
                ),
                param(
                    "cell",
                    ParamType::Num {
                        unit: Unit::Length,
                        min: 0.0,
                        max: 1.0e6,
                    },
                    Some(Value::Num(0.25)),
                    "Terrain sampling cell size in metres, greater than zero.",
                ),
            ],
        ),
        game_node(
            "game.surface@1",
            "Declare a surface for a rolling ball on a table. Example: `felt = game.surface@1(name=\"felt\", parts=[\"table\"], decel=0.5, bounce=0.5)`; parts are the output names of authored parts.",
            vec![
                param("name", TEXT, None, "Surface name."),
                param(
                    "parts",
                    ParamType::StrList {
                        max_items: 256,
                        max_len: 128,
                    },
                    None,
                    "Non-empty output names of authored parts using this surface.",
                ),
                param(
                    "decel",
                    ParamType::Num {
                        unit: Unit::None,
                        min: 0.0,
                        max: 1.0e9,
                    },
                    None,
                    "Positive roll deceleration.",
                ),
                param(
                    "bounce",
                    ParamType::Num {
                        unit: Unit::None,
                        min: 0.0,
                        max: 1.0,
                    },
                    Some(Value::Num(0.5)),
                    "Vertical bounce fraction.",
                ),
            ],
        ),
    ]
}

fn camera() -> Option<OperatorSpec> {
    let amount = |min: f64, max: f64| ParamType::Num { unit: Unit::None, min, max };
    game_node(
        "game.camera@1",
        "Say how the view follows the player in play mode. The first camera starts active; a rule switches with the action `effect:camera:NAME` (cut) or `effect:camera:NAME:SECONDS` (blend). Modes: `first` (eye at the target, the player's look), `third` (orbit behind the target with the mouse or stick, an optional shoulder offset, and a spring arm that shortens against walls), `top_down` (looks down from above), `side` (a side-scroller: looks along the depth axis and follows the target outside a dead zone), `iso` (a fixed isometric angle), `fixed` (stands at `at` and watches the target, like a security camera). Number parameters left at their automatic value use the mode's own default (third: distance 4.5, height 1.5, smoothing 0.08; top_down: distance 14; side: distance 10; iso: distance 16, yaw 45, pitch -35). `smoothing` is the seconds in which the view closes most of a gap to the target; `bounds` keeps the view inside the level. Pair it with `game.controller@1(relative=\"camera\")` so the hero walks where the player looks. Example: `cam = game.camera@1(name=\"main\", mode=\"third\", shoulder=0.5)`.",
        vec![
            param("name", TEXT, None, "The camera's name, used by the switch action."),
            param(
                "mode",
                ParamType::Choice { options: &["first", "third", "top_down", "side", "iso", "fixed"] },
                Some(Value::Str("third".to_owned())),
                "How the view follows the target.",
            ),
            param("target", TEXT, Some(Value::Str("tag:hero".to_owned())), "Selector of the entity it follows, such as tag:hero or kind:player."),
            param("distance", amount(0.0, 1000.0), Some(Value::Num(0.0)), "Arm length (third), height above the target (top_down), or distance back from the target's plane (side, iso), in metres; 0 uses the mode's default."),
            param("height", amount(-1.0, 100.0), Some(Value::Num(-1.0)), "Height of the pivot above the target's feet (the eye height in first person); -1 uses the mode's default."),
            param("shoulder", amount(-10.0, 10.0), Some(Value::Num(0.0)), "Third person: sideways offset of the camera to the right of the target, in metres (negative is left)."),
            param("pitch", amount(-1000.0, 1000.0), Some(Value::Num(999.0)), "Fixed-angle modes: the pitch in degrees (negative looks down); 999 uses the mode's default."),
            param("yaw", amount(-1000.0, 1000.0), Some(Value::Num(999.0)), "Fixed-angle modes: the yaw in degrees (0 looks along +z, 90 along +x); 999 uses the mode's default."),
            param("fov", amount(10.0, 150.0), Some(Value::Num(60.0)), "Vertical field of view in degrees."),
            param("smoothing", amount(-1.0, 60.0), Some(Value::Num(-1.0)), "Seconds in which the view closes most of a gap to the target; 0 follows rigidly, -1 uses the mode's default."),
            param("collide", ParamType::Bool, Some(Value::Bool(true)), "Third person: whether the arm shortens against walls instead of passing through them."),
            param("deadzone", amount(0.0, 100.0), Some(Value::Num(0.0)), "Half-size in metres of the area around the view in which the target can move without moving the view (a side-scroller's loose follow)."),
            param("at", TEXT, Some(Value::Str(String::new())), "Fixed camera only: where it stands, as x,y,z in metres."),
            param("bounds", TEXT, Some(Value::Str(String::new())), "Keeps the view's pivot inside the level: x_min,x_max,z_min,z_max in metres."),
            param("min_pitch", amount(-89.0, 89.0), Some(Value::Num(-60.0)), "Third person: lowest look angle in degrees."),
            param("max_pitch", amount(-89.0, 89.0), Some(Value::Num(75.0)), "Third person: highest look angle in degrees."),
        ],
    )
}

fn controller() -> Option<OperatorSpec> {
    let amount = |min: f64, max: f64| ParamType::Num { unit: Unit::None, min, max };
    game_node(
        "game.controller@1",
        "Move a kind from the player's input like a character: it speeds up and slows down, turns, falls under gravity, jumps (variable height, extra jumps, a short grace after leaving a ledge, a jump pressed just before landing still counts), lands on the ground and on the tops of solid boxes, and can move relative to the camera. The entity's y is its feet. Read the movement from the input named by `move` (axes `NAME.x` and `NAME.z`, so `game.input@1(name=\"move\", axis=\"x,z\", target=\"tag:hero\")`) and the jump from the input named `jump_input` (`game.input@1(name=\"jump\", key=\"press\")`). A rule or script can read the entity's props `grounded` (1 or 0) and `speed`. Modes are presets: `walk` (3D ground), `platformer` (confined to a plane: x only by default), `top_down` (no gravity, no jump). Example: `hero_ctl = game.controller@1(kind=\"player\", speed=6, jump=8, extra_jumps=1, relative=\"camera\")`.",
        vec![
            param("kind", TEXT, None, "Declared kind to move; every entity of it is controlled."),
            param(
                "mode",
                ParamType::Choice { options: &["walk", "platformer", "top_down"] },
                Some(Value::Str("walk".to_owned())),
                "walk: 3D ground movement. platformer: movement along one axis in a plane (lock defaults to z). top_down: no gravity and no jump.",
            ),
            param("speed", amount(0.0, 1000.0), Some(Value::Num(0.0)), "Fastest horizontal speed in metres per second; 0 uses the kind's speed (4 if it has none)."),
            param("accel", amount(0.0, 100_000.0), Some(Value::Num(0.0)), "How fast speed rises toward the input, metres per second squared; 0 means ten times the speed (full speed in a tenth of a second)."),
            param("decel", amount(0.0, 100_000.0), Some(Value::Num(0.0)), "How fast speed falls to rest when the input stops; 0 means the same as accel. Small values slide like ice."),
            param("air", amount(0.0, 1.0), Some(Value::Num(0.6)), "Share of accel and decel that works in the air: 1 steers as on the ground, 0 keeps the speed it left the ground with."),
            param("jump", amount(0.0, 1000.0), Some(Value::Num(0.0)), "Upward speed of a jump in metres per second (0 means no jumping); the height is jump squared over twice gravity: 8 with gravity 20 is 1.6 m."),
            param("gravity", amount(0.0, 1000.0), Some(Value::Num(20.0)), "Downward acceleration in metres per second squared (0 floats)."),
            param("coyote", amount(0.0, 2.0), Some(Value::Num(0.1)), "Seconds after walking off a ledge during which a jump still works."),
            param("buffer", amount(0.0, 2.0), Some(Value::Num(0.1)), "Seconds a jump pressed too early is remembered, to go off on landing."),
            param("extra_jumps", ParamType::Int { min: 0, max: 5 }, Some(Value::Int(0)), "Jumps allowed in the air after leaving the ground (1 is a double jump)."),
            param("cut", amount(0.0, 1.0), Some(Value::Num(0.5)), "Share of the upward speed kept when the jump button is released early: 1 always jumps full height, 0.3 makes taps short hops."),
            param("turn", amount(0.0, 100_000.0), Some(Value::Num(0.0)), "Turn rate toward the direction of motion in degrees per second; 0 faces the motion at once."),
            param("step", amount(0.0, 10.0), Some(Value::Num(0.3)), "Highest ledge in metres it walks up onto without jumping; taller boxes block the way."),
            param("fall_limit", amount(-10_000.0, 10_000.0), Some(Value::Num(-10_000.0)), "A height in metres below which the hero is put back on the last safe ground it stood on and the event `fell` is raised (a rule can take a life or restart: game.rule@1(on=\"fell\", do=\"add:lives:-1\")). -10000 means off; set it below the lowest platform, with game.ground@1(y=...) lower still."),
            param(
                "relative",
                ParamType::Choice { options: &["world", "camera"] },
                Some(Value::Str("world".to_owned())),
                "world: the movement axes are world x and z. camera: forward is where the camera looks (the host supplies `camera.yaw`), so a third-person or first-person hero walks where the player looks.",
            ),
            param(
                "lock",
                ParamType::Choice { options: &["none", "x", "z"] },
                Some(Value::Str("none".to_owned())),
                "A horizontal axis it may not move along (a side-scroller locks z); none uses the mode's default.",
            ),
            param("move", TEXT, Some(Value::Str("move".to_owned())), "Name of the input whose axes (`NAME.x`, `NAME.z`) move it."),
            param("jump_input", TEXT, Some(Value::Str("jump".to_owned())), "Name of the press input that jumps."),
        ],
    )
}

fn agent() -> Option<OperatorSpec> {
    game_node(
        "game.agent@1",
        "Make a kind patrol, notice and chase the hero. Example: `watch = game.agent@1(kind=\"creature\", patrol=[[0, 0], [6, 0]], speed=1.5)`.",
        vec![
            param("kind", TEXT, None, "Declared kind to control."),
            param(
                "patrol",
                ParamType::Points2 {
                    min: -1.0e6,
                    max: 1.0e6,
                    min_points: 2,
                    max_points: 2048,
                },
                None,
                "Ordered loop of horizontal x,z points in metres.",
            ),
            param(
                "speed",
                NUMBER,
                None,
                "Positive patrol speed in metres per second.",
            ),
            param(
                "chase_speed",
                NUMBER,
                Some(Value::Num(0.0)),
                "Positive chase speed; 0 selects 1.6 times speed.",
            ),
            param(
                "sight",
                NUMBER,
                Some(Value::Num(5.0)),
                "Positive notice distance in metres.",
            ),
            param(
                "lose",
                NUMBER,
                Some(Value::Num(0.0)),
                "Positive chase loss distance; 0 selects 1.5 times sight.",
            ),
        ],
    )
}
