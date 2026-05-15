/**
 * Updater panel — exercises every UpdateEvent variant against the
 * mock state machine. Once EMD-22 wires the real
 * `tauri-plugin-updater` flow, the `Check for updates` button can
 * drop the `simulate` calls and rely on the actual plugin events.
 */
import { Channel } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { commands, type UpdateEvent } from '../../bindings';

function formatEvent(event: UpdateEvent | null): string {
  if (!event) return 'idle';
  switch (event.kind) {
    case 'checking':
      return `checking (${event.reason})`;
    case 'up_to_date':
      return 'up to date';
    case 'available':
      return `available: ${event.version}${event.notes ? ` — ${event.notes}` : ''}`;
    case 'downloading':
      return `downloading: ${((event.progress ?? 0) * 100).toFixed(1)}%`;
    case 'ready_to_install':
      return `ready to install: ${event.version}`;
    case 'error':
      return `error [${event.code}] ${event.message}${event.will_retry ? ' (will retry)' : ''}`;
  }
}

export function UpdaterPanel() {
  const [event, setEvent] = useState<UpdateEvent | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // emdash-disable-next-line no-tauri-event-bus -- Channel<T> is the
    // sanctioned transport for renderer state sync (ADR-0004).
    const channel = new Channel<UpdateEvent>();
    channel.onmessage = (e) => setEvent(e);
    void commands.subscribeUpdaterEvents(channel);
  }, []);

  async function runCheck() {
    setError(null);
    const result = await commands.updaterCheck('manual');
    if (result.status === 'error') {
      setError(`check failed: [${result.error.code}] ${result.error.message}`);
    }
  }

  async function simulate(e: UpdateEvent) {
    setError(null);
    const result = await commands.updaterSimulateEvent(e);
    if (result.status === 'error') {
      setError(`simulate failed: [${result.error.code}] ${result.error.message}`);
    }
  }

  return (
    <section>
      <h2>Updater</h2>
      <p className="muted">
        State: <strong>{formatEvent(event)}</strong>. The mock buttons below walk the state machine
        through every variant — the real updater flow wires in with the packaging follow-up
        (EMD-22). See ADR-0008 for the design.
      </p>
      <div className="row" style={{ flexWrap: 'wrap', gap: '8px' }}>
        <button type="button" onClick={runCheck}>
          Check for updates
        </button>
        <button type="button" onClick={() => void simulate({ kind: 'up_to_date' })}>
          → up_to_date
        </button>
        <button
          type="button"
          onClick={() => void simulate({ kind: 'available', version: '0.2.0', notes: 'Bug fixes' })}
        >
          → available
        </button>
        <button
          type="button"
          onClick={() => void simulate({ kind: 'downloading', progress: 0.42 })}
        >
          → downloading
        </button>
        <button
          type="button"
          onClick={() => void simulate({ kind: 'ready_to_install', version: '0.2.0' })}
        >
          → ready_to_install
        </button>
        <button
          type="button"
          onClick={() =>
            void simulate({
              kind: 'error',
              code: 'network',
              message: 'connection refused',
              will_retry: true,
            })
          }
        >
          → error
        </button>
      </div>
      {error && <pre className="error">{error}</pre>}
    </section>
  );
}
