// Tauri → renderer event bridge.
//
// The Electron renderer registers many event channels (project events,
// task events, github events, etc.). The Tauri side ships a single
// `UiMutationEvent` channel via `subscribe_ui_mutations` (EMD-7) plus
// dedicated channels for the updater (`subscribe_updater_events`) and
// fs watcher (`fs_watcher_subscribe`). This bridge:
//
//   1. Holds a `Map<channel, Set<callback>>` for renderer subscriptions
//      registered via `window.electronAPI.eventOn`.
//   2. Subscribes once to each Tauri channel and translates incoming
//      events into the matching renderer channel name(s), then fans
//      them out.
//
// Channel name format (matches `createEventEmitter` in
// `@shared/ipc/events`): `eventName` or `eventName.topic`. The
// translation table here uses the literal channel strings the
// renderer's event modules emit (see `src/shared/events/*.ts`).

import { Channel, invoke as tauriInvoke } from '@tauri-apps/api/core';

type Callback = (data: unknown) => void;

const listeners = new Map<string, Set<Callback>>();

export function registerEventListener(channel: string, cb: Callback): () => void {
  let set = listeners.get(channel);
  if (!set) {
    set = new Set();
    listeners.set(channel, set);
  }
  set.add(cb);
  return () => {
    const current = listeners.get(channel);
    if (!current) return;
    current.delete(cb);
    if (current.size === 0) listeners.delete(channel);
  };
}

function emit(channel: string, data: unknown): void {
  const set = listeners.get(channel);
  if (!set) return;
  for (const cb of set) {
    try {
      cb(data);
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error(`[tauri-shim] listener for "${channel}" threw`, err);
    }
  }
}

// -- UiMutationEvent translation --------------------------------------

// The exact channel names below must match `src/shared/events/*.ts`.
// They use a simple `kebab-case-or-namespaced` convention. Topic
// (per-id) suffixes use `.<topic>` per createEventEmitter's contract.
//
// Where the renderer expects multiple cache invalidations off a single
// Tauri event (e.g. `task_created` should bump both the task list for
// the project and any per-task subscribers), we emit on every
// matching channel.

interface UiMutationEvent {
  kind: string;
  [key: string]: unknown;
}

function translateUiMutation(event: UiMutationEvent): void {
  switch (event.kind) {
    case 'project_created':
      emit('project.created', event);
      emit('project.changed', event);
      break;
    case 'project_updated':
      emit('project.updated', event);
      emit('project.changed', event);
      if (typeof event.id === 'string') emit(`project.updated.${event.id}`, event);
      break;
    case 'project_deleted':
      emit('project.deleted', event);
      emit('project.changed', event);
      if (typeof event.id === 'string') emit(`project.deleted.${event.id}`, event);
      break;

    case 'task_created':
      emit('task.created', event);
      emit('task.changed', event);
      if (typeof event.project_id === 'string') {
        emit(`task.created.${event.project_id}`, event);
        emit(`task.changed.${event.project_id}`, event);
      }
      break;
    case 'task_updated':
      emit('task.updated', event);
      emit('task.changed', event);
      if (typeof event.id === 'string') emit(`task.updated.${event.id}`, event);
      if (typeof event.project_id === 'string')
        emit(`task.changed.${event.project_id}`, event);
      break;
    case 'task_deleted':
      emit('task.deleted', event);
      emit('task.changed', event);
      if (typeof event.id === 'string') emit(`task.deleted.${event.id}`, event);
      if (typeof event.project_id === 'string')
        emit(`task.changed.${event.project_id}`, event);
      break;

    case 'agent_hook_event':
      emit('agent.event', event.event ?? event);
      if (typeof event.task_id === 'string') emit(`agent.event.${event.task_id}`, event.event ?? event);
      break;
    case 'agent_started':
      emit('agent.started', event);
      if (typeof event.task_id === 'string') emit(`agent.started.${event.task_id}`, event);
      break;
    case 'agent_exited':
      emit('agent.exited', event);
      if (typeof event.task_id === 'string') emit(`agent.exited.${event.task_id}`, event);
      break;

    case 'github_identity_changed':
      emit('github.identity-changed', event);
      break;
    case 'github_data_changed':
      emit('github.data-changed', event);
      if (typeof event.repo === 'string') emit(`github.data-changed.${event.repo}`, event);
      break;

    case 'linear_identity_changed':
      emit('linear.identity-changed', event);
      break;
    case 'linear_data_changed':
      emit('linear.data-changed', event);
      if (typeof event.team === 'string') emit(`linear.data-changed.${event.team}`, event);
      break;

    default:
      // Unknown variant — broadcast under a generic channel so the
      // renderer's debug listeners can still see it.
      emit('tauri.unknown-mutation', event);
      break;
  }
}

interface UpdateEvent {
  kind: string;
  [key: string]: unknown;
}

function translateUpdateEvent(event: UpdateEvent): void {
  // Renderer's update store listens on a `update.status` channel; the
  // exact name will firm up as the updater port lands.
  emit('update.status', event);
  emit(`update.status.${event.kind}`, event);
}

// -- Bridge install ---------------------------------------------------

let bridgeInstalled = false;

export async function installEventBridge(): Promise<void> {
  if (bridgeInstalled) return;
  bridgeInstalled = true;

  const subId = `tauri-shim-${Math.random().toString(36).slice(2)}`;

  const uiChannel = new Channel<UiMutationEvent>();
  uiChannel.onmessage = (event) => {
    translateUiMutation(event);
  };

  try {
    await tauriInvoke('subscribe_ui_mutations', { subId, onEvent: uiChannel });
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error('[tauri-shim] subscribe_ui_mutations failed', err);
  }

  const updaterChannel = new Channel<UpdateEvent>();
  updaterChannel.onmessage = (event) => {
    translateUpdateEvent(event);
  };

  try {
    await tauriInvoke('subscribe_updater_events', { onEvent: updaterChannel });
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error('[tauri-shim] subscribe_updater_events failed', err);
  }
}
