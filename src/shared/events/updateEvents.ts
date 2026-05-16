import { defineEvent } from '@shared/ipc/events';

// Subset of electron-updater's UpdateInfo, declared locally so the
// renderer doesn't pull in electron-updater at build time. The Tauri
// updater plugin populates the same shape (version + release notes +
// release date), so the renderer's update store consumes it as-is.
export interface UpdateInfo {
  version: string;
  releaseDate?: string;
  releaseName?: string | null;
  releaseNotes?: string | Array<{ version: string; note: string | null }> | null;
}

export const updateCheckingEvent = defineEvent<void>('update:checking');

export const updateAvailableEvent = defineEvent<{
  version: string;
  updateInfo: UpdateInfo;
}>('update:available');

export const updateNotAvailableEvent = defineEvent<void>('update:not-available');

export const updateDownloadingEvent = defineEvent<{ version: string }>('update:downloading');

export const updateProgressEvent = defineEvent<{
  percent: number;
  transferred: number;
  total: number;
  bytesPerSecond: number;
}>('update:progress');

export const updateDownloadedEvent = defineEvent<{ version: string }>('update:downloaded');

export const updateInstallingEvent = defineEvent<void>('update:installing');

export const updateErrorEvent = defineEvent<{ message: string }>('update:error');
