// A test MCP server for the engine: lets any MCP client (Claude Code, for one) build and edit a
// document through the page that is running the engine. No model, no API key: the client's model is
// the provider. It speaks MCP over stdio and relays each tool call to the editor page over a local
// WebSocket; the page runs it through the normal commands (text in, typed commands underneath).
//
//   claude mcp add spatial-engine -- bun engine/tools/mcp/server.mjs
//   open http://127.0.0.1:5173/?mcp=1     (the page connects to ws://127.0.0.1:8765)
//
// For local testing only: it binds to 127.0.0.1, takes one page at a time and has no authentication.
// stdout carries only MCP messages; everything else goes to stderr.

const PORT = Number(process.env.VE_MCP_PORT ?? 8765);
const TIMEOUT_MS = 90000;
let page = null;
let counter = 0;
const pending = new Map();

const log = (...args) => console.error('[spatial-engine mcp]', ...args);

Bun.serve({
  hostname: '127.0.0.1',
  port: PORT,
  fetch(request, server) {
    return server.upgrade(request) ? undefined : new Response('spatial-engine MCP bridge', { status: 426 });
  },
  websocket: {
    open(ws) { page?.close(); page = ws; log('editor page connected'); },
    close(ws) { if (page === ws) { page = null; log('editor page disconnected'); } },
    message(_, raw) {
      let message;
      try { message = JSON.parse(String(raw)); } catch { return; }
      const waiting = pending.get(message.id);
      if (!waiting) return;
      pending.delete(message.id);
      clearTimeout(waiting.timer);
      waiting.resolve(message);
    },
  },
});
log(`bridge listening on ws://127.0.0.1:${PORT}`);

function callPage(method, params) {
  if (!page) {
    return Promise.resolve({ error: `No editor page is connected. Open the engine editor with ?mcp=1 (for example http://127.0.0.1:5173/?mcp=1) and try again.` });
  }
  const id = ++counter;
  return new Promise((resolve) => {
    const timer = setTimeout(() => { pending.delete(id); resolve({ error: `The page did not answer ${method} within ${TIMEOUT_MS / 1000} s.` }); }, TIMEOUT_MS);
    pending.set(id, { resolve, timer });
    page.send(JSON.stringify({ id, method, params }));
  });
}

