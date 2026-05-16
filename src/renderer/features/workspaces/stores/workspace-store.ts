import { makeAutoObservable, observable, runInAction } from 'mobx';
import type { Issue, Workspace, WorkspaceLifecycleStatus } from '@shared/workspaces';
import type { ProjectSettingsStore } from '@renderer/features/projects/stores/project-settings-store';
import { DraftCommentsStore } from '@renderer/features/workspaces/diff-view/stores/draft-comments-store';
import { rpc } from '@renderer/lib/ipc';
import { log } from '@renderer/utils/logger';
import { conversationRegistry } from './conversation-registry';
import { infraRegistry } from './infra-registry';
import { WorkspaceViewModel } from './infra-view-model';

export type UnregisteredTaskPhase = 'creating' | 'create-error';

export type UnprovisionedTaskPhase =
  | 'provision'
  | 'provision-error'
  | 'teardown'
  | 'teardown-error'
  | 'idle';

export type UnregisteredTaskData = {
  id: string;
  name: string;
  status: WorkspaceLifecycleStatus;
  lastInteractedAt: string;
  createdAt: string;
  statusChangedAt: string;
  isPinned: boolean;
};

export class WorkspaceStore {
  state: 'unregistered' | 'unprovisioned' | 'provisioned';
  data: UnregisteredTaskData | Workspace;
  phase: UnregisteredTaskPhase | UnprovisionedTaskPhase | null;
  errorMessage: string | undefined = undefined;
  provisionProgressMessage: string | null = null;

  /** The workspace ID for this task session — null when unprovisioned. */
  workspaceId: string | null = null;
  /**
   * Stable view model — created when task first becomes registered, persists
   * across provision/unprovision cycles. Null only while task is unregistered.
   */
  viewModel: WorkspaceViewModel | null = null;
  /** Workspace-lifetime store for draft code-review comments. Null while unregistered. */
  draftComments: DraftCommentsStore | null = null;

  get displayName(): string {
    return this.data.name;
  }

  get isBootstrapping(): boolean {
    return (
      this.state === 'unregistered' ||
      (this.state === 'unprovisioned' &&
        (this.phase === 'provision' || this.phase === 'provision-error'))
    );
  }

  constructor(
    data: UnregisteredTaskData | Workspace,
    state: WorkspaceStore['state'],
    phase: UnregisteredTaskPhase | UnprovisionedTaskPhase | null = null
  ) {
    this.state = state;
    this.data = data;
    this.phase = phase;
    makeAutoObservable(this, {
      workspaceId: observable,
      viewModel: observable.ref,
      /** Deep observable so nested fields (e.g. `status`) notify observers (e.g. sidebar). */
      data: observable,
    });

    // Create stable task-lifetime stores immediately for registered tasks.
    if (state !== 'unregistered') {
      this._initRegisteredStores();
    }
  }

  private _initRegisteredStores(): void {
    const taskData = this.data as Workspace;
    this.draftComments = new DraftCommentsStore(taskData.id);
    this.viewModel = new WorkspaceViewModel(this);
  }

  transitionToProvisioned(
    data: Workspace,
    path: string,
    workspaceId: string,
    settingsStore: ProjectSettingsStore,
    baseRef: string,
    sshConnectionId?: string
  ): void {
    this.data = data;
    infraRegistry.acquire(
      data.projectId,
      workspaceId,
      path,
      settingsStore,
      baseRef,
      sshConnectionId
    );
    this.workspaceId = workspaceId;
    this.state = 'provisioned';
    this.phase = null;
    this.errorMessage = undefined;
    this.provisionProgressMessage = null;
    this.viewModel?.initialize();
  }

  transitionToUnprovisioned(data: Workspace, phase: UnprovisionedTaskPhase = 'idle'): void {
    this.viewModel?.suspend();
    if (this.workspaceId) {
      infraRegistry.release(data.projectId, this.workspaceId);
      this.workspaceId = null;
    }
    this.data = data;
    this.state = 'unprovisioned';
    this.phase = phase;
    this.errorMessage = undefined;
    this.provisionProgressMessage = null;

    // Create stable stores on first registration (when transitioning from unregistered).
    if (!this.draftComments) this._initRegisteredStores();
  }

  transitionToUnregistered(data: UnregisteredTaskData): void {
    this.viewModel?.suspend();
    if (this.workspaceId) {
      const projectId = (this.data as Workspace).projectId;
      infraRegistry.release(projectId, this.workspaceId);
      this.workspaceId = null;
    }
    this.data = data;
    this.state = 'unregistered';
    this.phase = 'creating';
    this.errorMessage = undefined;
  }

  activate(): void {
    if (this.workspaceId) {
      const projectId = (this.data as Workspace).projectId;
      infraRegistry.activate(projectId, this.workspaceId);
    }
  }

