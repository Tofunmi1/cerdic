# ADR 0074: The game's UI is a declared, screen-space tree drawn from procedural strokes

- Status: accepted (F4 of `plans/game-foundations-plan.md`)
- Date: 2026-11-08

## Context

Games need menus, a title and pause screen, health and score, prompts, a minimap, dialogue choices and tooltips. The engine already has a screen-space layer: scripts set elements (text, bar, panel) by id, `ve-game/src/ui.rs` lays them out in fractions of the screen height and returns rectangles and pen strokes, and `src/studio/GameUi.tsx` draws them on a canvas. Text comes from procedural skeleton fonts, so there are no font files (ADR 0003 still holds: nothing is pulled). Separately, `game.hud` draws pixel text on a panel in the world, which stays right for diegetic panels but is wrong for a menu.

## Decision

1. **The UI is a screen-space layer, not geometry in the world.** In-world panels (`game.hud`) remain for diegetic screens. Everything else is the UI tree, drawn over the 3D view, identical on a desktop window and any aspect ratio (units are fractions of the screen height, x runs from 0 to the aspect ratio).
2. **The tree is declared in the game text** (`ui ID type=... parent=... show=... ...`, node `game.ui@1`), not only built by scripts, so a person and a model describe it the same way as the rest of the game and it is part of the game's definition, hash and save. Scripts keep their own elements; both are drawn, declared ones first.
3. **The tree is driven by game state, never the other way round.** Text interpolates variables (`{score}`), bars bind to variables, elements show only in the flow states they name (`show=title,paused`), and a button raises the event `ui:ID` when accepted (or runs the actions given in `do=`). The UI holds one piece of state of its own, the focused button, which lives in the flow state so it is saved and replayed.
4. **Layout is by container.** `stack` (down or right, gap, padding, alignment) and `grid` (columns, cell size) place their children; everything else is placed by anchor and offset. The layout is computed in `ve-game` (pure, tested), which also answers "what is at this point" for the pointer.
5. **Navigation is by named actions**, `ui_next`, `ui_prev`, `ui_accept`, which the page binds to the keyboard (arrows, Tab, Enter) and the gamepad (d-pad, A) by default through the action map (ADR-less F6 module `actionMap.ts`); a pointer click is `game_ui_click(x, y)`. Visible buttons are focus-ordered by declaration.
6. **Prompts follow the device.** `{key:ACTION}` in a text is replaced by the label the page supplies for the active device (`W A S D`, `Space`, `A`), so a prompt never names a key the player is not using.
7. **Drawing stays a thin renderer.** The page draws the rectangles and strokes it is handed; it decides nothing about what is shown. A native build can draw the same frame itself.

## Consequences

- A title, pause, game over, HUD, menu and dialogue are a few lines of nodes; the example games use them.
- The UI is deterministic and testable without a browser (layout rectangles, focus order, bindings, hit tests).
- Two surfaces exist for UI (declared and scripted); a game that needs animation or logic beyond bindings uses a script for that element.
- Not in this decision: fonts beyond the skeleton presets, images and icons from files (none are pulled), rich text, and in-world floating numbers (they need a world-to-screen projection from the host and arrive with the game-feel work).
