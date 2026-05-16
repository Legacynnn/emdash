import { observer } from 'mobx-react-lite';
import { createContext, useContext, type ReactNode } from 'react';
import { ProjectViewWrapper } from '@renderer/features/projects/components/project-view-wrapper';
import type { ConversationManagerStore } from '@renderer/features/workspaces/conversations/conversation-manager';
import type { DevServerStore } from '@renderer/features/workspaces/stores/dev-server-store';
import type { InfraStore } from '@renderer/features/workspaces/stores/infra';
import type { WorkspaceViewModel } from '@renderer/features/workspaces/stores/infra-view-model';
import {
  getConversationsForTask,
  getRegisteredTaskData,
  getTerminalsForTask,
  getWorkspaceForTask,
  getWorkspaceStore,
  workspaceViewKind,
  type WorkspaceViewKind,
} from '@renderer/features/workspaces/stores/workspace-selectors';
import type { TerminalManagerStore } from '@renderer/features/workspaces/terminals/terminal-manager';

interface TaskViewContext {
  projectId: string;
  workspaceId: string;
  /** The BYOI/infra handle for this workspace, or null when not yet registered. */
  infraId: string | null;
}

const TaskViewContext = createContext<TaskViewContext | null>(null);

export const TaskViewWrapper = observer(function TaskViewWrapper({
  children,
  projectId,
  workspaceId,
}: {
  children: ReactNode;
  projectId: string;
  workspaceId: string;
}) {
  const infraId = getRegisteredTaskData(projectId, workspaceId)?.workspaceId ?? null;
  return (
    <ProjectViewWrapper projectId={projectId}>
      <TaskViewContext.Provider value={{ projectId, workspaceId, infraId }}>
        {children}
      </TaskViewContext.Provider>
    </ProjectViewWrapper>
  );
});

export function useTaskViewContext(): TaskViewContext {
  const context = useContext(TaskViewContext);
  if (!context) {
    throw new Error('useTaskViewContext must be used within a TaskViewContextProvider');
  }
  return context;
}

export function useWorkspaceViewKind(): WorkspaceViewKind {
  const { projectId, workspaceId } = useTaskViewContext();
  return workspaceViewKind(getWorkspaceStore(projectId, workspaceId), projectId);
}

// ---------------------------------------------------------------------------
// Focused hooks (Phase 4)
// ---------------------------------------------------------------------------

/** Returns the active InfraStore. Throws if the task is not provisioned. */
export function useWorkspace(): InfraStore {
  const { projectId, workspaceId } = useTaskViewContext();
  const workspace = getWorkspaceForTask(projectId, workspaceId);
  if (!workspace) {
    throw new Error('useWorkspace: task is not provisioned (no workspace)');
  }
  return workspace;
}

/** Returns the BYOI/infra handle. Throws if the workspace is not provisioned. */
export function useInfraId(): string {
  const { infraId } = useTaskViewContext();
  if (!infraId) throw new Error('useInfraId: workspace is not provisioned');
  return infraId;
}

/** Returns the DevServerStore. Throws if the task is not provisioned. */
export function useDevServers(): DevServerStore {
  const { projectId, workspaceId } = useTaskViewContext();
  const devServers = getWorkspaceStore(projectId, workspaceId)?.viewModel?.devServers;
  if (!devServers) throw new Error('useDevServers: task is not provisioned');
  return devServers;
}

/** Returns the WorkspaceViewModel. Throws if the task is not registered. */
export function useWorkspaceViewModel(): WorkspaceViewModel {
  const { projectId, workspaceId } = useTaskViewContext();
  const viewModel = getWorkspaceStore(projectId, workspaceId)?.viewModel;
  if (!viewModel) {
    throw new Error('useWorkspaceViewModel: task is not registered (no view model)');
  }
  return viewModel;
}

/** Returns the ConversationManagerStore for the task. Throws if not registered. */
export function useConversations(): ConversationManagerStore {
  const { workspaceId } = useTaskViewContext();
  const mgr = getConversationsForTask(workspaceId);
  if (!mgr) {
    throw new Error('useConversations: task is not registered (no conversation manager)');
  }
  return mgr;
}

/** Returns the TerminalManagerStore for the task. Throws if not registered. */
export function useTerminals(): TerminalManagerStore {
  const { workspaceId } = useTaskViewContext();
  const mgr = getTerminalsForTask(workspaceId);
  if (!mgr) {
    throw new Error('useTerminals: task is not registered (no terminal manager)');
  }
  return mgr;
}
