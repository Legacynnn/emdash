import { type Branch } from '@shared/git';
import { useTaskSettings } from '@renderer/features/workspaces/hooks/useTaskSettings';
import { useBranchSelection } from './use-branch-selection';
import { useTaskName } from './use-workspace-name';

export type FromBranchModeState = ReturnType<typeof useFromBranchMode>;

// The task name field starts empty. The user types the name; the
// branch and worktree derive from it via buildBranchName. No
// auto-generated placeholder or LLM round-trip — the field is the
// source of truth.
export function useFromBranchMode(
  selectedProjectId: string | undefined,
  defaultBranch: Branch | undefined,
  isUnborn: boolean,
  currentBranchName?: string | null
) {
  const { createBranchAndWorktree } = useTaskSettings();
  const branchSelection = useBranchSelection(
    selectedProjectId,
    defaultBranch,
    isUnborn,
    currentBranchName,
    createBranchAndWorktree
  );

  const taskName = useTaskName({ resetKey: selectedProjectId });

  const isValid =
    taskName.taskName.trim().length > 0 && branchSelection.selectedBranch !== undefined;

  return {
    ...branchSelection,
    ...taskName,
    isValid,
  };
}
