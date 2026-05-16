import { observer } from 'mobx-react-lite';
import { selectCurrentPr } from '@shared/pull-requests';
import { type Workspace } from '@shared/workspaces';
import { AgentStatusIndicator } from '@renderer/features/workspaces/components/agent-status-indicator';
import { WorkspaceContextMenu } from '@renderer/features/workspaces/components/workspace-context-menu';
import { WorkspaceGitDiffStats } from '@renderer/features/workspaces/components/workspace-git-diff-stats';
import {
  getTaskGitStore,
  getWorkspaceManagerStore,
  taskAgentStatus,
} from '@renderer/features/workspaces/stores/workspace-selectors';
import { type WorkspaceStore } from '@renderer/features/workspaces/stores/workspace-store';
import AgentLogo from '@renderer/lib/components/agent-logo';
import { PrBadge } from '@renderer/lib/components/pr-badge';
import { useNavigate } from '@renderer/lib/layout/navigation-provider';
import { useShowModal } from '@renderer/lib/modal/modal-provider';
import { Checkbox } from '@renderer/lib/ui/checkbox';
import { RelativeTime } from '@renderer/lib/ui/relative-time';
import { agentConfig } from '@renderer/utils/agentConfig';
import { cn } from '@renderer/utils/utils';

export type ReadyTask = WorkspaceStore & { data: Workspace };

export const TaskRow = observer(function TaskRow({
  task,
  isSelected,
  onToggleSelect,
}: {
  task: ReadyTask;
  isSelected: boolean;
  onToggleSelect: () => void;
}) {
  const { navigate } = useNavigate();
  const showRename = useShowModal('renameWorkspaceModal');
  const showConfirm = useShowModal('confirmActionModal');
  const taskManager = getWorkspaceManagerStore(task.data.projectId);

  const handleArchive = () => void taskManager?.archiveWorkspace(task.data.id);
  const handleRestore = () => void taskManager?.restoreWorkspace(task.data.id);
  const handleProvision = () => void taskManager?.provisionWorkspace(task.data.id);
  const handleDelete = () =>
    showConfirm({
      title: 'Delete task',
      description: `"${task.data.name}" will be permanently deleted. This action cannot be undone.`,
      confirmLabel: 'Delete',
      onSuccess: () => void taskManager?.deleteWorkspace(task.data.id),
    });
  const handleRename = () =>
    showRename({
      projectId: task.data.projectId,
      workspaceId: task.data.id,
      currentName: task.data.name,
    });

  const isArchived = Boolean(task.data.archivedAt);
  const canPin = task.state !== 'unregistered';
  const agentAttention = taskAgentStatus(task);
  const currentPr = task.data.prs ? selectCurrentPr(task.data.prs) : undefined;
  const branchName =
    getTaskGitStore(task.data.projectId, task.data.id)?.branchName ?? task.data.workspaceBranch;

  return (
    <WorkspaceContextMenu
      isPinned={task.data.isPinned}
      canPin={canPin}
      isArchived={isArchived}
      branchName={branchName}
      onPin={() => void task.setPinned(true)}
      onUnpin={() => void task.setPinned(false)}
      onRename={handleRename}
      onArchive={handleArchive}
      onRestore={handleRestore}
      onDelete={handleDelete}
    >
      <button
        onClick={() => {
          if (isArchived) return;
          handleProvision();
          navigate('workspace', { projectId: task.data.projectId, workspaceId: task.data.id });
        }}
        className="group flex items-center gap-2 rounded-lg p-3  hover:bg-background-1 transition-colors w-full"
      >
        <div
          onClick={(e) => e.stopPropagation()}
          className={cn(
            'transition-opacity',
            isSelected ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
          )}
        >
          <Checkbox
            checked={isSelected}
            onCheckedChange={onToggleSelect}
            aria-label="Select task"
          />
        </div>
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <span className="min-w-0 text-left text-sm truncate">{task.data.name}</span>
            <WorkspaceGitDiffStats task={task} className="text-xs shrink-0" />
            {currentPr && <PrBadge pr={currentPr} />}
          </div>
        </div>
        <div className="flex items-center shrink-0 [&>span]:ring-2 [&>span]:ring-background [&>span:not(:first-child)]:-ml-1.5">
          {Object.entries(task.conversationStats).map(([providerId, count]) => {
            const config = agentConfig[providerId as keyof typeof agentConfig];
            if (!config) return null;
            return (
              <span
                key={providerId}
                className="relative flex items-center justify-center h-5 w-5 rounded-sm bg-background-2 overflow-hidden"
                title={`${config.name}: ${String(count)}`}
              >
                <AgentLogo
                  logo={config.logo}
                  alt={config.alt}
                  isSvg={config.isSvg}
                  invertInDark={config.invertInDark}
                  className="h-3.5 w-3.5"
                />
                {count > 1 && (
                  <span className="absolute -bottom-px -right-px text-[8px] leading-none font-semibold bg-background text-foreground-passive px-px rounded-tl">
                    {count}
                  </span>
                )}
              </span>
            );
          })}
        </div>
        <div
          className={cn(
            'flex min-w-8 shrink-0 items-center justify-end',
            agentAttention ? 'justify-end' : 'justify-middle'
          )}
        >
          {agentAttention ? (
            <AgentStatusIndicator status={agentAttention} />
          ) : (
            <RelativeTime
              value={task.data.createdAt}
              className="text-xs text-foreground-passive font-mono pr-1"
              compact
            />
          )}
        </div>
      </button>
    </WorkspaceContextMenu>
  );
});
