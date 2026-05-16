// Tauri UI entry. The product UI lives in `src/renderer/` and is shared
// with the Electron build; this shell only installs the
// `window.electronAPI` polyfill so the renderer can talk to Tauri
// commands via the same call sites it already uses for Electron IPC.
//
// Once the Electron tree is removed, the renderer source can move under
// `src-tauri/ui/` without any code changes — the polyfill becomes the
// permanent transport.

import { installElectronApiPolyfill } from './shim/electron-api';

// Install BEFORE importing the renderer entry. The renderer's top-level
// bootstrap reads `window.electronAPI` synchronously. Tailwind v4
// `@source` declarations now live inside `src/renderer/index.css`
// (the file that already hosts `@import 'tailwindcss';`) — the
// previous shim/tailwind-sources.css bridge was a no-op because
// Tailwind only honours `@source` from the CSS that imports it.
installElectronApiPolyfill();

// Side-effecting import: the renderer's main.tsx calls bootstrap() at
// module top level. Dynamic import keeps it out of this package's tsc
// scope so Tauri-side typecheck doesn't try to follow into the
// renderer's full type graph.
void import('@renderer/main');
