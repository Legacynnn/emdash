import { getWorkspaceManagerStore } from '@renderer/features/workspaces/stores/workspace-selectors';
import { isRegistered } from '@renderer/features/workspaces/stores/workspace-store';

export type LinkedIssueInfo = { workspaceId: string; taskName: string };

/**
 * Reads from observable task state — call only inside `observer` components.
 * Returns a map from issue URL → linked task info for non-archived tasks,
 * optionally excluding a single task (e.g. when re-selecting the same task's issue).
 */
export function getLinkedIssueMap(
  projectId: string | undefined,
  excludeTaskId?: string
): Map<string, LinkedIssueInfo> {
  const map = new Map<string, LinkedIssueInfo>();
  if (!projectId) return map;
  const taskManager = getWorkspaceManagerStore(projectId);
  if (!taskManager) return map;
  for (const store of taskManager.tasks.values()) {
    if (!isRegistered(store)) continue;
    if (excludeTaskId && store.data.id === excludeTaskId) continue;
    if (store.data.archivedAt) continue;
    const url = store.data.linkedIssue?.url;
    if (!url || map.has(url)) continue;
    map.set(url, { workspaceId: store.data.id, taskName: store.data.name });
  }
  return map;
}
