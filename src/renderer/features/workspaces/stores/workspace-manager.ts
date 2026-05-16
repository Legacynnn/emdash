import { makeObservable, observable, reaction, runInAction, toJS } from 'mobx';
import { toast } from 'sonner';
import { prSyncProgressChannel, prUpdatedChannel } from '@shared/events/prEvents';
import {
  workspaceProvisionProgressChannel,
  workspaceStatusUpdatedChannel,
} from '@shared/events/workspaceEvents';
import type { WorkspaceViewSnapshot } from '@shared/view-state';
import type {
  CreateWorkspaceError,
  CreateWorkspaceParams,
  CreateWorkspaceWarning,
  Workspace,
  WorkspaceLifecycleStatus,
} from '@shared/workspaces';
import { getProjectManagerStore } from '@renderer/features/projects/stores/project-selectors';
import type { ProjectSettingsStore } from '@renderer/features/projects/stores/project-settings-store';
import type { RepositoryStore } from '@renderer/features/projects/stores/repository-store';
import { events, rpc } from '@renderer/lib/ipc';
import { viewStateCache } from '@renderer/lib/stores/view-state-cache';
import { log } from '@renderer/utils/logger';
import { conversationRegistry } from './conversation-registry';
import { infraRegistry } from './infra-registry';
import { terminalRegistry } from './terminal-registry';
import {
  createUnprovisionedTask,
  createUnregisteredTask,
  isProvisioned,
  isRegistered,
  isUnprovisioned,
  isUnregistered,
  type WorkspaceStore,
} from './workspace-store';

export async function markInitialConversationWorkingAfterProvision(
  task: WorkspaceStore | undefined,
  initialConversation: CreateWorkspaceParams['initialConversation']
): Promise<void> {
  if (!initialConversation?.initialPrompt?.trim()) return;
  if (!task || !isProvisioned(task)) return;
  try {
    const mgr = conversationRegistry.get(task.data.id);
    await mgr?.markConversationWorking(initialConversation.id);
  } catch (error) {
    log.warn('WorkspaceManagerStore: failed to mark initial conversation as working', {
      conversationId: initialConversation.id,
      workspaceId: initialConversation.workspaceId,
      error,
    });
  }
}

function formatCreateTaskError(error: CreateWorkspaceError): string {
  switch (error.type) {
    case 'project-not-found':
      return 'Project not found.';
    case 'initial-commit-required':
      return 'Create an initial commit to enable branch-based tasks.';
    case 'branch-create-failed': {
      switch (error.error.type) {
        case 'already_exists':
          return `Branch "${error.error.name}" already exists. Try a different task name.`;
        case 'invalid_base':
          return `Source branch "${error.error.from}" is not a valid base. Check that it exists locally or on the selected remote.`;
        case 'invalid_name':
          return `Branch "${error.error.name}" is not a valid branch name.`;
        default:
          return `Could not create branch "${error.branch}": ${error.error.message}`;
      }
    }
    case 'pr-fetch-failed':
      return error.error.type === 'not_found'
        ? `PR #${error.error.prNumber} was not found on remote "${error.remote}".`
        : `Could not fetch the pull request branch: ${error.error.message}`;
    case 'branch-not-found':
      return `Branch "${error.branch}" was not found locally or on the remote. Make sure the PR branch exists.`;
    case 'worktree-setup-failed':
      return error.message
        ? `Could not set up the worktree for branch "${error.branch}": ${error.message}`
        : `Could not set up the worktree for branch "${error.branch}".`;
    case 'provision-failed':
      return `Workspace could not be provisioned: ${error.message}`;
    case 'provision-timeout': {
      const seconds = Math.round(error.timeoutMs / 1000);
      const stepLabel = (() => {
        switch (error.step) {
          case 'resolving-worktree':
            return 'resolving the worktree';
          case 'initialising-workspace':
            return 'initialising the workspace';
          case 'running-provision-script':
            return 'running the provision script';
          case 'connecting':
            return 'connecting to the workspace';
          case 'setting-up-workspace':
            return 'setting up the workspace';
          case 'starting-sessions':
            return 'starting sessions';
          case null:
            return null;
        }
      })();
      return stepLabel
        ? `Workspace setup timed out after ${seconds}s while ${stepLabel}.`
        : `Workspace setup timed out after ${seconds}s before any step started.`;
    }
  }
}

