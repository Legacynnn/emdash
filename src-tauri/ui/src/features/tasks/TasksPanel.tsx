/**
 * Tasks list + create/delete for a single project. Follows the same
 * shape as `ProjectsPanel`: observer component, state-guard selector,
 * MobX store, broadcast-driven refresh through `useUiMutations`.
 */
import { observer } from 'mobx-react-lite';
import { useEffect, useState } from 'react';
import type { Project, WorkspacesCommandError_Serialize } from '../../bindings';
import { asReady, workspaceStoreKind, type WorkspaceStore } from '../../stores/taskStore';

function formatError(err: WorkspacesCommandError_Serialize): string {
  return `[${err.code}] ${err.message}`;
}

function isCommandError(value: unknown): value is WorkspacesCommandError_Serialize {
  return typeof value === 'object' && value !== null && 'code' in value && 'message' in value;
}

export interface TasksPanelProps {
  project: Project;
  store: WorkspaceStore;
}

export const TasksPanel = observer(function TasksPanel({ project, store }: TasksPanelProps) {
  const [name, setName] = useState('');
  const [branch, setBranch] = useState('main');
  const [pending, setPending] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  // Auto-load on first mount of a project view — the user sees the
  // current state without an extra Reload click.
  useEffect(() => {
    if (workspaceStoreKind(store) === 'idle') {
      void store.load();
    }
  }, [store]);

  const kind = workspaceStoreKind(store);
  const ready = asReady(store);

  async function handleCreate() {
    setActionError(null);
    setPending(true);
    try {
      const result = await store.create(name, { type: 'local', branch });
      if (isCommandError(result)) {
        setActionError(`create failed: ${formatError(result)}`);
        return;
      }
      setName('');
    } finally {
      setPending(false);
    }
  }

  async function handleDelete(id: string) {
    setActionError(null);
    setPending(true);
    try {
      const err = await store.delete(id);
      if (err) setActionError(`delete failed: ${formatError(err)}`);
    } finally {
      setPending(false);
    }
  }

  return (
    <section>
      <h2>
        Tasks <span className="muted">— {project.name}</span>
      </h2>
      <p className="muted">
        Each task gets a git worktree under{' '}
        <code>{project.path}/.emdash-worktrees/&lt;task-id&gt;</code>.
      </p>

      <div className="row">
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="task name"
          aria-label="task name"
          disabled={pending}
        />
        <input
          type="text"
          value={branch}
          onChange={(e) => setBranch(e.target.value)}
          placeholder="source branch"
          aria-label="source branch"
          disabled={pending}
        />
        <button
          type="button"
          onClick={handleCreate}
          disabled={pending || name.trim().length === 0 || branch.trim().length === 0}
        >
          {pending ? 'Working…' : 'Create task'}
        </button>
        <button type="button" onClick={() => store.load()} disabled={pending}>
          Reload
        </button>
      </div>

      {kind === 'loading' && <p className="muted">Loading…</p>}
      {kind === 'error' && store.state.kind === 'error' && (
        <pre className="error">{formatError(store.state.error)}</pre>
      )}
      {ready && ready.workspaces.length === 0 && <p className="muted">No tasks yet.</p>}
      {ready && ready.workspaces.length > 0 && (
        <ul className="project-list" aria-label="tasks">
          {ready.workspaces.map((t) => (
            <li key={t.id}>
              <div>
                <strong>{t.name}</strong>
                <div className="muted">
                  <code>{t.path}</code>
                </div>
                <div className="muted">
                  source:{' '}
                  {t.source_branch.type === 'local' ? (
                    <code>{t.source_branch.branch}</code>
                  ) : (
                    <code>
                      {t.source_branch.host}/{t.source_branch.branch}
                    </code>
                  )}
                </div>
              </div>
              <button
                type="button"
                onClick={() => handleDelete(t.id)}
                disabled={pending}
                aria-label={`remove ${t.name}`}
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}

      {actionError && <pre className="error">{actionError}</pre>}
    </section>
  );
});
