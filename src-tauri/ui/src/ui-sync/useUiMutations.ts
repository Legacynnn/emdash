/**
 * `useUiMutations` — single root-mount hook that opens the renderer's
 * one and only `Channel<UiMutationEvent>` subscription and routes each
 * incoming event through `dispatchUiMutation`.
 *
 * Lifecycle:
 *   - one fresh `subId` per mount (UUID v4)
 *   - subscribe on mount, unsubscribe explicitly on teardown
 *   - StrictMode (dev) double-invoke is handled: the second mount sees
 *     the first unsubscribed
 *
 * No other component in the renderer should subscribe directly. The
 * `Channel<UiMutationEvent>` primitive itself is an explicit exemption
 * (the only one) to the `no-tauri-event-bus` rule — high-bandwidth
 * per-call streams (PTY data) use a separate `Channel<Vec<u8>>`.
 */
import { Channel } from '@tauri-apps/api/core';
import { useEffect } from 'react';
import { commands, type UiMutationEvent } from '../bindings';
import { uuidV4 } from '../lib/uuid';
import { dispatchUiMutation, type Stores } from './dispatch';

export function useUiMutations(stores: Stores): void {
  useEffect(() => {
    const subId = uuidV4();
    let unsubscribed = false;

    // emdash-disable-next-line no-tauri-event-bus -- Channel<T> is the
    // sanctioned UI-mutation transport; see ADR-0004.
    const channel = new Channel<UiMutationEvent>();
    channel.onmessage = (event) => {
      if (unsubscribed) return;
      dispatchUiMutation(stores, event);
    };

    void commands.subscribeUiMutations(subId, channel);

    return () => {
      unsubscribed = true;
      void commands.unsubscribeUiMutations(subId);
    };
  }, [stores]);
}
