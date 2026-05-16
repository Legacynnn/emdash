/**
 * Smoke tests for the github.* routes in the renderer shim.
 *
 * The renderer's GitHub auth flow drives state via event channels
 * (device-code, success, error). Those events have no UiMutationEvent
 * counterpart on the Rust side — the shim synthesises them from
 * successful invoke results. This test verifies the synthesis happens
 * in the expected order with the expected payloads, mocked at the
 * @tauri-apps/api/core boundary.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { installElectronApiPolyfill } from '../electron-api';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: invokeMock,
    Channel: class {
      onmessage: ((event: unknown) => void) | null = null;
    },
  };
});

interface DeviceCodePayload {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

interface SuccessPayload {
  token: string;
  user: { id: number; login: string; name: string; email: string; avatar_url: string };
}

interface ErrorPayload {
  error: string;
  message: string;
}

interface ResultEnvelope {
  ok: boolean;
  value?: unknown;
  error?: { code: string; message: string };
}

function api() {
  installElectronApiPolyfill();
  // installElectronApiPolyfill is idempotent; safe to call from each test.
  return window.electronAPI!;
}

function captureChannelOnce<T>(channel: string): Promise<T> {
  return new Promise((resolve) => {
    const off = api().eventOn(channel, (data) => {
      off();
      resolve(data as T);
    });
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  // installEventBridge subscribes to these on first install; the
  // polyfill is idempotent, but in case a fresh test run hits the
  // subscribe path, return no-ops.
  invokeMock.mockImplementation(async (name: string) => {
    if (name === 'subscribe_ui_mutations') return null;
    if (name === 'subscribe_updater_events') return null;
    throw new Error(`unexpected invoke ${name}`);
  });
});

describe('shim github.auth (device flow)', () => {
  it('emits device-code then success on a happy path', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'subscribe_updater_events') return null;
      if (name === 'github_sign_in_device_flow_start') {
        return {
          user_code: 'ABCD-1234',
          verification_uri: 'https://github.com/login/device',
          device_code: 'dev-code-xyz',
          polling_interval_seconds: 5,
          expires_in_seconds: 900,
        };
      }
      if (name === 'github_sign_in_device_flow_poll') {
        return {
          login: 'octocat',
          id: '42',
          name: 'Octo Cat',
          email: 'octo@example.com',
          avatar_url: 'https://example.com/a.png',
          token_source: 'secure_storage',
        };
      }
      throw new Error(`unexpected invoke ${name}`);
    });

    const deviceCodePromise = captureChannelOnce<DeviceCodePayload>('github:auth:device-code');
    const successPromise = captureChannelOnce<SuccessPayload>('github:auth:success');

    const result = (await api().invoke('github.auth')) as ResultEnvelope;

    const deviceCode = await deviceCodePromise;
    expect(deviceCode).toEqual({
      userCode: 'ABCD-1234',
      verificationUri: 'https://github.com/login/device',
      expiresIn: 900,
      interval: 5,
    });

    const success = await successPromise;
    expect(success.user.login).toBe('octocat');
    expect(success.user.id).toBe(42);
    expect(result).toEqual({ ok: true, value: null });
  });

  it('emits error when the device-flow start fails', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'subscribe_updater_events') return null;
      if (name === 'github_sign_in_device_flow_start') {
        throw { code: 'network', message: 'offline' };
      }
      throw new Error(`unexpected invoke ${name}`);
    });

    const errorPromise = captureChannelOnce<ErrorPayload>('github:auth:error');
    const result = (await api().invoke('github.auth')) as ResultEnvelope;

    expect(await errorPromise).toEqual({ error: 'network', message: 'offline' });
    expect(result).toEqual({ ok: false, error: { code: 'network', message: 'offline' } });
    const pollCalls = invokeMock.mock.calls.filter(
      ([name]) => name === 'github_sign_in_device_flow_poll'
    );
    expect(pollCalls).toHaveLength(0);
  });

  it('emits device-code then error when polling fails', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'subscribe_updater_events') return null;
      if (name === 'github_sign_in_device_flow_start') {
        return {
          user_code: 'WXYZ-9999',
          verification_uri: 'https://github.com/login/device',
          device_code: 'dev-code-poll',
          polling_interval_seconds: 5,
          expires_in_seconds: 900,
        };
      }
      if (name === 'github_sign_in_device_flow_poll') {
        throw { code: 'oauth', message: 'access_denied' };
      }
      throw new Error(`unexpected invoke ${name}`);
    });

    const deviceCodePromise = captureChannelOnce<DeviceCodePayload>('github:auth:device-code');
    const errorPromise = captureChannelOnce<ErrorPayload>('github:auth:error');

    const result = (await api().invoke('github.auth')) as ResultEnvelope;

    expect((await deviceCodePromise).userCode).toBe('WXYZ-9999');
    expect(await errorPromise).toEqual({ error: 'oauth', message: 'access_denied' });
    expect(result).toEqual({ ok: false, error: { code: 'oauth', message: 'access_denied' } });
  });
});

describe('shim github.getStatus', () => {
  it('returns the renderer-shaped status when signed in', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'subscribe_updater_events') return null;
      if (name === 'github_me') {
        return {
          login: 'octocat',
          id: '42',
          name: null,
          email: null,
          avatar_url: null,
          token_source: 'cli',
        };
      }
      throw new Error(`unexpected invoke ${name}`);
    });

    const status = (await api().invoke('github.getStatus')) as {
      authenticated: boolean;
      user: { login: string } | null;
      tokenSource: string | null;
    };

    expect(status.authenticated).toBe(true);
    expect(status.user?.login).toBe('octocat');
    expect(status.tokenSource).toBe('cli');
  });

  it('returns signed-out shape when github_me returns null', async () => {
    invokeMock.mockImplementation(async (name: string) => {
      if (name === 'subscribe_ui_mutations') return null;
      if (name === 'subscribe_updater_events') return null;
      if (name === 'github_me') return null;
      throw new Error(`unexpected invoke ${name}`);
    });

    const status = (await api().invoke('github.getStatus')) as {
      authenticated: boolean;
      user: unknown;
      tokenSource: unknown;
    };
    expect(status).toEqual({ authenticated: false, user: null, tokenSource: null });
  });
});