function formatCreateTaskWarning(warning: CreateWorkspaceWarning): string {
  switch (warning.type) {
    case 'branch-publish-failed': {
      const detail =
        'message' in warning.error
          ? (warning.error.message ?? warning.error.type)
          : warning.error.type;
      return `Failed to publish branch "${warning.branch}" to "${warning.remote}": ${detail}`;
    }
  }
}

export class WorkspaceManagerStore {
  private readonly projectId: string;
  private readonly _repository: RepositoryStore;
  private readonly _settingsStore: ProjectSettingsStore;
  private readonly _baseRef: string;
  private _loadPromise: Promise<void> | null = null;
  private _teardownPromises = new Map<string, Promise<void>>();
  private _provisionPromises = new Map<string, Promise<void>>();

  private _unsubPrUpdated: (() => void) | null = null;
  private _unsubPrSyncProgress: (() => void) | null = null;
  private _unsubProvisionProgress: (() => void) | null = null;
  private _disposeRepositoryReaction: (() => void) | null = null;

  tasks = observable.map<string, WorkspaceStore>();

  constructor(
    projectId: string,
    repository: RepositoryStore,
    settingsStore: ProjectSettingsStore,
    baseRef: string
  ) {
    this.projectId = projectId;
    this._repository = repository;
    this._settingsStore = settingsStore;
    this._baseRef = baseRef;
    makeObservable(this, { tasks: observable });

    events.on(workspaceStatusUpdatedChannel, ({ workspaceId, projectId: evtProjectId, status }) => {
      if (evtProjectId !== this.projectId) return;
      const store = this.tasks.get(workspaceId);
      if (store && isProvisioned(store)) {
        runInAction(() => {
          store.data.status = status as WorkspaceLifecycleStatus;
        });
      }
    });

    this._unsubProvisionProgress = events.on(
      workspaceProvisionProgressChannel,
      ({ workspaceId, projectId: evtProjectId, message }) => {
        if (evtProjectId !== this.projectId) return;
        const store = this.tasks.get(workspaceId);
        if (store?.isBootstrapping) {
          runInAction(() => {
            store.provisionProgressMessage = message;
          });
        }
      }
    );

    this._unsubPrUpdated = events.on(prUpdatedChannel, ({ prs }) => {
      const repoUrl = this._repository.repositoryUrl;
      if (!repoUrl) return;
      for (const pr of prs) {
        if (pr.repositoryUrl !== repoUrl) continue;
        for (const [, store] of this.tasks) {
          if (!isRegistered(store)) continue;
          const task = store.data as Workspace;
          if (task.workspaceBranch !== pr.headRefName) continue;
          runInAction(() => {
            const idx = task.prs.findIndex((p) => p.url === pr.url);
            if (idx >= 0) {
              task.prs.splice(idx, 1, pr);
            } else {
              task.prs.push(pr);
            }
          });
        }
      }
    });

    this._unsubPrSyncProgress = events.on(prSyncProgressChannel, (progress) => {
      if (progress.status !== 'done') return;
      const repoUrl = this._repository.repositoryUrl;
      if (!repoUrl || progress.remoteUrl !== repoUrl) return;
      for (const [, store] of this.tasks) {
        if (isRegistered(store)) {
          void this._reloadPrsForTask(store);
        }
      }
    });

    this._disposeRepositoryReaction = reaction(
      () => this._repository.repositoryUrl,
      () => {
        for (const [, store] of this.tasks) {
          if (isRegistered(store)) {
            void this._reloadPrsForTask(store);
          }
        }
      }
    );
  }

  private async _reloadPrsForTask(store: WorkspaceStore): Promise<void> {
    if (!isRegistered(store)) return;
    const result = await rpc.pullRequests.getPullRequestsForWorkspace(
      this.projectId,
      store.data.id
    );
    if (!result.success) return;
    const prs = result.data.prs;
    runInAction(() => {
      if (isRegistered(store)) {
        (store.data as Workspace).prs = prs;
      }
    });
  }

