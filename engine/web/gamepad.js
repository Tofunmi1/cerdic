// Gamepad support for the viewer and for games. A standard-mapping controller does what the keyboard, mouse and desktop hand already do:
//   left stick   walk (right and forward), pressed in: sprint
//   right stick  look (turns the view like a mouse movement)
//   A / B        rise / sink (the free-fly viewer's E and Q)
//   right trigger  the desktop hand's trigger (use, fire, pull)     either bumper or left trigger  its grip (hold, carry)
// `readPad` is a pure function of the controller's state, so it is tested without a controller; `startGamepad` polls the browser.

const DEAD_ZONE = 0.18;
/** Look speed at full stick, in the pixels a mouse would have moved each second. */
const LOOK_PIXELS_PER_SECOND = 900;

/** A stick with a round dead zone, rescaled so the push just past the zone is the smallest motion, not a jump. */
export function stick(x, y, dead = DEAD_ZONE) {
  const length = Math.hypot(x, y);
  if (!(length > dead)) return [0, 0];
  const scaled = Math.min(1, (length - dead) / (1 - dead));
  return [(x / length) * scaled, (y / length) * scaled];
}

const down = (pad, index) => Boolean(pad.buttons?.[index]?.pressed);

/**
 * What a controller asks for this frame: `move` (right, up, forward in -1..1, sprint), `look` (pixels to turn by, `dt` seconds long) and the
 * desktop hand (`grip`, `trigger`).
 */
export function readPad(pad, dt) {
  const [lx, ly] = stick(pad.axes?.[0] ?? 0, pad.axes?.[1] ?? 0);
  const [rx, ry] = stick(pad.axes?.[2] ?? 0, pad.axes?.[3] ?? 0);
  const up = (down(pad, 0) ? 1 : 0) - (down(pad, 1) ? 1 : 0);
  return {
    move: { right: lx, up, forward: -ly, sprint: down(pad, 10) },
    look: { dx: rx * LOOK_PIXELS_PER_SECOND * dt, dy: ry * LOOK_PIXELS_PER_SECOND * dt },
    hand: { trigger: down(pad, 7), grip: down(pad, 5) || down(pad, 4) || down(pad, 6) },
  };
}

/** The first connected controller with the standard mapping (the one browsers give to most pads), or null. */
export function firstPad(pads) {
  for (const pad of pads ?? []) if (pad && pad.connected && pad.mapping === 'standard') return pad;
  return null;
}

/**
 * Polls the browser's controllers every frame and drives `api` (the runtime's exports). Only sends what changed, so an idle pad or no pad
 * costs a few comparisons. Returns a function that stops it.
 */
export function startGamepad(api, { getPads = () => globalThis.navigator?.getGamepads?.(), raf = globalThis.requestAnimationFrame?.bind(globalThis), now = () => performance.now() } = {}) {
  if (!raf) return () => {};
  let last = now();
  let handle = 0;
  let moving = false;
  let sentHand = '';
  const tick = () => {
    const t = now();
    const dt = Math.min(0.1, (t - last) / 1000);
    last = t;
    const pad = firstPad(getPads());
    if (pad) {
      const { move, look, hand } = readPad(pad, dt);
      const active = move.right !== 0 || move.up !== 0 || move.forward !== 0;
      // Send the motion while it is wanted and once more when it stops (zero), so releasing the stick stops the view.
      if (active || moving) api.viewer_move?.(move.right, move.up, move.forward, move.sprint);
      moving = active;
      if (look.dx !== 0 || look.dy !== 0) api.viewer_look?.(look.dx, look.dy);
      const key = `${hand.grip}|${hand.trigger}`;
      if (key !== sentHand) {
        sentHand = key;
        api.game_desktop_hand?.(hand.grip, hand.trigger);
      }
    } else if (moving) {
      api.viewer_move?.(0, 0, 0, false);
      moving = false;
    }
    handle = raf(tick);
  };
  handle = raf(tick);
  return () => {
    globalThis.cancelAnimationFrame?.(handle);
    raf = null;
  };
}
