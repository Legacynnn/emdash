import { ChevronRight, FolderOpen } from 'lucide-react';
import { observer } from 'mobx-react-lite';
import { useCallback, useEffect, useState } from 'react';
import { getPrNumber, isForkPr, type PullRequest } from '@shared/pull-requests';
import {
  getProjectManagerStore,
  getRepositoryStore,
  mountedProjectData,
} from '@renderer/features/projects/stores/project-selectors';
import { nextDefaultConversationTitle } from '@renderer/features/workspaces/conversations/conversation-title-utils';
import { ProjectSelector } from '@renderer/features/workspaces/create-workspace-modal/project-selector';
import { useAgentAutoApproveDefaults } from '@renderer/features/workspaces/hooks/useAgentAutoApproveDefaults';
import { useFeatureFlag } from '@renderer/lib/hooks/useFeatureFlag';
import { useNavigate } from '@renderer/lib/layout/navigation-provider';
import { type BaseModalProps } from '@renderer/lib/modal/modal-provider';
import { appState } from '@renderer/lib/stores/app-state';
import { AnimatedHeight } from '@renderer/lib/ui/animated-height';
import { ComboboxTrigger, ComboboxValue } from '@renderer/lib/ui/combobox';
import { ConfirmButton } from '@renderer/lib/ui/confirm-button';
import {
  DialogContentArea,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@renderer/lib/ui/dialog';
import { Switch } from '@renderer/lib/ui/switch';
import { ToggleGroup, ToggleGroupItem } from '@renderer/lib/ui/toggle-group';
import {
  resolveBranchLikeTaskStrategy,
  resolvePullRequestTaskStrategy,
} from './create-workspace-strategy';
import { FromBranchContent } from './from-branch-content';
import { FromIssueContent } from './from-issue-content';
import { FromPrContent } from './from-pr-content';
import { useInitialConversationState } from './initial-conversation-section';
import { useFromBranchMode } from './use-from-branch-mode';
import { useFromIssueMode } from './use-from-issue-mode';
import { useFromPullRequestMode } from './use-from-pull-request-mode';

type CreateWorkspaceStrategy = 'from-branch' | 'from-issue' | 'from-pull-request';

export const CreateWorkspaceModal = observer(function CreateWorkspaceModal({
  projectId,
  strategy = 'from-branch',
  initialPR,
  onClose,
}: BaseModalProps & {
  projectId?: string;
  strategy?: CreateWorkspaceStrategy;
  initialPR?: PullRequest;
}) {
  const [selectedProjectId, setSelectedProjectId] = useState<string | undefined>(() => {
    if (projectId) return projectId;
    const nav = appState.navigation;
    const navProjectId =
      nav.currentViewId === 'workspace'
        ? (nav.viewParamsStore['workspace'] as { projectId?: string } | undefined)?.projectId
        : nav.currentViewId === 'project'
          ? (nav.viewParamsStore['project'] as { projectId?: string } | undefined)?.projectId
          : undefined;
    return (
      navProjectId ??
      Array.from(getProjectManagerStore().projects.values())
        .reverse()
        .find((p) => p.state === 'mounted')?.data?.id
    );
  });
  const [selectedStrategy, setSelectedStrategy] = useState<CreateWorkspaceStrategy>(strategy);
  const [isTransitioning, setIsTransitioning] = useState(false);
  const [useBYOI, setUseBYOI] = useState(false);
  const [placement, setPlacement] = useState<'worktree' | 'local'>('worktree');

  const projectData = selectedProjectId
    ? mountedProjectData(getProjectManagerStore().projects.get(selectedProjectId))
    : null;
  const initialConversation = useInitialConversationState(selectedProjectId);
  const autoApproveDefaults = useAgentAutoApproveDefaults();

  useEffect(() => setUseBYOI(false), [selectedProjectId]);
  useEffect(() => {
    initialConversation.setProvider(null);
    initialConversation.setPrompt('');
    // setProvider and setPrompt are stable useState setters
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedProjectId]);

  const isWorkspaceProviderEnabled = useFeatureFlag('workspace-provider');
  useEffect(() => {
    if (!isWorkspaceProviderEnabled) setUseBYOI(false);
  }, [isWorkspaceProviderEnabled]);

  const repo = selectedProjectId ? getRepositoryStore(selectedProjectId) : undefined;
  const defaultBranch = repo?.defaultBranch;
  const isUnborn = repo?.isUnborn ?? false;
  const currentBranch = repo?.currentBranch ?? null;
  const { navigate } = useNavigate();

  const repositoryUrl = selectedProjectId
    ? (getRepositoryStore(selectedProjectId)?.repositoryUrl ?? undefined)
    : undefined;

  const fromBranch = useFromBranchMode(selectedProjectId, defaultBranch, isUnborn, currentBranch);
  const fromIssue = useFromIssueMode(selectedProjectId, defaultBranch, isUnborn, currentBranch);
  const fromPR = useFromPullRequestMode(selectedProjectId, defaultBranch, isUnborn, initialPR);

  // Keep the per-tab branch-selection state's placementMode in sync with
  // the modal-level placement toggle. `local-existing` is unused for now —
  // any future "switch to an existing branch" UX can layer it on without
  // touching this toggle.
  useEffect(() => {
    const mode = placement === 'local' ? 'local-new' : 'worktree';
    fromBranch.setPlacementMode(mode);
    fromIssue.setPlacementMode(mode);
  }, [placement, fromBranch, fromIssue]);

  // Local placement doesn't apply to from-pull-request (a PR review wants
  // its own worktree). Snap back to worktree if the user switches to PR.
  useEffect(() => {
    if (selectedStrategy === 'from-pull-request' && placement === 'local') {
      setPlacement('worktree');
    }
  }, [selectedStrategy, placement]);
  const fromPrUnavailable = selectedStrategy === 'from-pull-request' && !repositoryUrl;

  const activeMode = {
    'from-branch': fromBranch,
    'from-issue': fromIssue,
    'from-pull-request': fromPR,
  }[selectedStrategy];
  const canCreate = !!selectedProjectId && activeMode.isValid && !fromPrUnavailable;

  const handleCreateTask = useCallback(() => {
    if (!selectedProjectId) return;
    const id = crypto.randomUUID();
    const projectStore = getProjectManagerStore().projects.get(selectedProjectId);
    if (projectStore?.state !== 'mounted') return;

    const builtInitialConversation = initialConversation.provider
      ? {
          id: crypto.randomUUID(),
          projectId: selectedProjectId,
          workspaceId: id,
          provider: initialConversation.provider,
          title: nextDefaultConversationTitle(initialConversation.provider, []),
          initialPrompt: initialConversation.prompt.trim() || undefined,
          autoApprove: autoApproveDefaults.getDefault(initialConversation.provider),
        }
      : undefined;

    switch (selectedStrategy) {
      case 'from-branch': {
        if (!fromBranch.selectedBranch) return;
        const taskStrategy = resolveBranchLikeTaskStrategy({
          isUnborn,
          createBranchAndWorktree: fromBranch.createBranchAndWorktree,
          placementMode: placement === 'local' ? 'local-new' : 'worktree',
          workspaceBranch: fromBranch.taskName,
          pushBranch: fromBranch.pushBranch,
        });
        void projectStore.mountedProject!.taskManager.createTask({
          id,
          projectId: selectedProjectId,
          name: fromBranch.taskName,
          sourceBranch: fromBranch.selectedBranch,
          strategy: useBYOI ? { kind: 'no-worktree' } : taskStrategy,
          infraProvider: useBYOI ? 'byoi' : undefined,
          initialConversation: builtInitialConversation,
        });
        break;
      }
      case 'from-issue': {
        if (!fromIssue.selectedBranch) return;
        const taskStrategy = resolveBranchLikeTaskStrategy({
          isUnborn,
          createBranchAndWorktree: fromIssue.createBranchAndWorktree,
          placementMode: placement === 'local' ? 'local-new' : 'worktree',
          workspaceBranch: fromIssue.taskName,
          pushBranch: fromIssue.pushBranch,
        });
        void projectStore.mountedProject!.taskManager.createTask({
          id,
          projectId: selectedProjectId,
          name: fromIssue.taskName,
          sourceBranch: fromIssue.selectedBranch,
          strategy: useBYOI ? { kind: 'no-worktree' } : taskStrategy,
          linkedIssue: fromIssue.linkedIssue ?? undefined,
          infraProvider: useBYOI ? 'byoi' : undefined,
          initialConversation: builtInitialConversation,
        });
        break;
      }
      case 'from-pull-request': {
        if (!fromPR.linkedPR) return;
        const reviewBranch = fromPR.linkedPR.headRefName;
        const taskStrategy = resolvePullRequestTaskStrategy({
          checkoutMode: fromPR.checkoutMode,
          prNumber: getPrNumber(fromPR.linkedPR) ?? 0,
          headBranch: reviewBranch,
          headRepositoryUrl: fromPR.linkedPR.headRepositoryUrl,
          isFork: isForkPr(fromPR.linkedPR),
          workspaceBranch: fromPR.taskName,
          pushBranch: fromPR.branchSelection.pushBranch,
        });
        void projectStore.mountedProject!.taskManager.createTask({
          id,
          projectId: selectedProjectId,
          name: fromPR.taskName,
          sourceBranch: { type: 'local', branch: reviewBranch },
          initialStatus:
            fromPR.linkedPR.status === 'open' && !fromPR.linkedPR.isDraft ? 'review' : undefined,
          strategy: useBYOI ? { kind: 'no-worktree' } : taskStrategy,
          infraProvider: useBYOI ? 'byoi' : undefined,
          initialConversation: builtInitialConversation,
        });
        break;
      }
    }

    navigate('workspace', { projectId: selectedProjectId, workspaceId: id });
    onClose();
  }, [
    selectedProjectId,
    selectedStrategy,
    fromBranch,
    fromIssue,
    fromPR,
    isUnborn,
    useBYOI,
    placement,
    initialConversation,
    autoApproveDefaults,
    navigate,
    onClose,
  ]);

  return (
    <>
      <DialogHeader className="flex items-center gap-2">
        <ProjectSelector
          value={selectedProjectId}
          onChange={setSelectedProjectId}
          trigger={
            <ComboboxTrigger className="h-6 flex items-center gap-2 border border-border rounded-md px-2.5 py-1 text-sm outline-none">
              <FolderOpen className="size-3.5 shrink-0 text-muted-foreground" />
              <ComboboxValue placeholder="Select a project" />
            </ComboboxTrigger>
          }
        />
        <ChevronRight className="size-3.5 text-foreground-passive" />
        <DialogTitle>Create Workspace</DialogTitle>
      </DialogHeader>
      <DialogContentArea className="gap-4">
        <ToggleGroup
          className="w-full"
          value={[selectedStrategy]}
          onValueChange={([value]) => {
            if (value) {
              setSelectedStrategy(value as CreateWorkspaceStrategy);
            }
          }}
        >
          <ToggleGroupItem className="flex-1" value="from-branch">
            From Branch
          </ToggleGroupItem>
          <ToggleGroupItem className="flex-1" value="from-issue">
            From Issue
          </ToggleGroupItem>
          <ToggleGroupItem className="flex-1" value="from-pull-request">
            From Pull Request
          </ToggleGroupItem>
        </ToggleGroup>
        {isWorkspaceProviderEnabled && (
          <div className="flex items-center gap-2">
            <Switch size="sm" checked={useBYOI} onCheckedChange={setUseBYOI} />
            <span className="text-sm text-muted-foreground">Use BYOI infrastructure</span>
          </div>
        )}
        <AnimatedHeight onAnimatingChange={setIsTransitioning}>
          {selectedStrategy === 'from-branch' && (
            <FromBranchContent
              state={fromBranch}
              projectId={selectedProjectId}
              currentBranch={currentBranch}
              isUnborn={isUnborn}
              initialConversation={initialConversation}
              placement={placement}
              onPlacementChange={setPlacement}
            />
          )}
          {selectedStrategy === 'from-issue' && (
            <FromIssueContent
              state={fromIssue}
              projectId={selectedProjectId}
              currentBranch={currentBranch}
              repositoryUrl={repositoryUrl}
              projectPath={projectData?.path}
              disabled={isTransitioning}
              isUnborn={isUnborn}
              initialConversation={initialConversation}
              placement={placement}
              onPlacementChange={setPlacement}
            />
          )}
          {selectedStrategy === 'from-pull-request' && (
            <div className="flex flex-col gap-3">
              {!repositoryUrl && (
                <p className="text-sm text-muted-foreground">
                  Pull requests are currently available only for configured GitHub remotes.
                </p>
              )}
              <FromPrContent
                state={fromPR}
                projectId={selectedProjectId}
                repositoryUrl={repositoryUrl}
                disabled={isTransitioning || fromPrUnavailable}
                initialConversation={initialConversation}
              />
            </div>
          )}
        </AnimatedHeight>
      </DialogContentArea>
      <DialogFooter>
        <ConfirmButton size="sm" onClick={handleCreateTask} disabled={!canCreate}>
          Create
        </ConfirmButton>
      </DialogFooter>
    </>
  );
});
