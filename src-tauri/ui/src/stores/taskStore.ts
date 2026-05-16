/**
 * MobX store for the workspaces of one project. One `WorkspaceStore` instance
 * per project; the parent renderer (e.g. a project detail view) is
 * responsible for `dispose()`ing it when the user navigates away —
 * `dispose()` is cheap (clear the array) and idempotent.
 *
 * State-guard conventions mirror `ProjectStore`: callers use the
 * `workspaceStoreKind` / `asReady` selectors instead of poking at the
 * variant fields directly.
 */
import { makeAutoObservable, runInAction } from 'mobx';
import {
  commands,
  type Workspace,
  type WorkspacePlacement,
  type WorkspacesCommandError_Serialize,
  type WorkspaceSourceBranch,
} from '../bindings';

export type WorkspacesStoreState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; workspaces: Workspace[] }
  | { kind: 'error'; error: WorkspacesCommandError_Serialize };

export class WorkspaceStore {
  state: WorkspacesStoreState = { kind: 'idle' };
  readonly projectId: string;

  constructor(projectId: string) {
    this.projectId = projectId;
    makeAutoObservable(this);
  }

  async load(): Promise<void> {
    if (this.state.kind === 'loading') return;
    this.state = { kind: 'loading' };
    const result = await commands.workspacesList(this.projectId);
    runInAction(() => {
      if (result.status === 'ok') {
        this.state = { kind: 'ready', workspaces: result.data };
      } else {
        this.state = { kind: 'error', error: result.error };
      }
    });
  }

  async create(
    name: string,
    sourceBranch: WorkspaceSourceBranch,
    workspaceBranch: string | null = null,
    placement: WorkspacePlacement = 'worktree',
    existingBranch: boolean = false
  ): Promise<Workspace | WorkspacesCommandError_Serialize> {
    const result = await commands.workspacesCreate(
      this.projectId,
      name,
      sourceBranch,
      workspaceBranch,
      placement,
      existingBranch
    );
    return result.status === 'ok' ? result.data : result.error;
  }

  async delete(id: string): Promise<null | WorkspacesCommandError_Serialize> {
    const result = await commands.workspacesDelete(id);
    return result.status === 'ok' ? null : result.error;
  }

  /**
   * Drop a workspace from the in-memory list. The bridge calls this on
   * `workspace_deleted` so the UI updates without a server round-trip.
   */
  applyDeleted(id: string): void {
    if (this.state.kind !== 'ready') return;
    this.state = {
      kind: 'ready',
      workspaces: this.state.workspaces.filter((t) => t.id !== id),
    };
  }
}

export function asReady(store: WorkspaceStore): { workspaces: Workspace[] } | undefined {
  return store.state.kind === 'ready' ? { workspaces: store.state.workspaces } : undefined;
}

export function workspaceStoreKind(store: WorkspaceStore): WorkspacesStoreState['kind'] {
  return store.state.kind;
}
