import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import {
  ArrowDown,
  ArrowUp,
  Box,
  ChevronDown,
  Circle,
  Command,
  Headset,
  Layers,
  LoaderCircle,
  MessageSquare,
  MousePointer2,
  Move3D,
  Plus,
  Redo2,
  Rotate3D,
  RotateCcw,
  Save,
  Scaling,
  Sparkles,
  Undo2,
  X,
} from "lucide-react";
import type { SceneCommand, SceneDocument } from "../shared/commands";
import { makeObject, responseSchema } from "../shared/commands";
import { commit, createHistory, redo, undo } from "../shared/history";
import type { History } from "../shared/history";
import { readScene, saveScene, starterScene } from "./scene/document";
import Viewport, { xrStore } from "./scene/Viewport";
import type { Tool } from "./scene/Viewport";
import Inspector, { FactoryIcon } from "./components/Inspector";
import { useLiquidGlass } from "./ui/useLiquidGlass";

type Message = { role: "user" | "assistant"; text: string };

export default function App() {
  const workspaceRef = useRef<HTMLElement>(null);
  useLiquidGlass(workspaceRef);
  const [history, setHistory] = useState<History>(() => {
    try {
      return createHistory(readScene());
    } catch {
      return createHistory(structuredClone(starterScene));
    }
  });
  const historyRef = useRef(history);
  historyRef.current = history;
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [tool, setTool] = useState<Tool>("translate");
  const [prompt, setPrompt] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [mode, setMode] = useState("connecting");
  const [vr, setVR] = useState(false);
  const [notice, setNotice] = useState("");
  const [addMenu, setAddMenu] = useState(false);
  const [chatOpen, setChatOpen] = useState(true);
  const messageEnd = useRef<HTMLDivElement>(null);
  const scene = history.present;
  const selected = scene.objects.find((object) => object.id === selectedId);

  const notify = useCallback((text: string) => setNotice(text), []);
  useEffect(() => {
    const timer = setTimeout(() => setNotice(""), 5500);
    return () => clearTimeout(timer);
  }, [notice]);
  useEffect(() => {
    fetch("/api/status")
      .then((response) => {
        if (!response.ok) throw new Error();
        return response.json();
      })
      .then((result: { mode: string }) => setMode(result.mode))
      .catch(() => setMode("offline"));
    navigator.xr
      ?.isSessionSupported("immersive-vr")
      .then(setVR)
      .catch(() => setVR(false));
  }, []);
  useEffect(() => {
    messageEnd.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, busy]);

  const dispatch = useCallback(
    (commands: SceneCommand[]) => {
      try {
        const next = commit(historyRef.current, commands);
        historyRef.current = next;
        setHistory(next);
      } catch (error) {
        notify(error instanceof Error ? error.message : "Invalid scene change");
      }
    },
    [notify],
  );
  const stepHistory = useCallback((direction: "undo" | "redo") => {
    const next =
      direction === "undo"
        ? undo(historyRef.current)
        : redo(historyRef.current);
    historyRef.current = next;
    setHistory(next);
  }, []);
  const replaceScene = (nextScene: SceneDocument) => {
    const current = historyRef.current;
    const next = {
      past: [...current.past, current.present].slice(-100),
      present: nextScene,
      future: [],
      revision: current.revision + 1,
    };
    historyRef.current = next;
    setHistory(next);
    setSelectedId(null);
  };
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (
        (event.target as HTMLElement)?.matches(
          "input, textarea, [contenteditable]",
        )
      )
        return;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "z") {
        event.preventDefault();
        stepHistory(event.shiftKey ? "redo" : "undo");
      }
      if (event.key === "Escape") setSelectedId(null);
      if (event.key === "1") setTool("translate");
      if (event.key === "2") setTool("rotate");
      if (event.key === "3") setTool("scale");
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [stepHistory]);

  const sendPrompt = async (text: string) => {
    if (!text.trim() || busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setPrompt("");
    setMessages((current) => [...current, { role: "user", text }]);
    const snapshot = historyRef.current;
    try {
      const response = await fetch("/api/scene", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          prompt: text,
          scene: snapshot.present,
          selectedId: selected?.id ?? null,
        }),
        signal: AbortSignal.timeout(55000),
      });
      if (!response.ok) {
        const error = (await response.json()) as { error?: string };
        throw new Error(error.error ?? "Scene request failed");
      }
      const result = responseSchema.parse(await response.json());
      if (historyRef.current.revision !== snapshot.revision)
        throw new Error(
          "The scene changed while this request was running. Please send the prompt again.",
        );
      const next = commit(snapshot, result.commands);
      historyRef.current = next;
      setHistory(next);
      const created = result.commands.find(
        (command) => command.type === "create",
      );
      if (created?.type === "create") setSelectedId(created.object.id);
      setMessages((current) => [
        ...current,
        { role: "assistant", text: result.message },
      ]);
    } catch (error) {
      setMessages((current) => [
        ...current,
        {
          role: "assistant",
          text:
            error instanceof Error
              ? error.message
              : "Could not reach the scene adapter.",
        },
      ]);
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };
  const enterVR = async () => {
    try {
      await xrStore.enterVR();
    } catch (error) {
      notify(error instanceof Error ? error.message : "Could not enter VR");
    }
  };

  return (
    <main className="workspace" ref={workspaceRef}>
      <div className="viewport" data-testid="viewport">
        <Viewport
          objects={scene.objects}
          selectedId={selected?.id ?? null}
          tool={tool}
          onSelect={setSelectedId}
          onCommands={dispatch}
        />
      </div>
      <div className="viewport-vignette" />
      <header className="topbar">
        <div className="brand">
          <div className="brand-icon">
            <Layers size={21} strokeWidth={1.5} />
          </div>
          <div>
            spatial<span>lab</span>
            <small>AN OPEN CANVAS FOR IDEAS</small>
          </div>
        </div>
        <div className="scene-title glass">
          <span className="status-dot" />
          <span>Untitled studio</span>
          <span className="title-divider" />
          <span className="muted">Local workspace</span>
        </div>
        <div className="header-actions glass">
          <button
            title="Reload saved scene"
            aria-label="Reload scene"
            onClick={() => {
              try {
                replaceScene(readScene());
                notify("Saved scene reloaded");
              } catch {
                notify("Saved scene is invalid or unavailable");
              }
            }}
          >
            <RotateCcw size={16} />
          </button>
          <button
            title="Save to this browser"
            aria-label="Save"
            onClick={() => {
              try {
                saveScene(scene);
                notify("Scene saved to this browser");
              } catch {
                notify("Browser storage is unavailable");
              }
            }}
          >
            <Save size={16} />
            <span>Save</span>
          </button>
          <div className="vertical-rule" />
          <button
            className="vr-button"
            aria-label="Enter VR"
            disabled={!vr}
            title={
              vr
                ? "Enter immersive VR"
                : "WebXR headset unavailable. Requires HTTPS or localhost."
            }
            onClick={() => void enterVR()}
          >
            <Headset size={17} />
            <span>Enter VR</span>
          </button>
        </div>
      </header>
      <div className={`left-column ${chatOpen ? "" : "chat-closed"}`}>
        {chatOpen && (
          <section className="chat-panel surface" aria-label="Scene assistant">
            <div className="panel-title">
              <div>
                <Sparkles size={16} />
                <span>Scene assistant</span>
              </div>
              <button
                className="icon-button"
                aria-label="Close chat"
                onClick={() => setChatOpen(false)}
              >
                <X size={15} />
              </button>
            </div>
            <div className="provider-label">
              <span
                className={`status-dot ${mode === "offline" ? "offline" : ""}`}
              />
              {mode === "mock"
                ? "Mock mode · no LLM connected"
                : mode === "offline"
                ? "Server offline · start bun run dev"
                : mode === "connecting"
                ? "Connecting to scene adapter…"
                : "Live provider · server connected"}
            </div>
            <div className="chat-scroll">
              {!messages.length && (
                <div className="chat-intro">
                  <div className="assistant-orb">
                    <Sparkles size={24} strokeWidth={1.5} />
                  </div>
                  <p className="eyebrow">FROM WORDS TO WORLDS</p>
                  <h1>
                    Make room
                    <br />
                    for an idea.
                  </h1>
                  <p>
                    Describe a scene. Shape it together.
                    <br />
                    Every object is yours to change.
                  </p>
                  <div className="suggestions">
                    <button
                      disabled={
                        busy || mode === "connecting" || mode === "offline"
                      }
                      onClick={() => void sendPrompt("Build a table")}
                    >
                      <Box size={14} />
                      <span>Build a simple table</span>
                      <ArrowUp size={13} />
                    </button>
                    <button
                      disabled={
                        busy || mode === "connecting" || mode === "offline"
                      }
                      onClick={() => void sendPrompt("Add 3 blue columns")}
                    >
                      <Circle size={14} />
                      <span>Add three blue columns</span>
                      <ArrowUp size={13} />
                    </button>
                  </div>
                  <p className="mock-explanation">
                    {mode === "mock"
                      ? "Mock understands tables, boxes, columns, planes, colours and basic edits. Try “move it left” after selecting an object."
                      : "Use follow-up prompts to change selected objects."}
                  </p>
                </div>
              )}
              {messages.map((message, i) => (
                <div className={`chat-message ${message.role}`} key={i}>
                  {message.role === "assistant" && (
                    <span className="message-label">
                      <Sparkles size={11} /> Spatial Lab
                    </span>
                  )}
                  <p>{message.text}</p>
                </div>
              ))}
              {busy && (
                <div className="thinking">
                  <LoaderCircle size={13} className="spin" /> Shaping your
                  scene…
                </div>
              )}
              <div ref={messageEnd} />
            </div>
            <form
              className="prompt-form"
              onSubmit={(event) => {
                event.preventDefault();
                void sendPrompt(prompt);
              }}
            >
              <textarea
                aria-label="Describe your scene"
                placeholder={
                  selected
                    ? `Change ${selected.name.toLowerCase()}…`
                    : "Describe what you imagine…"
                }
                rows={2}
                value={prompt}
                maxLength={4000}
                onChange={(e) => setPrompt(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    void sendPrompt(prompt);
                  }
                }}
              />
              <div className="prompt-bottom">
                <span>
                  {selected ? (
                    <>
                      <MousePointer2 size={11} /> {selected.name}
                    </>
                  ) : (
                    <>
                      <Command size={11} /> Enter to create
                    </>
                  )}
                </span>
                <button
                  aria-label="Send prompt"
                  disabled={
                    busy ||
                    !prompt.trim() ||
                    mode === "offline" ||
                    mode === "connecting"
                  }
                >
                  {busy ? (
                    <LoaderCircle size={15} className="spin" />
                  ) : (
                    <ArrowUp size={17} />
                  )}
                </button>
              </div>
            </form>
          </section>
        )}
        <section className="scene-panel glass" aria-label="Scene objects">
          <div className="panel-title">
            <div>
              <Layers size={14} />
              <span>Scene</span>
              <span className="count">{scene.objects.length}</span>
            </div>
            <button
              className="icon-button"
              aria-label="Toggle assistant"
              onClick={() => setChatOpen((open) => !open)}
            >
              <MessageSquare size={14} />
            </button>
          </div>
          <div className="scene-list">
            {scene.objects.map((object) => (
              <button
                data-testid={`object-${object.id}`}
                className={`scene-row ${
                  selectedId === object.id ? "selected" : ""
                }`}
                key={object.id}
                onClick={() => setSelectedId(object.id)}
              >
                <FactoryIcon factory={object.factory} size={14} />
                <span>{object.name}</span>
                <i style={{ background: object.color }} />
              </button>
            ))}
          </div>
        </section>
      </div>
      <Inspector object={selected} onCommands={dispatch} />
      <div className="view-label">
        <span className="view-dot" /> PERSPECTIVE <span>·</span> METERS
      </div>
      <div className="viewport-guide">
        <MousePointer2 size={12} /> Drag to orbit <span>·</span> Scroll to zoom
      </div>
      <div
        className="bottom-toolbar glass"
        role="toolbar"
        aria-label="Scene tools"
      >
        <button
          title="Undo (⌘Z)"
          aria-label="Undo"
          disabled={!history.past.length}
          onClick={() => stepHistory("undo")}
        >
          <Undo2 size={17} />
        </button>
        <button
          title="Redo (⇧⌘Z)"
          aria-label="Redo"
          disabled={!history.future.length}
          onClick={() => stepHistory("redo")}
        >
          <Redo2 size={17} />
        </button>
        <div className="vertical-rule" />
        <div
          className="tool-group"
          style={
            {
              "--tool-index": ["translate", "rotate", "scale"].indexOf(tool),
            } as CSSProperties
          }
        >
          <span className="tool-selection" aria-hidden="true" />
          {(
            [
              { mode: "translate", label: "Move", Icon: Move3D },
              { mode: "rotate", label: "Rotate", Icon: Rotate3D },
              { mode: "scale", label: "Scale", Icon: Scaling },
            ] as const
          ).map(({ mode: modeName, label, Icon }, i) => (
            <button
              key={modeName}
              aria-label={label}
              aria-pressed={tool === modeName}
              className={tool === modeName ? "active-tool" : ""}
              title={`${label} (${i + 1})`}
              onClick={() => setTool(modeName)}
            >
              <Icon size={17} />
              <span>{label}</span>
            </button>
          ))}
        </div>
        <div className="vertical-rule" />
        <div className="add-wrapper">
          <button
            aria-label="Add object"
            aria-expanded={addMenu}
            onClick={() => setAddMenu(!addMenu)}
          >
            <Plus size={17} />
            <span>Add</span>
            <ChevronDown size={12} />
          </button>
          {addMenu && (
            <div className="add-menu glass">
              {(["box", "cylinder", "plane"] as const).map((factory) => (
                <button
                  key={factory}
                  onClick={() => {
                    const object = makeObject(
                      factory,
                      `${factory[0].toUpperCase()}${factory.slice(1)}`,
                      { position: [0, factory === "plane" ? 0.05 : 0.7, 2] },
                    );
                    dispatch([{ type: "create", object }]);
                    setSelectedId(object.id);
                    setAddMenu(false);
                  }}
                >
                  <FactoryIcon factory={factory} />
                  <span>{factory}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      <footer className="workspace-footer">
        <span>
          <i className="status-dot" /> Geometry engine ready{" "}
          <span className="muted">· Rust / WASM</span>
        </span>
        <span className="footer-right">
          A little imagination. A lot of possibility. <ArrowDown size={12} />
        </span>
      </footer>
      {notice && (
        <div role="status" className="toast surface">
          {notice}
          <button
            className="icon-button"
            aria-label="Dismiss notification"
            onClick={() => setNotice("")}
          >
            <X size={13} />
          </button>
        </div>
      )}
    </main>
  );
}
