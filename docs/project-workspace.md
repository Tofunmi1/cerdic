# Projects and world conversations

Open `/studio` to create or reopen a project. The Projects button in the Studio toolbar opens the same library. A project starts with one world, from blank text, a bundled example, or a `.txt`, `.world` or `.ve` file. Examples and imported files create separate projects rather than replacing another project's saved world.

Projects have stable IDs and a distinct stable world ID; renaming changes neither. The runtime derives document IDs from its seed, so each world uses a stable seed derived from its world name, distinct from the startup document. The workspace records the runtime ID when opening a world. Each committed edit synchronously checkpoints scene text to localStorage before the debounced IndexedDB save. Opening a project recovers a newer checkpoint, including an intentionally empty world, so reload does not depend on an asynchronous unload save. Changes are also flushed before switching projects. Loading waits for evaluation; parse/evaluation failures restore the previous world and do not keep a broken new project. Storage errors remain visible rather than showing a successful save.

The address becomes `/studio?project=<id>`; reload restores that browser's project. Existing `?world=` and `?example=` entry links create a project from their source and then adopt the project address. The library is local to this browser/origin, not a server account. Export scene text for an independent copy; the text export does not include conversations.

## Conversation ownership

Each `(projectId, worldId)` has its own conversation list. The conversation selector restores a previous thread; **New conversation** creates an empty thread without resetting the world. **Clear conversation** clears the selected thread only. The selected conversation and messages survive reload. Chat storage errors are shown below the composer. Legacy global chat data is retained under its old key but is not attached to an arbitrary project.

External `transcript.json` entries are accepted only when the payload explicitly matches the active context:

```json
{
  "projectId": "p-…",
  "worldId": "…",
  "conversationId": "…",
  "entries": [{ "id": "reply-1", "role": "assistant", "ts": 123, "text": "…" }]
}
```

## Sidebar assistant

Run Vite and `bun server/index.ts` (or the normal `bun run dev` command). Codex must be installed and signed in on the same computer (`codex login`). In Studio choose **Connect assistant → Connect local Codex**, select an available model, then **Start chatting**. The model list comes from the signed-in account; no API key is copied into the browser.

The local Bun server starts a dedicated Codex app-server process for the browser connection. Its experimental dynamic tools call the same browser engine dispatcher used by MCP; it does not start another copy of our registered MCP bridge or take over the external agent connection. Replies stream into the sidebar and tool calls become expandable activity cards. **Stop** interrupts the agent; a tool already executing may finish its current scene edit. Project and conversation switching are disabled while a turn is running.

Each `(projectId, worldId, conversationId)` maps to an independent persistent Codex thread. The mapping is stored locally in ignored `target/assistant-threads.json`; Codex stores its thread history using its own local session storage. Reloading the project restores the sidebar messages; reconnect local Codex to continue the same thread. A newly connected existing conversation includes its prior visible text when its agent thread is first created. **Clear conversation** rotates the conversation identity so the next prompt cannot inherit hidden agent context from the cleared thread.

The server listens on loopback and validates WebSocket origins. Agent threads use read-only sandboxing and no selected environments, with scene tools supplied directly by the application. Credentials remain inside the local Codex installation. This is a local development integration, not a hosted multi-user service. API-key provider adapters and authenticated remote MCP pairing remain separate work. This release supports one world per project.

For an external agent instead, the connection panel still offers MCP setup instructions and a local bridge toggle. That option uses `engine/tools/mcp/server.mjs` over stdio, with prompts and model selection in the external application. The sidebar's local Codex option does not require enabling that bridge.

## Verification

With Vite running on port 5173:

```sh
bun test tests/library.test.ts
bun test tests/world-checkpoints.test.ts
bun tests/project-flow.mjs
bun tests/world-reload-recovery.mjs
bun tests/startup-scenes.mjs
bun run typecheck
bun run build:web
# Opt-in: consumes tokens on the signed-in Codex account.
bun tests/sidebar-live.mjs
```

The focused browser check uses one headless Chromium and the real WASM engine. It covers creating two projects, geometry persistence through project switches and reload, independent conversations, new chat/history, renaming without losing identity, invalid-import recovery and a mobile dialog layout. It also opens the connection panel and exercises bridge enable/disconnect with an intercepted WebSocket, so the check does not take over a real agent's local bridge. Captures are written to `target/reports/project-flow/`.

The reload recovery check deliberately fails an IndexedDB write, confirms the stored project is still blank, then reloads and verifies geometry and chat recover. It also reloads immediately after another committed edit, before the autosave debounce. The startup check watches individual frames for legacy scene flashes.

The opt-in live sidebar check connects to the real local Codex account, creates a blue box through the engine, changes its measured height from 2 m to 3 m while retaining the box's node ID, then starts and stops a separate conversation. Captures are in `target/reports/sidebar-live/`. The integration follows the [Codex app-server protocol](https://developers.openai.com/codex/app-server/).
