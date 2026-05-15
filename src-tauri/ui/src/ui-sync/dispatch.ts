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
import type { ProjectStore } from '../stores/projectStore';

export interface Stores {
  projects: ProjectStore;
}

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
      return;
  }
  // Exhaustiveness check: a new variant added to UiMutationEvent without a
  // case here makes this a compile error.
  const _exhaustive: never = event;
  void _exhaustive;
}
