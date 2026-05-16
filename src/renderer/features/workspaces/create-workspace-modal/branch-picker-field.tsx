import { ChevronDown, GitBranch } from 'lucide-react';
import { BranchDisplay } from '@renderer/lib/components/branch-display';
import { ProjectBranchSelector } from '@renderer/lib/components/project-branch-selector';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@renderer/lib/ui/collapsible';
import { ComboboxTrigger, ComboboxValue } from '@renderer/lib/ui/combobox';
import { Field, FieldLabel } from '@renderer/lib/ui/field';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@renderer/lib/ui/select';
import { Switch } from '@renderer/lib/ui/switch';
import { cn } from '@renderer/utils/utils';
import { type BranchSelectionState } from './use-branch-selection';

export type Placement = 'worktree' | 'local';

interface BranchPickerFieldProps {
  state: BranchSelectionState;
  projectId?: string;
  currentBranch?: string | null;
  label?: string;
  className?: string;
  isUnborn?: boolean;
  placement: Placement;
  onPlacementChange: (next: Placement) => void;
  placementDisabled?: boolean;
}

export function BranchPickerField({
  state,
  projectId,
  currentBranch,
  label = 'From Branch',
  className,
  isUnborn = false,
  placement,
  onPlacementChange,
  placementDisabled = false,
}: BranchPickerFieldProps) {
  const { createBranchAndWorktree, setCreateBranchAndWorktree, pushBranch, setPushBranch } = state;
  const isLocal = placement === 'local';
  const placementSelectDisabled = placementDisabled || isUnborn;

  return (
    <div className={cn('border border-border rounded-md overflow-hidden', className)}>
      {!createBranchAndWorktree && currentBranch && !isLocal ? (
        <BranchDisplay label={label} branchName={currentBranch} />
      ) : projectId ? (
        <ProjectBranchSelector
          projectId={projectId}
          value={state.selectedBranch}
          onValueChange={state.setSelectedBranch}
          showRemoteSelectorFooter
          trigger={
            <ComboboxTrigger className="flex w-full items-center gap-2 justify-between hover:bg-background-1 data-popup-open:bg-background-1 p-2 outline-none">
              <div className="flex flex-col text-left text-sm gap-0.5">
                <span className="text-foreground-passive text-xs">{label}</span>
                <span className="flex items-center gap-1">
                  <GitBranch
                    absoluteStrokeWidth
                    strokeWidth={2}
                    className="size-3.5 shrink-0 text-foreground-muted"
                  />
                  <ComboboxValue placeholder="Select a branch" />
                </span>
              </div>

              <ChevronDown className="size-4 shrink-0 text-foreground-muted" />
            </ComboboxTrigger>
          }
        />
      ) : null}
      <Collapsible className="border-t border-border">
        <CollapsibleTrigger className="w-full p-2 hover:bg-background-1 data-open:bg-background-1 flex text-xs text-foreground-muted items-center gap-2 justify-between">
          Settings
          <ChevronDown className="size-4 shrink-0 text-foreground-muted" />
        </CollapsibleTrigger>
        <CollapsibleContent className="overflow-hidden h-(--collapsible-panel-height) transition-[height] duration-200 ease-out">
          <div className="p-2 flex flex-col gap-2">
            <Field orientation="horizontal">
              <FieldLabel className="text-xs text-foreground-muted">Placement</FieldLabel>
              <Select
                value={placement}
                onValueChange={(next) => onPlacementChange(next as Placement)}
                disabled={placementSelectDisabled}
              >
                <SelectTrigger className="h-6 w-auto shrink-0 gap-2 text-xs [&>span]:line-clamp-none">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="min-w-max">
                  <SelectItem value="worktree">Worktree</SelectItem>
                  <SelectItem value="local">Work locally</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            {!isLocal && !isUnborn && (
              <>
                <Field orientation="horizontal">
                  <Switch
                    checked={createBranchAndWorktree}
                    onCheckedChange={setCreateBranchAndWorktree}
                  />
                  <FieldLabel>Create workspace branch and worktree</FieldLabel>
                </Field>
                {createBranchAndWorktree && (
                  <Field orientation="horizontal">
                    <Switch checked={pushBranch} onCheckedChange={setPushBranch} />
                    <FieldLabel>Push branch to remote</FieldLabel>
                  </Field>
                )}
              </>
            )}
            {isUnborn && (
              <p className="text-xs text-foreground-muted">
                Create an initial commit to enable branch-based workspaces.
              </p>
            )}
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}
