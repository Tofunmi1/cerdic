// V3-12 and the socket frame gizmo (ADR 0015, ADR 0016): sockets of the selected part are drawn as oriented
// triads, a real click on one selects it, and choosing it as the pivot starts the move handles there.
import assert from "node:assert/strict";
import { lookAt, nodeId, openViewer, screenOf, select, shutdown } from "./support-v3.mjs";

const viewer = await openViewer(`base = solid.box(size=[1,1,1])
socket = frame.add(base, name="mount", position=[0,0.5,0], rotation=[0,0,90])
parent = xform.move(socket, by=[2,1,0])
leg = solid.box(size=[0.2,1,0.2])
foot = frame.add(leg, name="foot", position=[0,0.5,0])
attached = xform.attach(foot, to=parent, socket="foot", to_socket="mount")
output parent
output attached`, "v3-frames");
const { page } = viewer;
const near = (actual, expected, tolerance, why) =>
  assert(actual.every((value, index) => Math.abs(value - expected[index]) < tolerance), `${why}: ${actual} vs ${expected}`);
try {
  const attached = await nodeId(page, "attached");
  await lookAt(page, [1.2, 1.2, 5], [1.2, 0.8, 0]);
  await select(page, attached);

  // The frame's orientation comes through attachment: the foot now sits on the parent's mount, which is
  // turned a quarter turn about Z, so its X axis points up the world Y axis and its Y axis along -X.
  const frames = await page.evaluate((id) => window.__ve.evaluation.socketFrames([id]), attached);
  const foot = frames.find((frame) => frame.name === "foot");
  assert(foot, "the attached part carries the foot socket");
  near(foot.position, [2, 1.5, 0], 1e-5, "the socket origin");
  near(foot.axes[0], [0, 1, 0], 1e-5, "its X axis");
  near(foot.axes[1], [-1, 0, 0], 1e-5, "its Y axis");
  near(foot.axes[2], [0, 0, 1], 1e-5, "its Z axis");

  // The viewport draws it: a disc at the projected origin and three coloured axes.
  const disc = '#viewer-gizmo .gizmo-socket[data-socket-name="foot"]';
  await page.waitForSelector(disc, { timeout: 15000 });
  assert.equal(await page.locator("#viewer-gizmo .gizmo-frame").count() % 3, 0, "three axes for every socket");
  const where = await page.locator(disc).evaluate((circle) => {
    const rect = document.querySelector("#ve").getBoundingClientRect();
    return { x: rect.x + Number(circle.getAttribute("cx")), y: rect.y + Number(circle.getAttribute("cy")) };
  });
  const projected = await screenOf(page, foot.position);
  near([where.x, where.y], [projected.x, projected.y], 2, "the disc is where the socket projects on screen");

  // A real click on the disc selects the socket, in the viewport and in the inspector.
  await page.mouse.click(where.x, where.y);
  await page.waitForFunction(([part]) => {
    const picked = window.__ve.docPanel.selection.socket;
    return picked?.part === part && picked?.name === "foot";
  }, [attached], { timeout: 10000 });
  await page.waitForSelector(`${disc}.selected`, { timeout: 10000 });
  assert((await page.locator('#dock-inspector button.socket[data-socket="foot"]').getAttribute("class")).includes("selected"),
    "the inspector shows the same socket as selected");
  assert.equal((await page.evaluate(() => window.__ve.doc.state().revision)), 1, "picking a socket is view state: no command was sent");

  // As the pivot, the move handles start at the socket instead of at the part.
  await page.selectOption("#gizmo-pivot", "socket");
  // `attached`, not `visible`: this handle is exactly horizontal, so its box has no height.
  const handle = await page.waitForSelector('#viewer-gizmo [data-axis="0"]', { state: "attached", timeout: 15000 });
  const start = await handle.evaluate((line) => ({ x: Number(line.getAttribute("x1")), y: Number(line.getAttribute("y1")) }));
  const rect = await page.evaluate(() => document.querySelector("#ve").getBoundingClientRect().toJSON());
  near([rect.x + start.x, rect.y + start.y], [projected.x, projected.y], 3, "the handle starts at the socket");

  // V3-12: frame selected. From far away the part is a speck; F brings the selection to the middle of the view.
  const projectedBox = async () => page.evaluate((id) => {
    const bounds = window.__ve.evaluation.boundsByNode[id];
    const rect = document.querySelector("#ve").getBoundingClientRect();
    const points = [0, 1, 2, 3, 4, 5, 6, 7].map((corner) => [0, 1, 2].map((axis) => (corner >> axis) & 1 ? bounds.max[axis] : bounds.min[axis]))
      .map((corner) => JSON.parse(window.__ve.viewer.api.viewer_project(...corner, rect.width, rect.height)));
    const xs = points.map((p) => p.x), ys = points.map((p) => p.y);
    return { width: Math.max(...xs) - Math.min(...xs), centre: [(Math.max(...xs) + Math.min(...xs)) / 2 / rect.width, (Math.max(...ys) + Math.min(...ys)) / 2 / rect.height] };
  }, attached);
  await lookAt(page, [14, 10, 30], [0, 0, 0]); // `attached` is still the selection
  const far = await projectedBox();
  await page.evaluate(() => document.activeElement?.blur?.());
  await page.keyboard.press("KeyF");
  await page.waitForFunction((width) => {
    const rect = document.querySelector("#ve").getBoundingClientRect();
    const bounds = window.__ve.evaluation.boundsByNode[JSON.parse(window.__ve.viewer.api.selection_get()).node];
    if (!bounds) return false;
    const xs = [bounds.min, bounds.max].map((corner) => JSON.parse(window.__ve.viewer.api.viewer_project(...corner, rect.width, rect.height)).x);
    return Math.abs(xs[1] - xs[0]) > width * 3;
  }, far.width, { timeout: 15000 });
  const framed = await projectedBox();
  assert(framed.width > far.width * 3, `the selection fills more of the view: ${far.width} to ${framed.width}`);
  near(framed.centre, [0.5, 0.5], 0.2, "and sits near the middle");

  assert.deepEqual(viewer.errors, [], "no page errors");
  console.log("V3 frame gizmo acceptance passed");
} finally {
  await viewer.close();
  await shutdown();
}
