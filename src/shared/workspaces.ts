import type { CreateConversationParams } from '@shared/conversations';
import type { ProvisionStep } from '@shared/events/workspaceEvents';
import type { Branch, CreateBranchError, FetchPrForReviewError, PushError } from '@shared/git';
import type { PullRequest } from '@shared/pull-requests';

export type WorkspaceLifecycleStatus = 'todo' | 'in_progress' | 'review' | 'done' | 'cancelled';

export type Issue = {
  provider: 'github' | 'linear' | 'jira' | 'gitlab' | 'plain' | 'forgejo' | 'featurebase';
  url: string;
  title: string;
  identifier: string;
  description?: string;
  branchName?: string;
  status?: string;
  assignees?: string[];
  project?: string;
  updatedAt?: string;
  fetchedAt?: string;
};

export type Workspace = {
  id: string;
  projectId: string;
  name: string;
  status: WorkspaceLifecycleStatus;
  sourceBranch: Branch | undefined;
  workspaceBranch?: string;
  /** Mirrors Rust `WorkspacePlacement` in bindings. */
  placement: 'worktree' | 'local';
  createdAt: string;
  updatedAt: string;
  /** ISO timestamp: when lifecycle status last changed (current status entered). */
  statusChangedAt: string;
  archivedAt?: string;
  lastInteractedAt?: string;
  linkedIssue?: Issue;
  isPinned: boolean;
  prs: PullRequest[];
  conversations: Record<string, number>;
  workspaceGit?: { linesAdded: number; linesDeleted: number };
  /**
   * BYOI/infra handle: identifier of the underlying infrastructure
   * (local path, SSH workspace, or BYOI provider workspace) where this
   * Workspace is provisioned. Distinct from the Workspace entity's own
   * `id`. `null`/undefined while unprovisioned.
   */
  workspaceId?: string;
};

export type WorkspaceBootstrapStatus =
  | { status: 'ready' }
  | { status: 'bootstrapping' }
  | { status: 'error'; message: string }
  | { status: 'not-started' };

export type CreateWorkspaceStrategy =
  | { kind: 'new-branch'; workspaceBranch: string; pushBranch?: boolean }
  | { kind: 'checkout-existing' }
  | {
      kind: 'from-pull-request';
      prNumber: number;
      /** The PR's headRefName, used as the local branch name (same as `gh pr checkout`). */
      headBranch: string;
      headRepositoryUrl: string;
      isFork: boolean;
      workspaceBranch?: string;
      pushBranch?: boolean;
    }
  | { kind: 'no-worktree' };

export type CreateWorkspaceParams = {
  id: string;
  projectId: string;
  name: string;
  /** The branch to fork the new worktree from (not used for `from-pull-request` strategy) */
  sourceBranch: Branch;
  /** Controls branch creation, worktree setup, and git fetch strategy */
  strategy: CreateWorkspaceStrategy;
  /** The issue to link to the workspace */
  linkedIssue?: Issue;
  /**  */
  initialConversation?: CreateConversationParams;
  initialStatus?: WorkspaceLifecycleStatus;
  infraProvider?: 'byoi';
};

export type CreateWorkspaceError =
  | { type: 'project-not-found' }
  | { type: 'initial-commit-required'; branch: string }
  | { type: 'branch-create-failed'; branch: string; error: CreateBranchError }
  | { type: 'pr-fetch-failed'; error: FetchPrForReviewError; remote: string }
  | { type: 'branch-not-found'; branch: string }
  | { type: 'worktree-setup-failed'; branch: string; message?: string }
  | { type: 'provision-failed'; message: string }
  | { type: 'provision-timeout'; timeoutMs: number; step: ProvisionStep | null };

export type CreateWorkspaceWarning = {
  type: 'branch-publish-failed';
  branch: string;
  remote: string;
  error: PushError;
};

export type CreateWorkspaceSuccess = {
  workspace: Workspace;
  warning?: CreateWorkspaceWarning;
};

export type ProvisionWorkspaceResult = {
  path: string;
  workspaceId: string;
};

export function formatIssueAsPrompt(issue: Issue, initialPrompt?: string): string {
  const parts = [`[${issue.identifier}] ${issue.title}`, issue.url, issue.description].filter(
    Boolean
  );

  if (initialPrompt?.trim()) parts.push('', initialPrompt.trim());
  return parts.join('\n');
}