  loadTasks(): Promise<void> {
    if (!this._loadPromise) {
      this._loadPromise = rpc.tasks
        .getTasks(this.projectId)
        .then((tasks) => {
          runInAction(() => {
            for (const t of tasks) {
              this.tasks.set(t.id, createUnprovisionedTask(t));
              // Acquire conversation and terminal managers for each registered task.
              conversationRegistry.acquire(t.id, this.projectId);
              terminalRegistry.acquire(t.id, this.projectId);
            }
          });
          const reloadPromises = tasks.flatMap((t) => {
            const store = this.tasks.get(t.id);
            return store && isRegistered(store) ? [this._reloadPrsForTask(store)] : [];
          });
          void Promise.all(reloadPromises);
        })
        .catch((e) => {
          console.error('Error loading tasks', e);
        });
    }
    return this._loadPromise;
  }

  async createTask(params: CreateWorkspaceParams) {
    runInAction(() => {
      this.tasks.set(
        params.id,
        createUnregisteredTask({
          id: params.id,
          lastInteractedAt: new Date().toISOString(),
          createdAt: new Date().toISOString(),
          name: params.name,
          status: params.initialStatus ?? 'in_progress',
          statusChangedAt: new Date().toISOString(),
          isPinned: false,
        })
      );
    });

    const sourceBranch = structuredClone(toJS(params.sourceBranch));

    const result = await rpc.workspaces
      .createWorkspace({ ...params, sourceBranch })
      .catch((e: unknown) => {
        const message = e instanceof Error ? e.message : String(e);
        runInAction(() => {
          const current = this.tasks.get(params.id);
          if (current && isUnregistered(current)) {
            current.phase = 'create-error';
            current.errorMessage = message;
          }
        });
        throw e;
      });

    if (!result.success) {
      const message = formatCreateTaskError(result.error);
      runInAction(() => {
        const current = this.tasks.get(params.id);
        if (current && isUnregistered(current)) {
          current.phase = 'create-error';
          current.errorMessage = message;
        }
      });
      throw new Error(message);
    }

    runInAction(() => {
      const current = this.tasks.get(params.id);
      if (current && isUnregistered(current)) {
        current.transitionToUnprovisioned(result.data.task, 'provision');
        // Acquire conversation and terminal managers on first registration.
        conversationRegistry.acquire(params.id, this.projectId);
        terminalRegistry.acquire(params.id, this.projectId);
      }
    });

    this._settingsStore.pageData.invalidate();

    if (result.data.warning) {
      toast.error(formatCreateTaskWarning(result.data.warning));
    }

    await this.provisionWorkspace(params.id);
    await markInitialConversationWorkingAfterProvision(
      this.tasks.get(params.id),
      params.initialConversation
    );
  }

  async provisionWorkspace(workspaceId: string): Promise<void> {
    await getProjectManagerStore().mountProject(this.projectId);
    await this.loadTasks();

    const inFlight = this._provisionPromises.get(workspaceId);
    if (inFlight) return inFlight;

    const task = this.tasks.get(workspaceId);
    if (!task || !isUnprovisioned(task)) return;

    runInAction(() => {
      task.phase = 'provision';
    });

    const promise = this._doProvision(workspaceId).finally(() => {
      this._provisionPromises.delete(workspaceId);
    });

    this._provisionPromises.set(workspaceId, promise);
    return promise;
  }

