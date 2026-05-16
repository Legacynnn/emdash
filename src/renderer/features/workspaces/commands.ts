import { TASK_COMMAND_DEFS, type CommandDef, type TaskCommandId } from '@shared/commands';
import {
  getRegisteredTaskData,
  getTaskGitStore,
  getWorkspaceStore,
  getWorkspaceView,
} from '@renderer/features/workspaces/stores/workspace-selectors';
import type { CommandProvider } from '@renderer/lib/commands/types';
import { showModal } from '@renderer/lib/modal/modal-provider';
import { appState, sidebarStore } from '@renderer/lib/stores/app-state';

function taskDef(id: TaskCommandId): CommandDef {
  return TASK_COMMAND_DEFS.find((d) => d.id === id)!;
}

/**
 * Returns a CommandProvider for the task scope.
 *
 * getCommands() reads MobX observables so the command registry's
 * @computed activeCommands reacts to state changes automatically.
 */
export function createTaskCommandProvider(projectId: string, workspaceId: string): CommandProvider {
  return {
    scopeId: 'task',

    getCommands() {
      const taskStore = getWorkspaceStore(projectId, workspaceId);

      // Guard: only expose commands when the task is fully provisioned.
      if (taskStore?.state !== 'provisioned') return [];

      const workspaceView = getWorkspaceView(projectId, workspaceId);
      const tabManager = workspaceView?.tabManager;

      const taskIds = sidebarStore.visibleTaskIdsForProject(projectId);
      const currentIdx = taskIds.indexOf(workspaceId);

      const git = getTaskGitStore(projectId, workspaceId);
      const taskData = getRegisteredTaskData(projectId, workspaceId);

      const newConversationDef = taskDef('task.newConversation');
      const sidebarChangesDef = taskDef('task.sidebarChanges');
      const sidebarConversationsDef = taskDef('task.sidebarConversations');
      const sidebarFilesDef = taskDef('task.sidebarFiles');
      const viewTerminalsDef = taskDef('task.viewTerminals');
      const toggleTerminalDrawerDef = taskDef('task.toggleTerminalDrawer');
      const toggleRightSidebarDef = taskDef('task.toggleRightSidebar');
      const newTerminalDef = taskDef('task.newTerminal');
      const gitFetchDef = taskDef('task.gitFetch');
      const gitPullDef = taskDef('task.gitPull');
      const gitPushDef = taskDef('task.gitPush');
      const pinDef = taskDef('task.pin');
      const nextTaskDef = taskDef('task.nextTask');
      const prevTaskDef = taskDef('task.prevTask');

      return [
        // ── Conversations ──────────────────────────────────────────────────
        {
          id: newConversationDef.id,
          label: newConversationDef.label,
          description: newConversationDef.description,
          shortcutKey: newConversationDef.shortcutKey,
          group: newConversationDef.group,
          execute() {
            showModal('createConversationModal', {
              projectId,
              workspaceId,
              onSuccess: ({ conversationId }) => {
                tabManager?.openConversation(conversationId);
                workspaceView?.setFocusedRegion('main');
              },
            });
          },
        },

        // ── View sidebar panels ────────────────────────────────────────────
        {
          id: sidebarChangesDef.id,
          label: sidebarChangesDef.label,
          description: sidebarChangesDef.description,
          shortcutKey: sidebarChangesDef.shortcutKey,
          group: sidebarChangesDef.group,
          execute() {
            workspaceView?.setSidebarTab('changes');
            workspaceView?.setSidebarCollapsed(false);
          },
        },
        {
          id: sidebarConversationsDef.id,
          label: sidebarConversationsDef.label,
          description: sidebarConversationsDef.description,
          shortcutKey: sidebarConversationsDef.shortcutKey,
          group: sidebarConversationsDef.group,
          execute() {
            workspaceView?.setSidebarTab('conversations');
            workspaceView?.setSidebarCollapsed(false);
          },
        },
        {
          id: sidebarFilesDef.id,
          label: sidebarFilesDef.label,
          description: sidebarFilesDef.description,
          shortcutKey: sidebarFilesDef.shortcutKey,
          group: sidebarFilesDef.group,
          execute() {
            workspaceView?.setSidebarTab('files');
            workspaceView?.setSidebarCollapsed(false);
          },
        },
        {
          id: viewTerminalsDef.id,
          label: viewTerminalsDef.label,
          description: viewTerminalsDef.description,
          group: viewTerminalsDef.group,
          execute() {
            workspaceView?.setTerminalDrawerOpen(true);
          },
        },

        // ── Layout toggles ─────────────────────────────────────────────────
        {
          id: toggleTerminalDrawerDef.id,
          label: toggleTerminalDrawerDef.label,
          description: toggleTerminalDrawerDef.description,
          shortcutKey: toggleTerminalDrawerDef.shortcutKey,
          group: toggleTerminalDrawerDef.group,
          execute() {
            workspaceView?.setTerminalDrawerOpen(!workspaceView.isTerminalDrawerOpen);
          },
        },
        {
          id: toggleRightSidebarDef.id,
          // Dynamic label reflecting current collapsed/expanded state
          label: workspaceView?.isSidebarCollapsed ? 'Show Right Sidebar' : 'Hide Right Sidebar',
          description: toggleRightSidebarDef.description,
          shortcutKey: toggleRightSidebarDef.shortcutKey,
          group: toggleRightSidebarDef.group,
          execute() {
            workspaceView?.setSidebarCollapsed(!workspaceView.isSidebarCollapsed);
          },
        },

        // ── Terminals ─────────────────────────────────────────────────────
        {
          id: newTerminalDef.id,
          label: newTerminalDef.label,
          description: newTerminalDef.description,
          shortcutKey: newTerminalDef.shortcutKey,
          group: newTerminalDef.group,
          execute() {
            workspaceView?.openNewTerminal();
          },
        },

        // ── Git ────────────────────────────────────────────────────────────
        {
          id: gitFetchDef.id,
          label: gitFetchDef.label,
          description: gitFetchDef.description,
          group: gitFetchDef.group,
          enabled: git != null,
          execute() {
            void git?.fetchRemote();
          },
        },
        {
          id: gitPullDef.id,
          label: gitPullDef.label,
          description: gitPullDef.description,
          group: gitPullDef.group,
          enabled: git != null,
          execute() {
            void git?.pull();
          },
        },
        {
          id: gitPushDef.id,
          // Dynamic label: push vs publish branch
          label: git?.isBranchPublished ? 'Git Push' : 'Git Publish Branch',
          description: git?.isBranchPublished
            ? 'Push commits to remote'
            : 'Publish this branch to remote',
          group: gitPushDef.group,
          enabled: git != null,
          execute() {
            if (git?.isBranchPublished) {
              void git.push();
            } else {
              void git?.publishBranch();
            }
          },
        },

        // ── Workspace actions ───────────────────────────────────────────────────
        {
          id: pinDef.id,
          // Dynamic label: pin vs unpin
          label: taskData?.isPinned ? 'Unpin Workspace' : 'Pin Workspace',
          description: taskData?.isPinned
            ? 'Remove this task from pinned'
            : 'Pin this task to keep it at the top',
          group: pinDef.group,
          enabled: taskData != null,
          execute() {
            if (taskData) void taskStore?.setPinned(!taskData.isPinned);
          },
        },

        // ── Navigation ─────────────────────────────────────────────────────
        {
          id: nextTaskDef.id,
          label: nextTaskDef.label,
          description: nextTaskDef.description,
          group: nextTaskDef.group,
          enabled: currentIdx !== -1 && currentIdx < taskIds.length - 1,
          hideFromPalette: true,
          execute() {
            const nextId = taskIds[currentIdx + 1];
            if (nextId)
              appState.navigation.navigate('workspace', { projectId, workspaceId: nextId });
          },
        },
        {
          id: prevTaskDef.id,
          label: prevTaskDef.label,
          description: prevTaskDef.description,
          group: prevTaskDef.group,
          enabled: currentIdx > 0,
          hideFromPalette: true,
          execute() {
            const prevId = taskIds[currentIdx - 1];
            if (prevId)
              appState.navigation.navigate('workspace', { projectId, workspaceId: prevId });
          },
        },
      ];
    },
  };
}
