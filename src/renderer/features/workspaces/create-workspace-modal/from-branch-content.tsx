import { BranchPickerField, type Placement } from './branch-picker-field';
import {
  InitialConversationField,
  type InitialConversationState,
} from './initial-conversation-section';
import { type FromBranchModeState } from './use-from-branch-mode';
import { WorkspaceNameField } from './workspace-name-field';

interface FromBranchContentProps {
  state: FromBranchModeState;
  projectId?: string;
  currentBranch?: string | null;
  isUnborn?: boolean;
  initialConversation: InitialConversationState;
  placement: Placement;
  onPlacementChange: (next: Placement) => void;
}

export function FromBranchContent({
  state,
  projectId,
  currentBranch,
  isUnborn,
  initialConversation,
  placement,
  onPlacementChange,
}: FromBranchContentProps) {
  return (
    <div className="flex flex-col gap-4">
      <BranchPickerField
        state={state}
        projectId={projectId}
        currentBranch={currentBranch}
        isUnborn={isUnborn}
        placement={placement}
        onPlacementChange={onPlacementChange}
      />
      <WorkspaceNameField state={state} />
      <InitialConversationField state={initialConversation} />
    </div>
  );
}
