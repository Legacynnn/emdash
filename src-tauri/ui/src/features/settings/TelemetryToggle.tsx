/**
 * Renderer-side telemetry toggle + auto-event wiring.
 *
 * - "Send telemetry to help improve emdash - dev" toggle (off by default).
 * - On mount: fire one `app.dau_ping` and start listening for `window`
 *   focus/blur events. Both gates (compile-time host + user toggle)
 *   live in the host runtime, so the renderer can fire freely; an
 *   event with the toggle off becomes a no-op at the boundary.
 *
 * **No Tauri event-bus subscriptions.** Browser `window.focus` /
 * `window.blur` events are the right primitive — the ESLint rule
 * `no-tauri-event-bus` would catch a stray `listen('tauri://focus', …)`
 * if anyone reached for it.
 */
import { useEffect, useState } from 'react';
import { commands, type TelemetryCommandError, type TelemetryEvent } from '../../bindings';

function formatError(err: TelemetryCommandError): string {
  return `[${err.code}] ${err.message}`;
}

export function TelemetryToggle() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  // Initial fetch of the persisted toggle.
  useEffect(() => {
    void (async () => {
      const result = await commands.telemetryGetEnabled();
      if (result.status === 'ok') {
        setEnabled(result.data);
      } else {
        setError(`load failed: ${formatError(result.error)}`);
      }
    })();
  }, []);

  // Fire one DAU ping per app-root mount + on window focus / blur. The
  // host gates whether any of these actually leave the machine.
  useEffect(() => {
    const record = (event: TelemetryEvent) => {
      void commands.telemetryRecord(event, null, null, null);
    };

    record('app_dau_ping');
    record('app_focus');

    const onFocus = () => record('app_focus');
    const onBlur = () => record('app_unfocus');
    window.addEventListener('focus', onFocus);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('blur', onBlur);
    };
  }, []);

  async function toggle(next: boolean) {
    setError(null);
    setPending(true);
    try {
      const result = await commands.telemetrySetEnabled(next);
      if (result.status === 'ok') {
        setEnabled(next);
      } else {
        setError(`save failed: ${formatError(result.error)}`);
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <section>
      <h2>Settings — Telemetry</h2>
      <p className="muted">
        Help improve emdash - dev by sending anonymous usage events. Off by default. Stored locally
        in <code>app_settings</code>; broadcast nothing until you opt in.
      </p>
      <label className="row" style={{ alignItems: 'center', gap: '8px' }}>
        <input
          type="checkbox"
          checked={enabled === true}
          disabled={enabled === null || pending}
          onChange={(e) => void toggle(e.target.checked)}
          aria-label="enable telemetry"
        />
        <span>Send telemetry to help improve emdash - dev</span>
      </label>
      {error && <pre className="error">{error}</pre>}
    </section>
  );
}