  dispose(): void {
    this.viewModel?.dispose();
    this.viewModel = null;
    if (this.workspaceId) {
      const projectId = (this.data as Workspace).projectId;
      infraRegistry.release(projectId, this.workspaceId);
      this.workspaceId = null;
    }
    this.draftComments?.dispose();
    this.draftComments = null;
  }

  get conversationStats(): Record<string, number> {
    if (this.state === 'unregistered') {
      return {};
    }
    if (this.state === 'provisioned') {
      const mgr = conversationRegistry.get(this.data.id);
      if (mgr) {
        const counts: Record<string, number> = {};
        for (const conv of mgr.conversations.values()) {
          const id = conv.data.providerId;
          counts[id] = (counts[id] ?? 0) + 1;
        }
        return counts;
      }
    }
    return (this.data as Workspace).conversations;
  }

  async rename(name: string): Promise<void> {
    if (this.state !== 'provisioned') return;
    const task = registeredTaskData(this);
    if (!task) return;
    try {
      await rpc.workspaces.renameWorkspace(task.projectId, task.id, name);
      runInAction(() => {
        this.data.name = name;
      });
    } catch (e) {
      runInAction(() => {
        this.data.name = task.name;
      });
      log.error(e);
      throw e;
    }
  }

  async updateStatus(status: WorkspaceLifecycleStatus): Promise<void> {
    const previousStatus = this.data.status;
    const previousStatusChangedAt = this.data.statusChangedAt;
    const nextChangedAt = new Date().toISOString();
    runInAction(() => {
      this.data.status = status;
      this.data.statusChangedAt = nextChangedAt;
    });
    try {
      await rpc.workspaces.updateWorkspaceStatus(this.data.id, status);
    } catch (e) {
      runInAction(() => {
        this.data.status = previousStatus;
        this.data.statusChangedAt = previousStatusChangedAt;
      });
      log.error(e);
      throw e;
    }
  }

  async setPinned(isPinned: boolean): Promise<void> {
    if (this.state === 'unregistered') return;
    const task = registeredTaskData(this);
    if (!task) return;
    const previous = task.isPinned;
    runInAction(() => {
      task.isPinned = isPinned;
    });
    try {
      await rpc.workspaces.setWorkspacePinned(task.id, isPinned);
    } catch (e) {
      runInAction(() => {
        task.isPinned = previous;
      });
      log.error(e);
      throw e;
    }
  }

  async updateLinkedIssue(issue?: Issue): Promise<void> {
    if (this.state === 'unregistered') return;
    const task = registeredTaskData(this);
    if (!task) return;
    const previousIssue = task.linkedIssue;
    try {
      await rpc.workspaces.updateLinkedIssue(task.id, issue);
      runInAction(() => {
        task.linkedIssue = issue;
      });
    } catch (e) {
      runInAction(() => {
        task.linkedIssue = previousIssue;
      });
      console.error(e);
      throw e;
    }
  }
}

export type UnregisteredTask = WorkspaceStore & {
  state: 'unregistered';
  data: UnregisteredTaskData;
  phase: UnregisteredTaskPhase;
  errorMessage: string | undefined;
};

export type UnprovisionedTask = WorkspaceStore & {
  state: 'unprovisioned';
  data: Workspace;
  phase: UnprovisionedTaskPhase;
  errorMessage: string | undefined;
};

export function isUnregistered(t: WorkspaceStore): t is UnregisteredTask {
  return t.state === 'unregistered';
}

export function isRegistered(
  t: WorkspaceStore
): t is WorkspaceStore & { state: 'unprovisioned' | 'provisioned'; data: Workspace } {
  return t.state !== 'unregistered';
}

export function isUnprovisioned(t: WorkspaceStore): t is UnprovisionedTask {
  return t.state === 'unprovisioned';
}

export function isProvisioned(
  t: WorkspaceStore
): t is WorkspaceStore & { state: 'provisioned'; data: Workspace; workspaceId: string } {
  return t.state === 'provisioned';
}

/** Full `Workspace` payload when registered (unprovisioned or provisioned); `undefined` when unregistered. */
export function registeredTaskData(store: WorkspaceStore): Workspace | undefined {
  return isRegistered(store) ? store.data : undefined;
}

export function unregisteredTaskData(store: WorkspaceStore): UnregisteredTaskData | undefined {
  return isUnregistered(store) ? store.data : undefined;
}

export function createUnregisteredTask(data: UnregisteredTaskData): WorkspaceStore {
  return new WorkspaceStore(data, 'unregistered', 'creating');
}

export function createUnprovisionedTask(data: Workspace): WorkspaceStore {
  return new WorkspaceStore(data, 'unprovisioned', 'idle');
}
