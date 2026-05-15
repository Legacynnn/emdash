/**
 * Renderer smoke test for the telemetry toggle. Mocks `invoke` so we
 * can exercise the load → toggle → persist round-trip plus the
 * focus-event auto-record without a host process.
 */
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

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

describe('TelemetryToggle', () => {
  it('loads the persisted toggle and lets the user flip it', async () => {
    let persisted = false;
    invokeMock.mockImplementation(async (name: string, args?: Record<string, unknown>) => {
      if (name === 'telemetry_get_enabled') return persisted;
      if (name === 'telemetry_set_enabled') {
        persisted = args?.enabled as boolean;
        return null;
      }
      if (name === 'telemetry_record') return null;
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'unsubscribe_ui_mutations') return null;
      if (name === 'projects_list') return [];
      return null;
    });

    const { App } = await import('../../App');
    render(<App />);

    const toggle = await waitFor(() =>
      screen.getByRole('checkbox', { name: /enable telemetry/i }),
    );
    expect((toggle as HTMLInputElement).checked).toBe(false);

    fireEvent.click(toggle);
    await waitFor(() => {
      expect(persisted).toBe(true);
    });
  });

  it('records app_focus on window focus', async () => {
    const recorded: string[] = [];
    invokeMock.mockImplementation(async (name: string, args?: Record<string, unknown>) => {
      if (name === 'telemetry_record') {
        recorded.push(args?.event as string);
        return null;
      }
      if (name === 'telemetry_get_enabled') return false;
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'unsubscribe_ui_mutations') return null;
      if (name === 'projects_list') return [];
      return null;
    });

    const { App } = await import('../../App');
    render(<App />);

    // Initial mount fires dau_ping + focus.
    await waitFor(() => {
      expect(recorded).toContain('app_dau_ping');
      expect(recorded).toContain('app_focus');
    });

    // A second focus event should be recorded.
    const before = recorded.length;
    window.dispatchEvent(new Event('focus'));
    await waitFor(() => expect(recorded.length).toBeGreaterThan(before));
    expect(recorded[recorded.length - 1]).toBe('app_focus');
  });
});
