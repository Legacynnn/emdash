/**
 * Renderer smoke test for the Projects CRUD feature. Mocks `invoke` at
 * the `@tauri-apps/api/core` boundary so the store→bindings→UI loop
 * runs end-to-end without a host process.
 */
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

type InvokeMock = ReturnType<typeof vi.fn>;

const invokeMock: InvokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: invokeMock,
    // A tiny Channel stand-in that supports the onmessage assignment used by
    // useUiMutations. Nothing in this test fires events through it.
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
  // Dynamic import after the mock is registered.
  const { App } = await import('../../App');
  return App;
}

describe('ProjectsPanel', () => {
  it('renders an empty list after a successful Reload', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'projects_list') return [];
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'unsubscribe_ui_mutations') return null;
      // Other commands invoked from App.tsx UI (greet, get_path) are not
      // touched in this test — fall through to a benign default.
      return null;
    });

    const App = await loadApp();
    render(<App />);

    fireEvent.click(screen.getByRole('button', { name: /reload/i }));
    await waitFor(() => {
      expect(screen.getByText(/no projects yet/i)).toBeDefined();
    });
  });

  it('adds a project and surfaces it after the round-trip', async () => {
    let listResponse: Array<{
      id: string;
      name: string;
      path: string;
      created_at: string;
      updated_at: string;
    }> = [];

    invokeMock.mockImplementation(async (name: string, args?: Record<string, unknown>) => {
      if (name === 'projects_list') return listResponse;
      if (name === 'projects_add') {
        const created = {
          id: 'p1',
          name: 'repo',
          path: args?.path as string,
          created_at: 't',
          updated_at: 't',
        };
        listResponse = [created];
        return created;
      }
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'unsubscribe_ui_mutations') return null;
      return null;
    });

    const App = await loadApp();
    render(<App />);

    const pathInput = screen.getByLabelText(/project path/i);
    fireEvent.change(pathInput, { target: { value: '/tmp/repo' } });
    fireEvent.click(screen.getByRole('button', { name: /^add$/i }));

    // Wait for the add round-trip to complete (input is cleared) before
    // triggering reload — otherwise the click fires before the mock has
    // installed the new row in `listResponse`.
    await waitFor(() => {
      expect((pathInput as HTMLInputElement).value).toBe('');
    });

    fireEvent.click(screen.getByRole('button', { name: /reload/i }));

    await waitFor(() => {
      expect(screen.getByText('repo')).toBeDefined();
      expect(screen.getByText('/tmp/repo')).toBeDefined();
    });
  });

  it('surfaces an add error from the command envelope', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'projects_list') return [];
      if (name === 'projects_add') {
        // tauri-specta returns Err on Result<_, _> by throwing the error
        // envelope; mirror that here.
        throw { code: 'duplicate_path', message: 'a project already tracks this path: /tmp/repo' };
      }
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'unsubscribe_ui_mutations') return null;
      return null;
    });

    const App = await loadApp();
    render(<App />);

    fireEvent.change(screen.getByLabelText(/project path/i), {
      target: { value: '/tmp/repo' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^add$/i }));

    await waitFor(() => {
      expect(screen.getByText(/duplicate_path/i)).toBeDefined();
    });
  });
});