  private async _doProvision(workspaceId: string): Promise<void> {
    const task = this.tasks.get(workspaceId);
    if (!task || !isUnprovisioned(task)) return;

    let resolution: Awaited<ReturnType<typeof rpc.workspaces.resolveBootstrap>>;

    runInAction(() => {
      const current = this.tasks.get(workspaceId);
      if (current && isUnprovisioned(current)) {
        const wsId = (current.data as Workspace).workspaceId;
        if (wsId) {
          infraRegistry.setBootstrapState(this.projectId, wsId, { kind: 'resolving' });
        }
      }
    });

    try {
      resolution = await rpc.workspaces.resolveBootstrap({
        projectId: this.projectId,
        workspaceId,
      });
    } catch (err) {
      runInAction(() => {
        const current = this.tasks.get(workspaceId);
        if (current && isUnprovisioned(current)) {
          current.phase = 'provision-error';
          current.errorMessage = err instanceof Error ? err.message : String(err);
          const wsId = (current.data as Workspace).workspaceId;
          if (wsId) {
            infraRegistry.setBootstrapState(this.projectId, wsId, {
              kind: 'error',
              message: current.errorMessage,
            });
          }
        }
      });
      throw err;
    }

    if (resolution.kind === 'branch_elsewhere' || resolution.kind === 'path_missing') {
      runInAction(() => {
        const current = this.tasks.get(workspaceId);
        if (current && isUnprovisioned(current)) {
          const wsId = (current.data as Workspace).workspaceId;
          if (wsId) {
            infraRegistry.setBootstrapState(this.projectId, wsId, {
              kind: 'needs-resolution',
              resolution,
            });
          }
          // phase stays 'provision' — the registry holds the resolution details
        }
      });
      return;
    }

    if (resolution.kind === 'needs_create') {
      try {
        await rpc.workspaces.createWorktree({ projectId: this.projectId, workspaceId });
      } catch (err) {
        runInAction(() => {
          const current = this.tasks.get(workspaceId);
          if (current && isUnprovisioned(current)) {
            current.phase = 'provision-error';
            current.errorMessage = err instanceof Error ? err.message : String(err);
          }
        });
        throw err;
      }
    }

    await this._finishProvision(workspaceId);
  }

  async continueProvision(
    workspaceId: string,
    action: 'adopt' | 'create' | 'cancel',
    candidatePath?: string
  ): Promise<void> {
    const task = this.tasks.get(workspaceId);
    if (!task || !isUnprovisioned(task)) return;

    const wsId = (task.data as Workspace).workspaceId;

    if (action === 'cancel') {
      runInAction(() => {
        const current = this.tasks.get(workspaceId);
        if (current && isUnprovisioned(current)) {
          current.phase = 'idle';
          if (wsId) {
            infraRegistry.setBootstrapState(this.projectId, wsId, { kind: 'pending' });
          }
        }
      });
      return;
    }

    runInAction(() => {
      const current = this.tasks.get(workspaceId);
      if (current && isUnprovisioned(current)) {
        current.phase = 'provision';
        if (wsId) {
          infraRegistry.setBootstrapState(this.projectId, wsId, { kind: 'resolving' });
        }
      }
    });

    try {
      if (action === 'adopt') {
        if (!candidatePath) {
          throw new Error('adoptWorktree called without a candidatePath');
        }
        await rpc.workspaces.adoptWorktree({
          projectId: this.projectId,
          workspaceId,
          candidatePath,
        });
      } else if (action === 'create') {
        await rpc.workspaces.createWorktree({ projectId: this.projectId, workspaceId });
      }
    } catch (err) {
      runInAction(() => {
        const current = this.tasks.get(workspaceId);
        if (current && isUnprovisioned(current)) {
          current.phase = 'provision-error';
          current.errorMessage = err instanceof Error ? err.message : String(err);
        }
      });
      throw err;
    }

    await this._finishProvision(workspaceId);
  }

  private async _finishProvision(workspaceId: string): Promise<void> {
    const result = await rpc.workspaces.provisionWorkspace(workspaceId).catch((err: unknown) => {
      runInAction(() => {
        const current = this.tasks.get(workspaceId);
        if (current && isUnprovisioned(current)) {
          current.phase = 'provision-error';
          current.errorMessage = err instanceof Error ? err.message : String(err);
        }
      });
      throw err;
    });

    const savedSnapshot = (await viewStateCache.get(`task:${workspaceId}`)) as
      | WorkspaceViewSnapshot
      | undefined;

    runInAction(() => {
      const current = this.tasks.get(workspaceId);
      if (current && isUnprovisioned(current)) {
        if (savedSnapshot && current.viewModel) {
          current.viewModel.restoreSnapshot(savedSnapshot);
        }
        current.transitionToProvisioned(
          { ...current.data, lastInteractedAt: new Date().toISOString() },
          result.path,
          result.workspaceId,
          this._settingsStore,
          this._baseRef,
          result.sshConnectionId ?? undefined
        );
        current.activate();
      }
    });
  }

