import { observer } from 'mobx-react-lite';
import { useEffect, type ReactNode } from 'react';
import { type ViewDefinition } from '@renderer/app/view-registry';
import {
  getWorkspaceManagerStore,
  getWorkspaceStore,
  workspaceViewKind,
} from '@renderer/features/workspaces/stores/workspace-selectors';
import {
  TaskViewWrapper,
  useWorkspaceViewModel,
} from '@renderer/features/workspaces/workspace-view-context';
import { createTaskCommandProvider } from './commands';
import { EditorProvider } from './editor/editor-provider';
import { useIsActiveTask } from './hooks/use-is-active-workspace';
import { TaskMainPanel } from './main-panel';
import { WorkspaceTitlebar } from './workspace-titlebar';

/**
 * Syncs TabManagerStore.isVisible with the active task state.
 * Controls telemetry conversation scope.
 */
const TabManagerVisibilitySync = observer(function TabManagerVisibilitySync({
  workspaceId,
}: {
  workspaceId: string;
}) {
  const workspaceView = useWorkspaceViewModel();
  const isActive = useIsActiveTask(workspaceId);

  useEffect(() => {
    workspaceView.tabManager.setVisible(isActive);
    return () => {
      workspaceView.tabManager.setVisible(false);
    };
  }, [workspaceView.tabManager, isActive]);

  return null;
});

const TaskViewWrapperWithProviders = observer(function TaskViewWrapperWithProviders({
  children,
  projectId,
  workspaceId,
}: {
  children: ReactNode;
  projectId: string;
  workspaceId: string;
}) {
  const taskStore = getWorkspaceStore(projectId, workspaceId);
  const kind = workspaceViewKind(taskStore, projectId);

  // Auto-provision when the task view is rendered with an idle task — covers
  // session restore where the task wasn't in openTaskIds, direct navigation,
  // and any other path that lands on the task view before provisioning runs.
  useEffect(() => {
    if (kind !== 'idle') return;
    if (taskStore && 'archivedAt' in taskStore.data && taskStore.data.archivedAt) return;

    getWorkspaceManagerStore(projectId)
      ?.provisionWorkspace(workspaceId)
      .catch(() => {});
  }, [kind, projectId, workspaceId, taskStore]);

  if (kind !== 'ready') {
    return (
      <TaskViewWrapper projectId={projectId} workspaceId={workspaceId}>
        {children}
      </TaskViewWrapper>
    );
  }

  return (
    <TaskViewWrapper projectId={projectId} workspaceId={workspaceId}>
      <TabManagerVisibilitySync workspaceId={workspaceId} />
      <EditorProvider key={workspaceId} workspaceId={workspaceId} projectId={projectId}>
        {children}
      </EditorProvider>
    </TaskViewWrapper>
  );
});

export const workspaceView = {
  WrapView: TaskViewWrapperWithProviders,
  TitlebarSlot: WorkspaceTitlebar,
  MainPanel: TaskMainPanel,
  commandProvider: ({ projectId, workspaceId }: { projectId: string; workspaceId: string }) =>
    createTaskCommandProvider(projectId, workspaceId),
} satisfies ViewDefinition<{ projectId: string; workspaceId: string }>;
