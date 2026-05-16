import { useParams, useWorkspaceSlots } from '@renderer/lib/layout/navigation-provider';

export function useIsActiveTask(workspaceId: string): boolean {
  const { currentView } = useWorkspaceSlots();
  const { params } = useParams('workspace');
  return currentView === 'workspace' && params.workspaceId === workspaceId;
}