  async teardownTask(workspaceId: string): Promise<void> {
    const inFlight = this._teardownPromises.get(workspaceId);
    if (inFlight) return inFlight;

    const task = this.tasks.get(workspaceId);
    if (!task) return;

    runInAction(() => {
      const current = this.tasks.get(workspaceId);
      if (!current) return;
      if (isProvisioned(current)) {
        current.transitionToUnprovisioned({ ...current.data }, 'teardown');
      } else if (isUnprovisioned(current)) {
        current.phase = 'teardown';
      }
    });

    const promise = rpc.tasks
      .teardownTask(this.projectId, workspaceId)
      .then(() => {
        runInAction(() => {
          const current = this.tasks.get(workspaceId);
          if (current && isUnprovisioned(current)) {
            current.phase = 'idle';
          }
        });
      })
      .catch((err: unknown) => {
        runInAction(() => {
          const current = this.tasks.get(workspaceId);
          if (current && isUnprovisioned(current)) {
            current.phase = 'teardown-error';
          }
        });
        throw err;
      })
      .finally(() => {
        this._teardownPromises.delete(workspaceId);
      });

    this._teardownPromises.set(workspaceId, promise);
    return promise;
  }

  async setTaskPinned(workspaceId: string, isPinned: boolean): Promise<void> {
    const task = this.tasks.get(workspaceId);
    if (!task) return;
    await task.setPinned(isPinned);
  }

  async archiveWorkspace(workspaceId: string): Promise<void> {
    const currentTask = this.tasks.get(workspaceId);
    if (!currentTask || !isRegistered(currentTask)) return;
    const previousArchivedAt = currentTask.data.archivedAt;

    try {
      runInAction(() => {
        const task = this.tasks.get(workspaceId);
        if (task && isRegistered(task)) {
          task.data.archivedAt = new Date().toISOString();
        }
      });
      await rpc.workspaces.archiveWorkspace(this.projectId, workspaceId);
      void this.teardownTask(workspaceId).catch(() => {});
    } catch (e) {
      runInAction(() => {
        const task = this.tasks.get(workspaceId);
        if (task && isRegistered(task)) {
          task.data.archivedAt = previousArchivedAt;
        }
      });
      throw e;
    }
  }

  async restoreWorkspace(workspaceId: string): Promise<void> {
    const task = this.tasks.get(workspaceId);
    if (!task || !isRegistered(task)) return;
    const archivedAt = task.data.archivedAt;

    try {
      await rpc.workspaces.restoreWorkspace(workspaceId);
      runInAction(() => {
        const current = this.tasks.get(workspaceId);
        if (current && isRegistered(current)) {
          current.data.archivedAt = undefined;
        }
      });
    } catch (e) {
      runInAction(() => {
        const current = this.tasks.get(workspaceId);
        if (current && isRegistered(current)) {
          current.data.archivedAt = archivedAt;
        }
      });
      throw e;
    }
  }

  async deleteWorkspace(workspaceId: string): Promise<void> {
    const task = this.tasks.get(workspaceId);
    if (!task) return;

    runInAction(() => {
      this.tasks.delete(workspaceId);
    });

    try {
      // Release conversation and terminal registries before disposing the task.
      conversationRegistry.release(workspaceId);
      terminalRegistry.release(workspaceId);
      task.dispose();
      await rpc.workspaces.deleteWorkspace(this.projectId, workspaceId);
    } catch (e) {
      runInAction(() => {
        this.tasks.set(workspaceId, task);
      });
      throw e;
    }
  }

  dispose(): void {
    this._unsubPrUpdated?.();
    this._unsubPrUpdated = null;
    this._unsubPrSyncProgress?.();
    this._unsubPrSyncProgress = null;
    this._unsubProvisionProgress?.();
    this._unsubProvisionProgress = null;
    this._disposeRepositoryReaction?.();
    this._disposeRepositoryReaction = null;
  }
}
