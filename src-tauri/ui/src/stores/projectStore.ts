/**
 * MobX store for the projects list. Drives the renderer state-guard
 * pattern described in the root `AGENTS.md`: callers route through
 * `getProjectStore` / `asReady` and never reach for internal fields
 * directly.
 */
import { makeAutoObservable, runInAction } from 'mobx';
import { commands, type Project, type ProjectsCommandError } from '../bindings';

export type ProjectsStoreState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; projects: Project[] }
  | { kind: 'error'; error: ProjectsCommandError };

export class ProjectStore {
  state: ProjectsStoreState = { kind: 'idle' };

  constructor() {
    makeAutoObservable(this);
  }

  async load(): Promise<void> {
    if (this.state.kind === 'loading') return;
    this.state = { kind: 'loading' };
    const result = await commands.projectsList();
    runInAction(() => {
      if (result.status === 'ok') {
        this.state = { kind: 'ready', projects: result.data };
      } else {
        this.state = { kind: 'error', error: result.error };
      }
    });
  }

  /**
   * Optimistic-free add: the renderer waits for the server response and
   * the broadcast `ProjectCreated` event triggers a re-load (the bridge
   * is the source of truth for cache invalidation). Returns the new
   * project on success.
   */
  async add(path: string): Promise<Project | ProjectsCommandError> {
    const result = await commands.projectsAdd(path);
    if (result.status === 'ok') {
      return result.data;
    }
    return result.error;
  }

  async remove(id: string): Promise<null | ProjectsCommandError> {
    const result = await commands.projectsRemove(id);
    if (result.status === 'ok') return null;
    return result.error;
  }

  /**
   * Drop an entry from the local list without re-fetching. The bridge
   * calls this when a `project_deleted` mutation arrives so the UI
   * updates immediately without a network round-trip.
   */
  applyDeleted(id: string): void {
    if (this.state.kind !== 'ready') return;
    this.state = {
      kind: 'ready',
      projects: this.state.projects.filter((p) => p.id !== id),
    };
  }
}

// State-guard selectors. Keep these pure: safe in observer components,
// effects, and event handlers.

export function asReady(store: ProjectStore): { projects: Project[] } | undefined {
  return store.state.kind === 'ready' ? { projects: store.state.projects } : undefined;
}

export function projectStoreKind(store: ProjectStore): ProjectsStoreState['kind'] {
  return store.state.kind;
}
