import type { Workspace } from '@shared/workspaces';
import { isUnmountedProject } from '@renderer/features/projects/stores/project';
import { getProjectManagerStore } from '@renderer/features/projects/stores/project-selectors';
import type { AgentStatus } from '@renderer/features/workspaces/conversations/conversation-manager';
import type { DiffViewStore } from '@renderer/features/workspaces/diff-view/stores/diff-view-store';
import type { FileModelLifecycleStore } from '@renderer/features/workspaces/editor/stores/file-model-lifecycle-store';
import { conversationRegistry } from './conversation-registry';
import { infraRegistry } from './infra-registry';
import type { WorkspaceViewModel } from './infra-view-model';
import { terminalRegistry } from './terminal-registry';
import type { WorkspaceManagerStore } from './workspace-manager';
import {
  isProvisioned,
  isRegistered,
  isUnprovisioned,
  isUnregistered,
  registeredTaskData,
  type WorkspaceStore,
} from './workspace-store';

/** Call only inside `observer` components (or other MobX reactions). */
export function getWorkspaceManagerStore(projectId: string): WorkspaceManagerStore | undefined {
  const p = getProjectManagerStore().projects.get(projectId);
  return p?.mountedProject?.taskManager;
}

/** Call only inside `observer` components (or other MobX reactions). */
export function getWorkspaceStore(
  projectId: string,
  workspaceId: string
): WorkspaceStore | undefined {
  return getWorkspaceManagerStore(projectId)?.tasks.get(workspaceId);
}

/** Registered task payload (`Workspace`) when the row exists and is not unregistered; otherwise undefined. */
export function getRegisteredTaskData(
  projectId: string,
  workspaceId: string
): Workspace | undefined {
  const store = getWorkspaceStore(projectId, workspaceId);
  if (!store) return undefined;
  return registeredTaskData(store);
}

/** Call only inside `observer` components (or other MobX reactions). */
export function getWorkspaceView(
  projectId: string,
  workspaceId: string
): WorkspaceViewModel | undefined {
  return getWorkspaceStore(projectId, workspaceId)?.viewModel ?? undefined;
}

/** Call only inside `observer` components (or other MobX reactions). */
export function getEditorView(
  projectId: string,
  workspaceId: string
): FileModelLifecycleStore | undefined {
  return getWorkspaceView(projectId, workspaceId)?.editorView;
}

/** Call only inside `observer` components (or other MobX reactions). */
export function getDiffView(projectId: string, workspaceId: string): DiffViewStore | undefined {
  return getWorkspaceView(projectId, workspaceId)?.diffView ?? undefined;
}

export function getTaskGitStore(projectId: string, workspaceId: string) {
  const store = getWorkspaceStore(projectId, workspaceId);
  if (!store?.workspaceId) return undefined;
  return infraRegistry.get(projectId, store.workspaceId)?.git;
}

export function taskAgentStatus(store: WorkspaceStore): AgentStatus | null {
  const mgr = conversationRegistry.get(store.data.id);
  return mgr?.taskStatus ?? null;
}

export type WorkspaceViewKind =
  | 'missing'
  | 'project-mounting' // project is still opening — task data not yet available
  | 'project-error' // project failed to open
  | 'creating'
  | 'create-error'
  | 'provisioning'
  | 'provision-error'
  | 'teardown'
  | 'teardown-error'
  | 'idle'
  | 'needs-resolution'
  | 'ready';

/**
 * Derives the task view kind from the project + task store state.
 *
 * Pass `projectId` so that "project still opening" can be distinguished from
 * "task genuinely missing". Call only inside `observer` components.
 */
export function workspaceViewKind(
  store: WorkspaceStore | undefined,
  projectId: string
): WorkspaceViewKind {
  const projectStore = getProjectManagerStore().projects.get(projectId);

  if (!projectStore) return 'missing';

  if (isUnmountedProject(projectStore)) {
    if (projectStore.phase === 'opening') return 'project-mounting';
    if (projectStore.phase === 'error') return 'project-error';
    return 'project-mounting';
  }

  if (projectStore.state === 'unregistered') return 'missing';

  if (!store) return 'missing';

  if (isUnregistered(store)) {
    if (store.phase === 'creating') return 'creating';
    return 'create-error';
  }
  if (isUnprovisioned(store)) {
    if (store.phase === 'provision') {
      const wsId = isRegistered(store) ? (store.data as Workspace).workspaceId : null;
      if (wsId) {
        const bs = infraRegistry.bootstrapStateFor(projectId, wsId);
        if (bs?.kind === 'needs-resolution') return 'needs-resolution';
      }
      return 'provisioning';
    }
    if (store.phase === 'provision-error') return 'provision-error';
    if (store.phase === 'teardown') return 'teardown';
    if (store.phase === 'teardown-error') return 'teardown-error';
    return 'idle';
  }
  return 'ready';
}

/** Returns the narrowed provisioned task store if the task is provisioned, otherwise undefined. */
export function asProvisioned(
  store: WorkspaceStore | undefined
): (WorkspaceStore & { state: 'provisioned'; workspaceId: string }) | undefined {
  return store && isProvisioned(store) ? store : undefined;
}

// ---------------------------------------------------------------------------
// New focused selectors (Phase 4)
// ---------------------------------------------------------------------------

export function getWorkspaceForTask(projectId: string, workspaceId: string) {
  const wsId = getWorkspaceStore(projectId, workspaceId)?.workspaceId;
  return wsId ? (infraRegistry.get(projectId, wsId) ?? undefined) : undefined;
}

export function getWorkspaceViewModel(
  projectId: string,
  workspaceId: string
): WorkspaceViewModel | undefined {
  return getWorkspaceStore(projectId, workspaceId)?.viewModel ?? undefined;
}

export function getConversationsForTask(workspaceId: string) {
  return conversationRegistry.get(workspaceId);
}

export function getTerminalsForTask(workspaceId: string) {
  return terminalRegistry.get(workspaceId);
}

/** Returns the display name from any task store variant. */
export function taskDisplayName(store: WorkspaceStore | undefined): string | undefined {
  if (!store) return undefined;
  return store.data.name;
}

/** Returns the error message for error states. */
export function taskErrorMessage(store: WorkspaceStore | undefined): string | undefined {
  if (!store) return undefined;
  if (isUnregistered(store) && store.phase === 'create-error') {
    return store.errorMessage ?? 'Failed to create task';
  }
  if (isUnprovisioned(store)) {
    if (store.phase === 'provision-error') {
      return store.errorMessage ?? 'Failed to set up workspace';
    }
    if (store.phase === 'teardown-error') {
      return store.errorMessage ?? 'Failed to tear down task';
    }
  }
  return undefined;
}

/** Returns the mount error message for the project. */
export function projectMountErrorMessage(projectId: string): string {
  const store = getProjectManagerStore().projects.get(projectId);
  if (store && isUnmountedProject(store) && store.phase === 'error') {
    return store.error ?? 'Failed to open project';
  }
  return 'Failed to open project';
}
