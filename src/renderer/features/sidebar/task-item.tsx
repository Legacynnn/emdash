import { observer } from 'mobx-react-lite';
import { selectCurrentPr } from '@shared/pull-requests';
import { TaskSidebarAgentStatus } from '@renderer/features/sidebar/task-sidebar-agent-status';
import { WorkspaceContextMenu } from '@renderer/features/workspaces/components/workspace-context-menu';
import { WorkspaceGitDiffStats } from '@renderer/features/workspaces/components/workspace-git-diff-stats';
import {
  getTaskGitStore,
  getWorkspaceForTask,
  getWorkspaceManagerStore,
  getWorkspaceStore,
} from '@renderer/features/workspaces/stores/workspace-selectors';
import { type WorkspaceStore } from '@renderer/features/workspaces/stores/workspace-store';
import { useWorkspaceLayoutContext } from '@renderer/lib/layout/layout-provider';
import {
  useNavigate,
  useParams,
  useWorkspaceSlots,
} from '@renderer/lib/layout/navigation-provider';
import { useShowModal } from '@renderer/lib/modal/modal-provider';
import { cn } from '@renderer/utils/utils';
import { PrBadge } from '../../lib/components/pr-badge';
import { SidebarMenuRow } from './sidebar-primitives';

interface SidebarTaskItemProps {
  workspaceId: string;
  projectId: string;
  /** Pinned strip uses tighter padding than tasks nested under a project. */
  rowVariant?: 'underProject' | 'pinned';
}

export const SidebarTaskItem = observer(function SidebarTaskItem({
  workspaceId,
  projectId,
  rowVariant = 'underProject',
}: SidebarTaskItemProps) {
  const { navigate } = useNavigate();
  const { setCollapsed } = useWorkspaceLayoutContext();
  const showRename = useShowModal('renameWorkspaceModal');
  const showConfirm = useShowModal('confirmActionModal');

  const { currentView } = useWorkspaceSlots();
  const { params } = useParams('workspace');
  const isActive =
    currentView === 'workspace' &&
    params.workspaceId === workspaceId &&
    params.projectId === projectId;

  const task = getWorkspaceStore(projectId, workspaceId);
  const taskManager = getWorkspaceManagerStore(projectId);
  if (!task) return null;

  const isBootstrapping =
    task.state === 'unregistered' ||
    (task.state === 'unprovisioned' &&
      (task.phase === 'provision' || task.phase === 'provision-error'));

  const taskName = task.data.name;

  const handleProvision = () => {
    if (task.state !== 'unprovisioned' || task.phase !== 'idle') return;
    void taskManager?.provisionWorkspace(workspaceId);
  };

  const handleArchive = () => {
    if (isActive) navigate('project', { projectId });
    void taskManager?.archiveWorkspace(workspaceId);
  };

  const handleRename = () => showRename({ projectId, workspaceId, currentName: taskName });

  const handleDelete = () =>
    showConfirm({
      title: 'Delete task',
      description: `"${taskName}" will be permanently deleted. This action cannot be undone.`,
      confirmLabel: 'Delete',
      onSuccess: () => {
        void taskManager?.deleteWorkspace(workspaceId);
        if (isActive) navigate('project', { projectId });
      },
    });

  const canPin = task.state !== 'unregistered';

  const workspaceStore = getWorkspaceForTask(projectId, workspaceId);
  const git = getTaskGitStore(projectId, workspaceId);
  const branchName =
    git?.branchName ?? ('workspaceBranch' in task.data ? task.data.workspaceBranch : undefined);
  const handleReconnect =
    workspaceStore?.connectionState != null ? () => workspaceStore.reconnect() : undefined;

  return (
    <WorkspaceContextMenu
      isPinned={task.data.isPinned}
      canPin={canPin}
      isArchived={false}
      branchName={branchName}
      onPin={() => void task.setPinned(true)}
      onUnpin={() => void task.setPinned(false)}
      onRename={handleRename}
      onArchive={handleArchive}
      onReconnect={handleReconnect}
      onDelete={handleDelete}
    >
      <SidebarMenuRow
        className={cn(
          'group/row flex items-center justify-between px-1 h-8 gap-1',
          rowVariant === 'pinned' ? 'pl-2' : 'pl-8'
        )}
        isActive={isActive}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => {
          handleProvision();
          navigate('workspace', { projectId, workspaceId });
        }}
        onDoubleClick={() => setCollapsed('left', true)}
      >
        <div className="flex min-w-0 flex-1 items-center gap-1 self-stretch overflow-hidden">
          <span
            className={cn(
              'min-w-0 truncate text-left transition-colors',
              isBootstrapping && 'text-foreground/40'
            )}
          >
            {taskName}
          </span>
          <WorkspaceGitDiffStats
            task={task}
            className="h-full shrink-0 flex items-center pl-1 pr-1"
          />
          <RenderPrBadge task={task} />
        </div>
        <TaskSidebarAgentStatus task={task} />
      </SidebarMenuRow>
    </WorkspaceContextMenu>
  );
});

const RenderPrBadge = observer(function RenderPrBadge({ task }: { task: WorkspaceStore }) {
  if (!('prs' in task.data)) return null;
  const pr = selectCurrentPr(task.data.prs);
  return pr ? <PrBadge variant="compact" pr={pr} hoverDelay={100} /> : null;
});
