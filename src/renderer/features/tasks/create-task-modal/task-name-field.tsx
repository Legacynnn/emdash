import { Field, FieldLabel } from '@renderer/lib/ui/field';
import { Input } from '@renderer/lib/ui/input';
import { type TaskNameState } from './use-task-name';

interface TaskNameFieldProps {
  state: TaskNameState;
}

// The typed value here is also the branch name. Anything users type
// flows to `git worktree add -b <value>` verbatim, so the hint
// matches the live transformer's allowlist.
export function TaskNameField({ state }: TaskNameFieldProps) {
  const { taskName, handleTaskNameChange, showSlugHint } = state;

  return (
    <Field>
      <FieldLabel>Task name</FieldLabel>
      <Input
        data-autofocus
        value={taskName}
        onChange={(e) => handleTaskNameChange(e.target.value)}
        placeholder="feat/add-search"
      />
      {showSlugHint && (
        <p className="text-xs text-muted-foreground mt-1">
          Branch names only allow lowercase letters, numbers, and{' '}
          <code className="rounded bg-muted/60 px-1">- _ / .</code>
        </p>
      )}
    </Field>
  );
}
