import { ChevronDown, GitBranch } from 'lucide-react';
import { BranchDisplay } from '@renderer/lib/components/branch-display';
import { ProjectBranchSelector } from '@renderer/lib/components/project-branch-selector';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@renderer/lib/ui/collapsible';
import { ComboboxTrigger, ComboboxValue } from '@renderer/lib/ui/combobox';
import { Field, FieldLabel } from '@renderer/lib/ui/field';
import { Switch } from '@renderer/lib/ui/switch';
import { ToggleGroup, ToggleGroupItem } from '@renderer/lib/ui/toggle-group';
import { cn } from '@renderer/utils/utils';
import { type PlacementMode } from './create-workspace-strategy';
import { type BranchSelectionState } from './use-branch-selection';

interface BranchPickerFieldProps {
  state: BranchSelectionState;
  projectId?: string;
  currentBranch?: string | null;
  label?: string;
  className?: string;
  isUnborn?: boolean;
}

const PLACEMENT_LABELS: Record<PlacementMode, string> = {
  worktree: 'New (worktree)',
  'local-new': 'New (local)',
  'local-existing': 'Existing local',
};

export function BranchPickerField({
  state,
  projectId,
  currentBranch,
  label = 'From Branch',
  className,
  isUnborn = false,
}: BranchPickerFieldProps) {
  const {
    createBranchAndWorktree,
    setCreateBranchAndWorktree,
    pushBranch,
    setPushBranch,
    placementMode,
    setPlacementMode,
  } = state;
  const sourceLabel = placementMode === 'local-existing' ? 'Existing branch' : label;

  return (
    <div className={cn('border border-border rounded-md overflow-hidden', className)}>
      <div className="p-2 border-b border-border bg-background-1">
        <ToggleGroup
          className="w-full"
          value={[placementMode]}
          onValueChange={([value]) => {
            if (!value) return;
            const next = value as PlacementMode;
            if (isUnborn && next === 'local-existing') return;
            setPlacementMode(next);
          }}
        >
          {(['worktree', 'local-new', 'local-existing'] as PlacementMode[]).map((mode) => (
            <ToggleGroupItem
              key={mode}
              className="flex-1 text-xs"
              value={mode}
              disabled={isUnborn && mode === 'local-existing'}
            >
              {PLACEMENT_LABELS[mode]}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>
      {!createBranchAndWorktree && currentBranch && placementMode === 'worktree' ? (
        <BranchDisplay label={sourceLabel} branchName={currentBranch} />
      ) : projectId ? (
        <ProjectBranchSelector
          projectId={projectId}
          value={state.selectedBranch}
          onValueChange={state.setSelectedBranch}
          showRemoteSelectorFooter={placementMode !== 'local-existing'}
          trigger={
            <ComboboxTrigger className="flex w-full items-center gap-2 justify-between hover:bg-background-1 data-popup-open:bg-background-1 p-2 outline-none">
              <div className="flex flex-col text-left text-sm gap-0.5">
                <span className="text-foreground-passive text-xs">{sourceLabel}</span>
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
      {placementMode === 'local-existing' ? (
        <p className="border-t border-border bg-background-1 px-2 py-1 text-xs text-foreground-muted">
          Switches the project directory to this branch. Workspace shares the project tree.
        </p>
      ) : placementMode === 'local-new' ? (
        <p className="border-t border-border bg-background-1 px-2 py-1 text-xs text-foreground-muted">
          Creates a new branch off the selected source and switches the project to it. No worktree.
        </p>
      ) : !isUnborn ? (
        <Collapsible className="border-t border-border">
          <CollapsibleTrigger className="w-full p-2 hover:bg-background-1 data-open:bg-background-1 flex text-xs text-foreground-muted items-center gap-2 justify-between">
            Should create and push feature branch
            <ChevronDown className="size-4 shrink-0 text-foreground-muted" />
          </CollapsibleTrigger>
          <CollapsibleContent className="overflow-hidden h-(--collapsible-panel-height) transition-[height] duration-200 ease-out">
            <div className="p-2 flex flex-col gap-2">
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
            </div>
          </CollapsibleContent>
        </Collapsible>
      ) : (
        <p className="border-t border-border bg-background-1 px-2 py-1 text-xs text-foreground-muted">
          Create an initial commit to enable branch-based workspaces.
        </p>
      )}
    </div>
  );
}
