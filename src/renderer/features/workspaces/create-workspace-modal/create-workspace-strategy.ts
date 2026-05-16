import type { CreateWorkspaceStrategy } from '@shared/workspaces';

type BranchLikeTaskStrategy = Extract<
  CreateWorkspaceStrategy,
  { kind: 'new-branch' | 'no-worktree' }
>;
type PullRequestTaskStrategy = Extract<CreateWorkspaceStrategy, { kind: 'from-pull-request' }>;

export function resolveBranchLikeTaskStrategy(input: {
  isUnborn: boolean;
  createBranchAndWorktree: boolean;
  workspaceBranch: string;
  pushBranch: boolean;
}): BranchLikeTaskStrategy {
  if (input.isUnborn || !input.createBranchAndWorktree) {
    return { kind: 'no-worktree' };
  }

  return {
    kind: 'new-branch',
    workspaceBranch: input.workspaceBranch,
    pushBranch: input.pushBranch,
  };
}

export function resolvePullRequestTaskStrategy(input: {
  checkoutMode: 'checkout' | 'new-branch';
  prNumber: number;
  headBranch: string;
  headRepositoryUrl: string;
  isFork: boolean;
  workspaceBranch: string;
  pushBranch: boolean;
}): PullRequestTaskStrategy {
  if (input.checkoutMode === 'checkout') {
    return {
      kind: 'from-pull-request',
      prNumber: input.prNumber,
      headBranch: input.headBranch,
      headRepositoryUrl: input.headRepositoryUrl,
      isFork: input.isFork,
    };
  }

  return {
    kind: 'from-pull-request',
    prNumber: input.prNumber,
    headBranch: input.headBranch,
    headRepositoryUrl: input.headRepositoryUrl,
    isFork: input.isFork,
    workspaceBranch: input.workspaceBranch,
    pushBranch: input.pushBranch,
  };
}
