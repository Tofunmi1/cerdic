// Conversations belong to a project and world. Legacy global messages are left untouched,
// because there is no reliable way to assign them to a world after the fact.
import { useEffect, useSyncExternalStore } from 'react';
export type ToolCall = { name: string; status: 'running' | 'done' | 'error'; summary?: string; input?: unknown; output?: unknown };
export type Entry = { id: string; role: 'user' | 'assistant' | 'activity'; ts: number; text?: string; tool?: ToolCall };
export type Conversation = { id: string; title: string; entries: Entry[]; createdAt: number };
type Bucket = { activeId: string; conversations: Conversation[]; error?: string };
export type ChatScope = { projectId: string; worldId: string };
const empty: Bucket = { activeId: '', conversations: [] };
const buckets = new Map<string, Bucket>();
const listeners = new Set<() => void>();
const keyOf = (scope: ChatScope) => `cerdic-chat:${scope.projectId}:${scope.worldId}`;
const fresh = (): Conversation => ({ id: crypto.randomUUID(), title: 'New conversation', entries: [], createdAt: Date.now() });
function read(key: string): Bucket {
  if (buckets.has(key)) return buckets.get(key)!;
  let bucket: Bucket;
  try {
    const parsed = JSON.parse(localStorage.getItem(key) ?? 'null') as Bucket | null;
    if (parsed && Array.isArray(parsed.conversations) && parsed.conversations.length && parsed.conversations.every(c => typeof c.id === 'string' && typeof c.title === 'string' && Array.isArray(c.entries))) {
      bucket = { ...parsed, activeId: parsed.conversations.some(c => c.id === parsed.activeId) ? parsed.activeId : parsed.conversations[0].id };
    } else { const chat = fresh(); bucket = { activeId: chat.id, conversations: [chat] }; }
  } catch { const chat = fresh(); bucket = { activeId: chat.id, conversations: [chat] }; }
  buckets.set(key, bucket);
  return bucket;
}
function write(key: string, bucket: Bucket) {
  try { localStorage.setItem(key, JSON.stringify(bucket)); bucket = { ...bucket, error: undefined }; }
  catch { bucket = { ...bucket, error: 'Conversation could not be saved. Browser storage may be full or blocked.' }; }
  buckets.set(key, bucket);
  listeners.forEach(fn => fn());
}
export function useTranscript(scope: ChatScope | null) {
  const key = scope ? keyOf(scope) : null;
  const bucket = useSyncExternalStore(fn => { listeners.add(fn); return () => { listeners.delete(fn); }; }, () => key ? read(key) : empty);
  const current = bucket.conversations.find(c => c.id === bucket.activeId);
  const mutateEntries = (change: (entries: Entry[]) => Entry[], title?: string) => {
    if (!key) return;
    const latest = read(key);
    write(key, { ...latest, conversations: latest.conversations.map(c => c.id === latest.activeId ? { ...c, entries: change(c.entries).slice(-500), title: title && !c.entries.some(e => e.role === 'user') ? title : c.title } : c) });
  };
  const send = (text: string) => mutateEntries(entries => [...entries, { id: crypto.randomUUID(), role: 'user', ts: Date.now(), text }], text.slice(0, 50));
  const clear = () => {
    if (!key) return;
    const latest = read(key), chat = fresh();
    // A cleared conversation also gets a fresh agent identity, so hidden model
    // history cannot survive a clear of the visible conversation.
    write(key, { ...latest, activeId: chat.id, conversations: latest.conversations.map(c => c.id === latest.activeId ? chat : c) });
  };
  const log = (entry: Omit<Entry, 'ts'> & { ts?: number }) => mutateEntries(entries => {
    const full = { ...entry, ts: entry.ts ?? Date.now() } as Entry;
    const index = entries.findIndex(existing => existing.id === entry.id);
    if (index < 0) return [...entries, full];
    return entries.map((existing, i) => i === index ? { ...existing, ...full, ts: existing.ts } : existing);
  });
  const newChat = () => {
    if (!key) return;
    const latest = read(key), chat = fresh();
    write(key, { ...latest, activeId: chat.id, conversations: [chat, ...latest.conversations] });
  };
  const selectChat = (id: string) => {
    if (!key) return;
    const latest = read(key);
    if (latest.conversations.some(c => c.id === id)) write(key, { ...latest, activeId: id });
  };
  // An external transcript must identify its exact project, world and conversation.
  useEffect(() => {
    if (!key || !scope || !bucket.activeId) return;
    let stop = false;
    let timer: ReturnType<typeof setTimeout>;
    const conversationId = bucket.activeId;
    const poll = async () => {
      try {
        const response = await fetch('/transcript.json', { cache: 'no-store' });
        if (response.ok) {
          const data = await response.json() as Partial<ChatScope> & { conversationId?: string; entries?: Entry[] };
          if (!stop && data.projectId === scope.projectId && data.worldId === scope.worldId && data.conversationId === conversationId && Array.isArray(data.entries)) {
            const latest = read(key);
            const chat = latest.conversations.find(c => c.id === conversationId);
            if (chat) {
              const merged = new Map(chat.entries.map(e => [e.id, e]));
              let changed = false;
              for (const entry of data.entries) {
                if (typeof entry.id !== 'string' || typeof entry.ts !== 'number' || !['assistant', 'activity'].includes(entry.role)) continue;
                if (JSON.stringify(merged.get(entry.id)) !== JSON.stringify(entry)) { merged.set(entry.id, entry); changed = true; }
              }
              if (changed) write(key, { ...latest, conversations: latest.conversations.map(c => c.id === conversationId ? { ...c, entries: [...merged.values()].sort((a,b) => a.ts-b.ts).slice(-500) } : c) });
            }
          }
        }
      } catch { /* An external transcript is optional. */ }
      if (!stop) timer = setTimeout(poll, 1500);
    };
    void poll();
    return () => { stop = true; clearTimeout(timer); };
  }, [key, bucket.activeId, scope?.projectId, scope?.worldId]);
  return { entries: current?.entries ?? [], send, clear, log, newChat, selectChat, conversations: bucket.conversations, conversationId: bucket.activeId, error: bucket.error };
}
