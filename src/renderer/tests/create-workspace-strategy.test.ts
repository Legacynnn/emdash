import { describe, expect, it } from 'vitest';
import {
  resolveBranchLikeTaskStrategy,
  resolvePullRequestTaskStrategy,
} from '@renderer/features/workspaces/create-workspace-modal/create-workspace-strategy';

describe('resolveBranchLikeTaskStrategy', () => {
  it('returns new-branch with pushBranch when branch/worktree creation is enabled', () => {
    expect(
      resolveBranchLikeTaskStrategy({
        isUnborn: false,
        createBranchAndWorktree: true,
        workspaceBranch: 'issue-task',
        pushBranch: false,
      })
    ).toEqual({
      kind: 'new-branch',
      workspaceBranch: 'issue-task',
      pushBranch: false,
    });
  });

  it('returns no-worktree when branch/worktree creation is disabled', () => {
    expect(
      resolveBranchLikeTaskStrategy({
        isUnborn: false,
        createBranchAndWorktree: false,
        workspaceBranch: 'issue-task',
        pushBranch: true,
      })
    ).toEqual({ kind: 'no-worktree' });
  });

  it('returns no-worktree for unborn repositories', () => {
    expect(
      resolveBranchLikeTaskStrategy({
        isUnborn: true,
        createBranchAndWorktree: true,
        workspaceBranch: 'issue-task',
        pushBranch: true,
      })
    ).toEqual({ kind: 'no-worktree' });
  });

  it('returns new-local for local-new placement', () => {
    expect(
      resolveBranchLikeTaskStrategy({
        isUnborn: false,
        createBranchAndWorktree: true,
        placementMode: 'local-new',
        workspaceBranch: 'in-place-feature',
        pushBranch: false,
      })
    ).toEqual({ kind: 'new-local', workspaceBranch: 'in-place-feature' });
  });

  it('returns existing-local for local-existing placement', () => {
    expect(
      resolveBranchLikeTaskStrategy({
        isUnborn: false,
        createBranchAndWorktree: true,
        placementMode: 'local-existing',
        workspaceBranch: 'fallback-name',
        existingBranch: 'feat/in-progress',
        pushBranch: false,
      })
    ).toEqual({ kind: 'existing-local', workspaceBranch: 'feat/in-progress' });
  });

  it('falls back to workspaceBranch when existingBranch is absent in local-existing', () => {
    expect(
      resolveBranchLikeTaskStrategy({
        isUnborn: false,
        createBranchAndWorktree: true,
        placementMode: 'local-existing',
        workspaceBranch: 'feat/foo',
        pushBranch: false,
      })
    ).toEqual({ kind: 'existing-local', workspaceBranch: 'feat/foo' });
  });
});

describe('resolvePullRequestTaskStrategy', () => {
  it('includes workspaceBranch and pushBranch in new-branch mode', () => {
    expect(
      resolvePullRequestTaskStrategy({
        checkoutMode: 'new-branch',
        prNumber: 42,
        headBranch: 'feature/pr-head',
        headRepositoryUrl: 'https://github.com/contributor/repo',
        isFork: false,
        workspaceBranch: 'pr-task',
        pushBranch: false,
      })
    ).toEqual({
      kind: 'from-pull-request',
      prNumber: 42,
      headBranch: 'feature/pr-head',
      headRepositoryUrl: 'https://github.com/contributor/repo',
      isFork: false,
      workspaceBranch: 'pr-task',
      pushBranch: false,
    });
  });

  it('omits workspaceBranch and pushBranch in checkout mode', () => {
    expect(
      resolvePullRequestTaskStrategy({
        checkoutMode: 'checkout',
        prNumber: 42,
        headBranch: 'feature/pr-head',
        headRepositoryUrl: 'https://github.com/contributor/repo',
        isFork: false,
        workspaceBranch: 'pr-task',
        pushBranch: false,
      })
    ).toEqual({
      kind: 'from-pull-request',
      prNumber: 42,
      headBranch: 'feature/pr-head',
      headRepositoryUrl: 'https://github.com/contributor/repo',
      isFork: false,
    });
  });
});
