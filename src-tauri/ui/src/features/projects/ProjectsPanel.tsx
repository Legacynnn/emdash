/**
 * Projects CRUD panel. The first feature on the new Tauri stack — meant
 * as the template every later feature follows: bindings → MobX store →
 * observer component that routes through state-guard selectors.
 */
import { observer } from 'mobx-react-lite';
import { useState } from 'react';
import type { ProjectsCommandError } from '../../bindings';
import { asReady, projectStoreKind, type ProjectStore } from '../../stores/projectStore';

function formatError(err: ProjectsCommandError): string {
  return `[${err.code}] ${err.message}`;
}

function isCommandError(value: unknown): value is ProjectsCommandError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    'message' in value
  );
}

export interface ProjectsPanelProps {
  store: ProjectStore;
}

export const ProjectsPanel = observer(function ProjectsPanel({ store }: ProjectsPanelProps) {
  const [path, setPath] = useState('');
  const [actionError, setActionError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const kind = projectStoreKind(store);
  const ready = asReady(store);

  async function handleAdd() {
    setActionError(null);
    setPending(true);
    try {
      const result = await store.add(path);
      if (isCommandError(result)) {
        setActionError(`add failed: ${formatError(result)}`);
        return;
      }
      setPath('');
    } finally {
      setPending(false);
    }
  }

  async function handleRemove(id: string) {
    setActionError(null);
    setPending(true);
    try {
      const err = await store.remove(id);
      if (err) {
        setActionError(`remove failed: ${formatError(err)}`);
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <section>
      <h2>Projects</h2>
      <p className="muted">
        Add a project by absolute path. The bridge round-trips every mutation through{' '}
        <code>UiMutationEvent</code>; the list updates without a manual refresh.
      </p>

      <div className="row">
        <input
          type="text"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder="/absolute/path/to/repo"
          aria-label="project path"
          disabled={pending}
        />
        <button
          type="button"
          onClick={handleAdd}
          disabled={pending || path.trim().length === 0}
        >
          {pending ? 'Working...' : 'Add'}
        </button>
        <button type="button" onClick={() => store.load()} disabled={pending}>
          Reload
        </button>
      </div>

      {kind === 'idle' && (
        <p className="muted">
          Click <strong>Reload</strong> to fetch the current list.
        </p>
      )}
      {kind === 'loading' && <p className="muted">Loading…</p>}
      {kind === 'error' && store.state.kind === 'error' && (
        <pre className="error">{formatError(store.state.error)}</pre>
      )}
      {ready && ready.projects.length === 0 && (
        <p className="muted">No projects yet.</p>
      )}
      {ready && ready.projects.length > 0 && (
        <ul className="project-list" aria-label="projects">
          {ready.projects.map((p) => (
            <li key={p.id}>
              <div>
                <strong>{p.name}</strong>
                <div className="muted">
                  <code>{p.path}</code>
                </div>
              </div>
              <button
                type="button"
                onClick={() => handleRemove(p.id)}
                disabled={pending}
                aria-label={`remove ${p.name}`}
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