const GUIDE = `The document is a graph of operators, written one node per line:

  name = family.op@1(input_node, ..., key=value, ...)
  output part_name = node_name        (or: output node_name)

Inputs are earlier node names, positional in port order (or port=name). Parameters are key=value.
Lengths are metres (suffixes mm and cm work); angles are degrees unless you write rad (for example
angle=90deg or 1.57rad). Numbers, "text", true/false and [lists] are the only values. Lines starting
with # are comments. Only operators from list_operators can be used. Y is up. Profiles live in the XY
plane: solid.revolve spins one (x >= 0) about Y, solid.extrude stands one up from y=0.

Example (a vase, with a 3 mm minimum wall that the engine enforces):
  outer = profile.polygon@1(points=[[0,0],[0.06,0],[0.11,0.12],[0.07,0.26],[0.05,0.3],[0,0.3]])
  inner = profile.polygon@1(points=[[0,0.01],[0.056,0.01],[0.104,0.12],[0.064,0.26],[0.04,0.31],[0,0.31]])
  wall = check.min_wall@1(outer, inner, min=3mm)
  body = solid.revolve@1(outer, angle=360deg, segments=64)
  cavity = solid.revolve@1(inner, angle=360deg, segments=64)
  vase = solid.difference@1(body, cavity)
  output vase

Smooth shapes: a few well-chosen profile points are enough if you round them with
profile.smooth@1(profile, iterations=3, keep_axis=true), which turns the corners into a smooth curve
(keep_axis=true for a profile you will revolve, so its base stays flat). For a hollow vessel, revolve
an outer and an inner profile and subtract; let the inner profile rise above the outer rim to open
the top, and add check.min_wall@1 to keep the wall thick enough. Lay out real dimensions in metres.

Furniture and machines: solid.box, solid.cylinder, solid.cone, solid.torus and solid.capsule are
centred on the origin (cone, cylinder and capsule stand along Y); profile.rect and profile.circle
make profiles to extrude. Place a part with xform.move, then repeat it: xform.mirror@1(solid,
axis="x") adds the mirror image across the plane through the origin (put the part off-centre first),
xform.array@1(solid, count_x=3, count_y=1, count_z=2, spacing=[0.2, 0, 0.2]) makes a grid,
xform.radial@1(solid, count=16, radius=0.1) places copies evenly around the Y axis (each moved out
along X first), and xform.repeat@1(solid, count=12, step=[0, 0.18, 0], rotate_step=[0, 15, 0]) makes a
staircase or spiral (copy i is turned i times rotate_step, then moved i times step). Copies are
fused into one solid, so make neighbours overlap a little where they must join. A table:
  leg = solid.box@1(size=[0.05, 0.72, 0.05])
  leg_corner = xform.move@1(leg, by=[0.55, 0.36, 0.30])
  legs_pair = xform.mirror@1(leg_corner, axis="x")
  legs = xform.mirror@1(legs_pair, axis="z")
  top = solid.box@1(size=[1.2, 0.04, 0.7])
  top_raised = xform.move@1(top, by=[0, 0.72, 0])
  table = solid.union@1(legs, top_raised)
  output table

Groups (build once, use many times): a group is an assembly with its own nodes, a result and the
parameters you choose to expose; an instance is one use of it. Write the group first, then instances:
  group wheel
    tyre = solid.torus@1(major=0.3, minor=0.06, segments=48, tube_segments=16)
    hub = solid.cylinder@1(height=0.2, radius=0.08, segments=32)
    spoke = solid.box@1(size=[0.22, 0.03, 0.03])
    spokes = xform.radial@1(spoke, count=4, radius=0.14)
    rim = solid.union@1(tyre, hub, spokes)
    expose radius = tyre.major
    result rim
  end
  front = wheel()
  big = wheel(radius=0.4)
  moved = xform.move@1(big, by=[1, 0, 0])
Rules: the nodes inside a group read only each other; "result" names the member that is the group's
result (it must be a solid); "expose name = member.param" lets an instance override that parameter
with name=value (the member's own limits apply). An instance is written like a node whose operator is
the group's name (no dot, no @version), and it can be moved, combined, given a material or made an
output like any solid. A group may take solids as inputs: inside it write
  mount = group.input@1(name="mount")
and bind it on an instance with mount=post. Changing a node inside a group changes every instance;
equal instances are computed once. Group names are plain words (letters, digits, underscore).

Rigid alignment: xform.align@1(solid, from=[x,y,z], to=[x,y,z], pivot=[0,0,0]). Directions must be
nonzero; their magnitude is ignored. The pivot stays fixed. Opposite directions use a deterministic
half-turn.
Assembly sockets: frame.add@1(solid, name="mount", position=[x,y,z], rotation=[x,y,z]) attaches a
named rigid frame (angles in degrees in text) to a solid. xform.attach@1(source, to=parent,
socket="base", to_socket="mount") matches position and orientation; names travel with evaluated
results through groups and do not change group interface versions. Missing sockets return a named
diagnostic listing available sockets. xform.mirror namespaces reflected sockets as mirror.name.
frame.add and xform.attach accept solids only; realize an instance set before attaching it.
Shared placements (no union): inst.array@1(solid_or_set, count_x=1, count_y=1, count_z=1, spacing=[x,y,z]),
inst.radial@1(solid_or_set, count, radius=0), inst.repeat@1(solid_or_set, count, step=[x,y,z], rotate_step=[x,y,z]),
inst.mirror@1(solid_or_set, axis="x"). Sets share one prototype and allow at most 16384 placements.
Move/rotate/scale also work on sets. A set is a valid output. Use inst.realize@1(set) only when one
boolean solid is needed; triangle and memory limits apply before union. Example fence:
  post = solid.box@1(size=[0.05,1,0.05])
  posts = inst.array@1(post, count_x=20, spacing=[0.2,0,0])
  output fence = posts
For a later boolean operation, write fused = inst.realize@1(posts) and feed fused to the boolean.

Colour and finish: solid.material@1(solid, color=[r,g,b], metallic=0, roughness=0.5, emissive=[0,0,0])
gives a solid a surface (colour is sRGB 0..1 as in a colour picker, metallic 0 plastic/paint/wood to 1
bare metal, roughness 0 mirror to 1 matte, emissive glows above 1). The shape is unchanged. Put it on
the solid you output, for example:
  red = solid.material@1(vase, color=[0.85, 0.08, 0.08], roughness=0.3)
  output vase = red
Transforms and booleans keep the first input's surface.

How to work: call get_text to see the current document. apply_text ADDS the nodes of a program to
the document (it never replaces existing ones) and may read nodes already there by their label (see
get_nodes), as the example above reads vase; a new node cannot reuse an existing label, and
"output name = node" points an existing output at a different node. To change something that
already exists use apply_commands, which addresses nodes by ID (get_nodes lists each node's id, label, operator,
parameters and inputs; set_param is the usual command). Read the result after each call. A refused edit leaves the
document unchanged and says why (for example constraint.min_wall with the measured thickness).`;

