/**
 * Single dispatch helper for every `UiMutationEvent` variant.
 *
 * **Discipline contract** (also enforced by `eslint-plugin-emdash`):
 * this is the *only* place that turns a `UiMutationEvent` into a store
 * mutation. Renderer code never calls `listen(...)` from
 * `@tauri-apps/api/event` and the host never `app.emit(...)`s — every
 * cache-invalidation hop lands here. See
 * `docs/decisions/0004-ui-mutation-event-bridge.md` for the full
 * rationale.
 */
import type { UiMutationEvent } from '../bindings';
import type { RendererStores } from '../stores';

export type Stores = RendererStores;

export function dispatchUiMutation(stores: Stores, event: UiMutationEvent): void {
  switch (event.kind) {
    case 'project_created':
    case 'project_updated':
      // Host broadcasts only after a successful write, so re-load to pick up
      // the freshest row order/state. List is small in v1 — a full refresh
      // is the simplest correct dispatcher.
      void stores.projects.load();
      return;
    case 'project_deleted':
      stores.projects.applyDeleted(event.id);
      // A deleted project takes its tasks with it (ON DELETE CASCADE on the
      // DB side); drop the corresponding TaskStore so subsequent navigations
      // don't surface stale state.
      stores.tasksByProject.delete(event.id);
      return;
    case 'task_created':
    case 'task_updated': {
      const taskStore = stores.tasksByProject.get(event.project_id);
      if (taskStore) void taskStore.load();
      return;
    }
    case 'task_deleted': {
      const taskStore = stores.tasksByProject.get(event.project_id);
      taskStore?.applyDeleted(event.id);
      return;
    }
    case 'agent_hook_event':
      // EMD-9: classified agent-hook event. Renderer surface lands in a
      // follow-up; the dispatch arm exists today to keep the
      // exhaustiveness check honest.
      return;
    case 'github_identity_changed':
    case 'github_data_changed':
      // EMD-13: GitHub provider events. Cache-invalidation hooks for
      // identity / repo-scoped data are landed by the consumers (PR
      // list view, identity badge) as those views ship. For now the
      // arm exists to satisfy the exhaustiveness check.
      return;
    case 'linear_identity_changed':
    case 'linear_data_changed':
      // EMD-14: Linear provider events. Consumers (issue picker, board
      // sync indicator) will land cache invalidation here.
      return;
    case 'agent_started':
    case 'agent_exited':
      // EMD-27: Local agent lifecycle. The PTY xterm consumers receive
      // bytes via the dedicated Channel<Vec<u8>>; this arm is just for
      // store-level signals (e.g. agent status badge).
      return;
    case 'conversation_created':
    case 'conversation_updated':
    case 'conversation_deleted':
    case 'terminal_created':
    case 'terminal_updated':
    case 'terminal_deleted':
      // Per-task conversation / terminal CRUD. The renderer-side store
      // (Electron renderer) reloads via the `conversation.changed`
      // and `terminal.changed` event-bus topics that the shim emits;
      // the dispatch arm exists to keep the exhaustiveness check
      // honest until a Tauri-native task store consumes these.
      return;
  }
  // Exhaustiveness check: a new variant added to UiMutationEvent without a
  // case here makes this a compile error.
  const _exhaustive: never = event;
  void _exhaustive;
}
