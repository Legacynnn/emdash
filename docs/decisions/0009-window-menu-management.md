# 0009: Window + menu management (macOS chrome, drag-drop, known gaps)

## Status

Accepted

## Context

EMD-5 scaffolds a runnable webview window. EMD-20 makes it feel like
a native app — chrome, menu, window-state persistence, drag-drop —
without taking on more native shims than Tauri 2 provides out of the
box. Three forces shaped the decisions below:

- **Tauri 2's menu API is narrower than Electron's.** Some
  macOS-native integrations (Services submenu, Speech submenu) have
  no native Tauri 2 equivalent. Patching over with custom shims
  creates a long tail of platform-specific code; documenting the
  gaps openly lets us decide *if* a feature actually needs them.
- **The multi-window memory leak (tauri#5397).** Without an explicit
  teardown on `CloseRequested`, owned resources (PTY sessions, SSH
  pools, file watchers) outlive the window that started them. The
  fix is one line per resource at the `app.rs` boundary.
- **App icon work is packaging-shaped.** Iconset generation, dev vs
  prod tinting, and the platform-specific artifact path all belong
  with the rest of the bundling pipeline (EMD-22), not here. This
  ADR documents the deferment.

## Decision

### macOS chrome

`tauri.conf.json > app.windows[0]`:

- `titleBarStyle: "Overlay"` — title bar overlays the webview, so
  the renderer can paint behind it. Matches Electron emdash's inset
  chrome.
- `trafficLightPosition: { x: 18, y: 18 }` — inset traffic-light
  controls. Tauri 2.4+ native; no `tauri-plugin-decorum` needed for
  this alone.
- `minWidth: 640`, `minHeight: 400` — prevents the user from
  collapsing the window into an unusable strip.

### Window state persistence

`tauri-plugin-window-state` v2. Persists position + size + maximize
state across launches; restore is automatic at window-create time.
We don't write our own DB-backed implementation:

- Same data model and on-disk file location as upstream Helmor.
- One less surface to maintain.

The plugin writes to `<config_dir>/window-state.json`. Backup /
portable-mode support inherits whatever Tauri uses for that path.

### Application menu

Built in Rust at `src/app_menu.rs` via Tauri's `MenuBuilder` /
`SubmenuBuilder` / `PredefinedMenuItem`. Structure:

- **emdash - dev** — About, Hide, Hide Others, Show All, Quit
- **File** — Close Window
- **Edit** — Undo, Redo, Cut, Copy, Paste, Select All
- **View** — Toggle Full Screen
- **Window** — Minimize, Zoom, Close Window
- **Help** — placeholder (filled when docs URL is settled)

The Tauri menu API supports these `PredefinedMenuItem` roles
natively: `about`, `hide`, `hide_others`, `show_all`, `quit`,
`close_window`, `undo`, `redo`, `cut`, `copy`, `paste`, `select_all`,
`fullscreen`, `minimize`, `maximize`. That covers everything the
Electron build wired through Electron's menu role API.

### Known gaps (no native Tauri 2 support)

- **macOS Services submenu** — Tauri 2's `Menu` API doesn't expose a
  way to insert the Services proxy item. Workaround would be an
  Objective-C runtime shim; **deferred to v1.x** if a feature
  requires it. Most users won't notice; the Services entry is
  rarely interacted with.
- **macOS Speech submenu** — Same story. Deferred.
- **Dock badge** (per [tauri#4489]) — No native Tauri 2 API. Would
  require an Objective-C shim. **NOT in v1.** The renderer can show
  unread / pending state in-app. Re-evaluate if a feature lands
  whose value depends on dock-badge visibility (e.g. background
  agent completion).

[tauri#4489]: https://github.com/tauri-apps/tauri/issues/4489

### App icon

The window has the default Tauri icon today. Production iconset
generation (Apple `.icns`, Windows `.ico`, Linux PNG matrix) plus
dev / prod tinting (red-orange tint to distinguish a development
build from a release build on the dock) is **packaging-shaped work**
and lives in EMD-22. Until then the bundle is intentionally visually
distinct from any signed release — no risk of confusing a dev build
for a prod one.

### Drag-and-drop

`dragDropEnabled: false` in `tauri.conf.json`.

This flag is documented as misleadingly named: `false` means
"disable Tauri's native drag-drop handling and let the DOM see the
events." Which is what we want — the renderer can register HTML5
`dragover` / `drop` handlers on the projects panel and add a project
by drop.

`true` would route file drops through a Tauri-side handler that
suppresses the DOM event, which would break the in-renderer drop
target.

**Drag-out** (the inverse — dragging a file from the app to the
Finder) is NOT in v1. If a feature ever needs it, `tauri-plugin-dragout`
exists; we'll evaluate at that point.

### Memory hygiene on window close

`Builder::on_window_event` handles `WindowEvent::CloseRequested` by
calling `Registry::drain()` on the PTY registry. The hook is
extensible — SSH, watchers, agent-hook listeners get their drain
calls added at the same site as they land. The pattern is "explicit
teardown of every Send + Sync resource the window managed."

This mitigates the multi-window leak path documented in
[tauri-apps/tauri#5397]: without explicit drain, the resource handles
outlive the window's lifetime and the next window creates fresh
copies.

[tauri-apps/tauri#5397]: https://github.com/tauri-apps/tauri/issues/5397

## Consequences

### Easier

- Window state persists. A user who resizes the window once doesn't
  have to do it on every launch.
- The menu structure matches user expectations on macOS without us
  re-implementing macOS roles by hand.
- Drag-and-drop is a simple DOM contract — feature panels can listen
  to the events they care about without a Tauri-side handler.

### Harder

- The Services / Speech submenus and dock badge are documented gaps.
  Any feature spec that requires them needs to budget for a native
  shim (deferred).
- The default Tauri icon ships until EMD-22. Users may temporarily
  see a generic icon on the dock between this PR landing and EMD-22
  merging — acceptable, since the new product isn't yet announced.
- The window-state plugin writes to `<config_dir>/window-state.json`.
  This is one more file the user might want to delete for "factory
  reset"; document it in the recovery section of the eventual user
  manual.
