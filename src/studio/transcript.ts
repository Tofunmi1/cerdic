// The conversation shown in the chat panel. There is no model in the page yet, so the assistant (Claude Code in the terminal) writes its side to
// public/transcript.json and the panel polls it; what the person types is kept in the browser. Engine events (a document committed, an evaluation
// running) become activity cards on their own, so the panel always shows what the engine is doing.
import { useEffect, useRef, useState } from 'react';

export type ToolCall = { name: string; status: 'running' | 'done' | 'error'; summary?: string; input?: unknown; output?: unknown };
export type Entry = {
  id: string; role: 'user' | 'assistant' | 'activity'; ts: number;
  text?: string; tool?: ToolCall;
};

const LOCAL_KEY = 'studio-chat-local';

function readLocal(): Entry[] {
  try { return JSON.parse(localStorage.getItem(LOCAL_KEY) ?? '[]') as Entry[]; } catch { return []; }
}

/** The merged conversation: the assistant's transcript file, the person's own messages, and engine activity, in time order. */
export function useTranscript() {
  const [remote, setRemote] = useState<Entry[]>([]);
  const [local, setLocal] = useState<Entry[]>(readLocal);
  const [activity, setActivity] = useState<Entry[]>([]);
  const etag = useRef('');

  useEffect(() => {
    let stop = false;
    const poll = async () => {
      try {
        const response = await fetch('/transcript.json', { cache: 'no-store' });
        if (response.ok) {
          const text = await response.text();
          if (text !== etag.current) {
            etag.current = text;
            const data = JSON.parse(text) as { entries?: Entry[] };
            if (!stop) setRemote(data.entries ?? []);
          }
        }
      } catch { /* no transcript yet */ }
      if (!stop) setTimeout(poll, 1500);
    };
    void poll();
    return () => { stop = true; };
  }, []);

  const send = (text: string) => {
    const entry: Entry = { id: `u${Date.now()}`, role: 'user', ts: Date.now(), text };
    setLocal(previous => {
      const next = [...previous, entry].slice(-200);
      try { localStorage.setItem(LOCAL_KEY, JSON.stringify(next)); } catch { /* ignore */ }
      return next;
    });
  };
  const clear = () => { setLocal([]); try { localStorage.removeItem(LOCAL_KEY); } catch { /* ignore */ } setActivity([]); };
  const log = (entry: Omit<Entry, 'ts'> & { ts?: number }) =>
    setActivity(previous => {
      const index = previous.findIndex(existing => existing.id === entry.id);
      const full = { ...entry, ts: entry.ts ?? Date.now() } as Entry;
      if (index >= 0) { const copy = previous.slice(); copy[index] = { ...copy[index], ...full, ts: copy[index].ts }; return copy; }
      return [...previous, full].slice(-100);
    });

  const entries = [...remote, ...local, ...activity].sort((a, b) => a.ts - b.ts);
  return { entries, send, clear, log };
}
