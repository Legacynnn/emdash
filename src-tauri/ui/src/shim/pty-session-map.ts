// sessionId ↔ PtyId map used by the renderer-shim's pty.* routes.
//
// The Electron renderer thinks in opaque string `sessionId`s
// (`projectId:scopeId:leafId`); Tauri exposes its PTY surface via
// `PtyId`s returned from `pty_spawn` / `agents_start`. The renderer
// keeps using string sessionIds — this map handles the translation.
//
// `associate(...)` is called by the agent-spawn path (the
// `attachAgentSession` helper in event-bridge.ts) once the host
// hands back a PtyId. `pty.subscribe` is idempotent: if no PtyId is
// bound yet the entry is just registered as "pending" and writes
// queue until a PtyId is associated. (In practice agents spawn
// synchronously enough that the queue stays empty, but the design
// avoids ordering races.)

type Pending = { kind: 'pending' };
type Bound = { kind: 'bound'; ptyId: string };
type Entry = Pending | Bound;

const entries = new Map<string, Entry>();

export const ptySessionMap = {
  ensure(sessionId: string): void {
    if (!entries.has(sessionId)) entries.set(sessionId, { kind: 'pending' });
  },
  associate(sessionId: string, ptyId: string): void {
    entries.set(sessionId, { kind: 'bound', ptyId });
  },
  get(sessionId: string): string | undefined {
    const e = entries.get(sessionId);
    return e?.kind === 'bound' ? e.ptyId : undefined;
  },
  forget(sessionId: string): void {
    entries.delete(sessionId);
  },
  // Returns a snapshot for debugging — not used in production paths.
  snapshot(): Record<string, Entry> {
    return Object.fromEntries(entries);
  },
};
