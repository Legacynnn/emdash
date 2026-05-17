// `window.electronAPI` polyfill backed by Tauri.
//
// The Electron renderer reads `window.electronAPI.{invoke,eventSend,
// eventOn,getPathForFile}` from `src/preload/index.ts`. To run that
// renderer inside Tauri without UI changes, this shell installs the
// same shape over Tauri's invoke + event channels.
//
// invoke()         -> @tauri-apps/api/core invoke (with name + arg
//                     translation via the routing table)
// eventOn/Send     -> Tauri Channel-backed bridge that translates each
//                     UiMutationEvent variant into the renderer's
//                     per-channel topics
// getPathForFile() -> File.webkitRelativePath fallback (DOM File API
//                     does not expose absolute paths in the webview)
//
// The routing table lives in `./route-table.ts`. The event bridge lives
// in `./event-bridge.ts`. Anything not covered yet rejects with a
// clearly-marked NotImplementedError so the renderer surfaces the
// missing surface explicitly instead of hanging silently.

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { installEventBridge, registerEventListener } from './event-bridge';
import { resolveRoute } from './route-table';

declare global {
  interface Window {
    electronAPI?: {
      invoke: (channel: string, ...args: unknown[]) => Promise<unknown>;
      eventSend: (channel: string, data: unknown) => void;
      eventOn: (channel: string, cb: (data: unknown) => void) => () => void;
      getPathForFile: (file: File) => string;
    };
  }
}

export class TauriShimNotImplementedError extends Error {
  constructor(
    public readonly channel: string,
    public readonly args: unknown[]
  ) {
    super(`[tauri-shim] No Tauri route for channel "${channel}"`);
    this.name = 'TauriShimNotImplementedError';
  }
}

async function shimInvoke(channel: string, ...args: unknown[]): Promise<unknown> {
  const route = resolveRoute(channel);
  if (!route) {
    logToHost('error', `missing route: ${channel} ${safeStringify(args)}`);
    // eslint-disable-next-line no-console
    console.warn(`[tauri-shim] missing route: ${channel}`, args);
    throw new TauriShimNotImplementedError(channel, args);
  }

  if (route.kind === 'static') {
    return route.value;
  }

  if (route.kind === 'invoke') {
    const tauriArgs = route.adapt ? route.adapt(args) : undefined;
    try {
      const result = await tauriInvoke(route.command, tauriArgs);
      return route.transform ? route.transform(result, args) : result;
    } catch (err) {
      const normalized = normalizeTauriError(err, channel);
      logToHost('error', `invoke ${channel} → ${route.command}: ${normalized.message}`);
      throw normalized;
    }
  }

  if (route.kind === 'custom') {
    try {
      return await route.handler(args);
    } catch (err) {
      const normalized = normalizeTauriError(err, channel);
      logToHost('error', `custom ${channel}: ${normalized.message}`);
      throw normalized;
    }
  }

  // Exhaustiveness guard.
  const _exhaustive: never = route;
  throw new Error(`[tauri-shim] unknown route kind: ${JSON.stringify(_exhaustive)}`);
}

function logToHost(level: 'error' | 'warn' | 'info' | 'debug', message: string): void {
  // Best-effort — never throw from the log path itself.
  tauriInvoke('app_log_renderer', { level, message }).catch(() => {});
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function shimEventSend(channel: string, data: unknown): void {
  // Renderer→main events are uncommon; most flows use invoke. Log so
  // missing translations are visible during development.
  // eslint-disable-next-line no-console
  console.debug(`[tauri-shim] eventSend (no-op): ${channel}`, data);
}

function shimEventOn(channel: string, cb: (data: unknown) => void): () => void {
  return registerEventListener(channel, cb);
}

function shimGetPathForFile(file: File): string {
  // Tauri webview does not expose absolute filesystem paths from
  // drop/drag File objects. Renderer call sites that need a real path
  // must move to `@tauri-apps/plugin-dialog` or use webkitRelativePath
  // when present.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (file as any).webkitRelativePath ?? '';
}

function normalizeTauriError(err: unknown, channel: string): Error {
  if (err instanceof Error) return err;
  if (err && typeof err === 'object') {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const obj = err as any;
    const code = obj.code ? String(obj.code) : 'tauri_error';
    const message = obj.message ? String(obj.message) : JSON.stringify(err);
    const wrapped = new Error(`[${channel}] [${code}] ${message}`);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (wrapped as any).code = code;
    return wrapped;
  }
  return new Error(`[${channel}] ${String(err)}`);
}

let installed = false;

export function installElectronApiPolyfill(): void {
  if (installed) return;
  installed = true;

  window.electronAPI = {
    invoke: shimInvoke,
    eventSend: shimEventSend,
    eventOn: shimEventOn,
    getPathForFile: shimGetPathForFile,
  };

  // Mirror uncaught errors + unhandled rejections to the host stderr
  // so headless dev runs surface renderer-side breakage even without
  // DevTools open.
  window.addEventListener('error', (event) => {
    const where = event.filename ? ` (${event.filename}:${event.lineno})` : '';
    logToHost('error', `uncaught: ${event.message}${where}`);
  });
  window.addEventListener('unhandledrejection', (event) => {
    const reason = event.reason;
    const message = reason instanceof Error ? reason.message : safeStringify(reason);
    logToHost('error', `unhandled rejection: ${message}`);
  });

  // Subscribe to the single Tauri UiMutationEvent channel and fan out
  // to the renderer's per-channel listeners. Safe to start before the
  // renderer mounts because the bridge buffers nothing — listeners
  // register lazily via shimEventOn.
  installEventBridge().catch((err) => {
    logToHost('error', `event bridge failed to install: ${String(err)}`);
    // eslint-disable-next-line no-console
    console.error('[tauri-shim] event bridge failed to install', err);
  });
}
