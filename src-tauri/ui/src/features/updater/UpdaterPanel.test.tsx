/**
 * Smoke test for the updater panel. Renders the panel + verifies it
 * subscribes to the host channel and exposes the simulate-flow
 * controls. The state-machine *logic* is exhaustively tested on the
 * Rust side (`src/updater/state.rs`); the renderer just needs to
 * present the events correctly.
 */
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();

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

describe('UpdaterPanel', () => {
  it('subscribes to updater events and shows the simulate controls', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'projects_list') return [];
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'unsubscribe_ui_mutations') return null;
      if (name === 'subscribe_updater_events') return null;
      return null;
    });

    const { App } = await import('../../App');
    render(<App />);

    await waitFor(() => {
      // Subscription fires on mount.
      expect(invokeMock.mock.calls.some((call) => call[0] === 'subscribe_updater_events')).toBe(
        true
      );
    });

    // Initial state surface is "idle" before any event arrives.
    expect(screen.getByText(/State:/i).textContent).toContain('idle');

    // The simulate buttons are present.
    expect(screen.getByRole('button', { name: /check for updates/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /→ up_to_date/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /→ available/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /→ downloading/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /→ ready_to_install/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /→ error/i })).toBeDefined();
  });
});
