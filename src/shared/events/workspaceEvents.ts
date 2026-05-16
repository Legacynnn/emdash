import { defineEvent } from '@shared/ipc/events';
import type { PullRequest } from '@shared/pull-requests';

export const workspaceStatusUpdatedChannel = defineEvent<{
  workspaceId: string;
  projectId: string;
  status: string;
}>('workspace:status-updated');

export const workspacePrUpdatedChannel = defineEvent<{
  workspaceId: string;
  projectId: string;
  prs: PullRequest[];
}>('workspace:pr-updated');

export type ProvisionStep =
  | 'resolving-worktree'
  | 'initialising-workspace'
  | 'running-provision-script'
  | 'connecting'
  | 'setting-up-workspace'
  | 'starting-sessions';

export const workspaceProvisionProgressChannel = defineEvent<{
  workspaceId: string;
  projectId: string;
  step: ProvisionStep;
  message: string;
}>('workspace:provision-progress');
