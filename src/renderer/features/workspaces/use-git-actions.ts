import { useMutation } from '@tanstack/react-query';
import { getTaskGitStore } from '@renderer/features/workspaces/stores/workspace-selectors';

export function useGitActions(projectId: string, workspaceId: string) {
  const git = getTaskGitStore(projectId, workspaceId)!;

  const hasUpstream = git?.isBranchPublished;

  const gitFetchMutation = useMutation({
    mutationFn: () => git.fetchRemote(),
  });

  const gitPullMutation = useMutation({
    mutationFn: () => git.pull(),
  });

  const gitPushMutation = useMutation({
    mutationFn: () => git?.push(),
  });

  const gitPublishMutation = useMutation({
    mutationFn: () => git.publishBranch(),
  });

  return {
    hasUpstream,
    aheadCount: git.aheadCount,
    behindCount: git.behindCount,
    publish: () => gitPublishMutation.mutate(),
    isPublishing: gitPublishMutation.isPending,
    fetch: () => gitFetchMutation.mutate(),
    isFetching: gitFetchMutation.isPending,
    pull: () => gitPullMutation.mutate(),
    isPulling: gitPullMutation.isPending,
    push: () => gitPushMutation.mutate(),
    isPushing: gitPushMutation.isPending,
  };
}