const object = (properties, required = []) => ({ type: 'object', properties, required });
const TOOLS = [
  { name: 'get_guide', description: 'How the text language works, with an example. Read this first.', inputSchema: object({}) },
  { name: 'list_operators', description: 'The operators you may use: ports, parameters with units and ranges, one-line help.', inputSchema: object({}) },
  { name: 'get_text', description: 'The current document as canonical text, and its revision.', inputSchema: object({}) },
  { name: 'get_nodes', description: 'Every node with its id, label, operator, parameters, inputs, flags, the group it belongs to (nodes inside a group definition) or its instance data, plus the outputs and the groups. Use the ids in apply_commands.', inputSchema: object({}) },
  { name: 'get_status', description: 'Revision, node count, evaluation state (last failure, timings) and the last projected geometry.', inputSchema: object({}) },
  { name: 'apply_text', description: 'Adds the nodes described by a program in the text language to the document, evaluates it, and reports the outcome or the diagnostics (each with a line and column). Atomic: nothing changes unless all of it is valid.', inputSchema: object({ text: { type: 'string' }, label: { type: 'string' } }, ['text']) },
  { name: 'apply_commands', description: 'Applies typed commands, for example [{"set_param":{"node":"n_abc12345","param":"segments","value":96}}]. Commands: add_node, remove_node, connect, disconnect, set_param, set_label, set_flags, set_output, and for groups set_instance_param, clear_instance_param, upgrade_group, expose_param, unexpose_param, set_group_output. Atomic.', inputSchema: object({ commands: { type: 'array', items: { type: 'object' } }, label: { type: 'string' } }, ['commands']) },
  { name: 'undo', description: 'Undoes the last committed edit.', inputSchema: object({}) },
  { name: 'redo', description: 'Redoes the last undone edit.', inputSchema: object({}) },
  { name: 'frame_view', description: 'Moves the editor camera so the whole model fills the view, from a three-quarter angle. Call it after building something so the person watching sees it.', inputSchema: object({}) },
  { name: 'set_view', description: 'Moves the editor camera: position and target as [x, y, z] metres, so the person watching can see the result.', inputSchema: object({ position: { type: 'array', items: { type: 'number' } }, target: { type: 'array', items: { type: 'number' } } }, ['position', 'target']) },
];

const send = (message) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...message }) + '\n');
const content = (value, isError = false) => ({ content: [{ type: 'text', text: typeof value === 'string' ? value : JSON.stringify(value, null, 2) }], isError });

async function callTool(name, args) {
  if (name === 'get_guide') return content(GUIDE);
  if (!TOOLS.some((t) => t.name === name)) return content(`unknown tool ${name}`, true);
  const reply = await callPage(name, args ?? {});
  if (reply.error) return content(reply.error, true);
  return content(reply.result ?? {});
}

let buffer = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', async (chunk) => {
  buffer += chunk;
  let newline;
  while ((newline = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, newline).trim();
    buffer = buffer.slice(newline + 1);
    if (!line) continue;
    let message;
    try { message = JSON.parse(line); } catch { continue; }
    const { id, method, params } = message;
    if (method === 'initialize') {
      send({ id, result: { protocolVersion: params?.protocolVersion ?? '2024-11-05', capabilities: { tools: {} }, serverInfo: { name: 'spatial-engine', version: '0.1.0' } } });
    } else if (method === 'tools/list') {
      send({ id, result: { tools: TOOLS } });
    } else if (method === 'tools/call') {
      send({ id, result: await callTool(params?.name, params?.arguments) });
    } else if (method === 'ping') {
      send({ id, result: {} });
    } else if (id !== undefined) {
      send({ id, error: { code: -32601, message: `method not found: ${method}` } });
    }
  }
});
process.stdin.on('end', () => process.exit(0));
