import { observer } from 'mobx-react-lite';
import { commitRef, remoteRef, type GitChange } from '@shared/git';
import { getPrNumber, type PullRequest } from '@shared/pull-requests';
import { getRepositoryStore } from '@renderer/features/projects/stores/project-selectors';
import { usePrefetchDiffModels } from '@renderer/features/workspaces/diff-view/changes-panel/hooks/use-prefetch-diff-models';
import {
  useInfraId,
  useTaskViewContext,
  useWorkspaceViewModel,
} from '@renderer/features/workspaces/workspace-view-context';
import { VirtualizedChangesList } from '../virtualized-changes-list';

export const PrFilesList = observer(function PrFilesList({ pr }: { pr: PullRequest }) {
  const { projectId } = useTaskViewContext();
  const infraId = useInfraId();
  const workspaceView = useWorkspaceViewModel();
  const prStore = workspaceView.prStore!;

  const repo = getRepositoryStore(projectId);
  const baseRef = remoteRef(repo?.baseRemote ?? 'origin', pr.baseRefName);
  const modifiedRef = commitRef(pr.headRefOid);
  const prFiles = prStore.getFiles(pr).data ?? [];

  const prefetchPrDiff = usePrefetchDiffModels(projectId, infraId, 'pr', baseRef, modifiedRef);

  const activePath =
    workspaceView.tabManager.activeDescriptor?.kind === 'diff' &&
    workspaceView.tabManager.activeDescriptor.diffGroup === 'pr'
      ? workspaceView.tabManager.activeDescriptor.path
      : undefined;

  const handleSelectChange = (change: GitChange) => {
    workspaceView.tabManager.openDiffPreview(
      {
        path: change.path,
        type: 'git',
        group: 'pr',
        originalRef: baseRef,
        modifiedRef,
        prNumber: getPrNumber(pr) ?? undefined,
      },
      change.status
    );
  };

  const handleDoubleClickChange = (change: GitChange) => {
    workspaceView.tabManager.openDiff(
      {
        path: change.path,
        type: 'git',
        group: 'pr',
        originalRef: baseRef,
        modifiedRef,
        prNumber: getPrNumber(pr) ?? undefined,
      },
      change.status
    );
  };

  return (
    <VirtualizedChangesList
      className="py-3"
      changes={prFiles}
      activePath={activePath}
      onSelectChange={handleSelectChange}
      onDoubleClickChange={handleDoubleClickChange}
      onPrefetch={(change) => prefetchPrDiff(change.path)}
    />
  );
});
