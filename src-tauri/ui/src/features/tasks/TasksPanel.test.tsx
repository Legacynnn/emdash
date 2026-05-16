/**
 * Renderer smoke test for the Tasks CRUD feature. Same mock-boundary
 * pattern as `ProjectsPanel.test.tsx` — invoke at
 * `@tauri-apps/api/core` returns fixture data so the store→bindings
 * loop runs without a host.
 */
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

type InvokeMock = ReturnType<typeof vi.fn>;

const invokeMock: InvokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: invokeMock,
    Channel: class {
      onmessage: ((event: unknown) => void) | null = null;
    },
  };
});

beforeEach(() => {
  invokeMock.mockReset();
  cleanup();
});

async function loadApp() {
  const { App } = await import('../../App');
  return App;
}

const project = {
  id: 'p1',
  name: 'repo',
  path: '/tmp/repo',
  created_at: 't',
  updated_at: 't',
};

const taskRow = {
  id: 'task-1',
  project_id: 'p1',
  name: 'Feature X',
  status: 'active' as const,
  placement: 'worktree' as const,
  path: '/tmp/repo/.emdash-worktrees/task-1',
  source_branch: { type: 'local' as const, branch: 'main' },
  pty_id: null,
  created_at: 't',
  updated_at: 't',
};

describe('TasksPanel', () => {
  it('appears after selecting a project and lists tasks', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'projects_list') return [project];
      if (name === 'workspaces_list') return [taskRow];
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'unsubscribe_ui_mutations') return null;
      return null;
    });

    const App = await loadApp();
    render(<App />);

    fireEvent.click(screen.getByRole('button', { name: /reload/i }));
    await waitFor(() => {
      expect(screen.getByText('repo')).toBeDefined();
    });

    fireEvent.click(screen.getByRole('button', { name: /select repo/i }));

    await waitFor(() => {
      expect(screen.getByText('Feature X')).toBeDefined();
      expect(screen.getByText('/tmp/repo/.emdash-worktrees/task-1')).toBeDefined();
    });
  });

  it('surfaces a worktree-failed error from the command envelope', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'projects_list') return [project];
      if (name === 'workspaces_list') return [];
      if (name === 'workspaces_create') {
        throw {
          code: 'worktree_failed',
          message: 'git worktree add failed: fatal: branch missing',
        };
      }
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'unsubscribe_ui_mutations') return null;
      return null;
    });

    const App = await loadApp();
    render(<App />);

    fireEvent.click(screen.getByRole('button', { name: /reload/i }));
    await waitFor(() => expect(screen.getByText('repo')).toBeDefined());
    fireEvent.click(screen.getByRole('button', { name: /select repo/i }));
    await waitFor(() => expect(screen.getByText(/no tasks yet/i)).toBeDefined());

    fireEvent.change(screen.getByLabelText(/task name/i), {
      target: { value: 'Feature Y' },
    });
    fireEvent.click(screen.getByRole('button', { name: /create task/i }));

    await waitFor(() => {
      expect(screen.getByText(/worktree_failed/i)).toBeDefined();
    });
  });
});
