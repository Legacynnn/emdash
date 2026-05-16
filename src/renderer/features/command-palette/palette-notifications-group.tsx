import { Command } from 'cmdk';
import { useObserver } from 'mobx-react-lite';
import {
  asMounted,
  getProjectManagerStore,
} from '@renderer/features/projects/stores/project-selectors';
import type { ConversationStore } from '@renderer/features/workspaces/conversations/conversation-manager';
import { conversationRegistry } from '@renderer/features/workspaces/stores/conversation-registry';
import { getWorkspaceView } from '@renderer/features/workspaces/stores/workspace-selectors';
import {
  isRegistered,
  type WorkspaceStore,
} from '@renderer/features/workspaces/stores/workspace-store';
import type { NavigateFnTyped } from '@renderer/lib/layout/navigation-provider';
import { cn } from '@renderer/utils/utils';
import { PaletteConversationItem } from './palette-conversation-item';
import { PaletteTaskItem } from './palette-task-item';

type NotificationItem =
  | { kind: 'task'; projectId: string; taskStore: WorkspaceStore }
  | { kind: 'conversation'; projectId: string; workspaceId: string; conv: ConversationStore };

const GROUP_CLASS = cn(
  '[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5',
  '[&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium',
  '[&_[cmdk-group-heading]]:text-foreground/50'
);

interface PaletteNotificationsGroupProps {
  currentProjectId: string | undefined;
  currentTaskId: string | undefined;
  onClose: () => void;
  navigate: NavigateFnTyped;
}

export function PaletteNotificationsGroup({
  currentProjectId,
  currentTaskId,
  onClose,
  navigate,
}: PaletteNotificationsGroupProps) {
  const items = useObserver((): NotificationItem[] => {
    const result: NotificationItem[] = [];

    for (const projectStore of getProjectManagerStore().projects.values()) {
      const mounted = asMounted(projectStore);
      if (!mounted) continue;
      const pid = mounted.data.id;

      for (const [tid, taskStore] of mounted.taskManager.tasks) {
        if (!isRegistered(taskStore)) continue;
        const conversations = conversationRegistry.get(tid);
        if (!conversations) continue;

        const status = conversations.taskStatus;
        // Only surface awaiting-input, error, completed — not working or idle.
        if (!status || status === 'idle' || status === 'working') continue;

        if (pid === currentProjectId && tid === currentTaskId) {
          // We're already in this task — surface individual unseen conversations.
          for (const conv of conversations.conversations.values()) {
            if (!conv.seen && conv.indicatorStatus) {
              result.push({ kind: 'conversation', projectId: pid, workspaceId: tid, conv });
            }
          }
        } else {
          result.push({ kind: 'task', projectId: pid, taskStore });
        }
      }
    }

    return result;
  });

  if (items.length === 0) return null;

  return (
    <Command.Group heading="Notifications" className={GROUP_CLASS}>
      {items.map((item) => {
        if (item.kind === 'conversation') {
          return (
            <PaletteConversationItem
              key={item.conv.data.id}
              conv={item.conv}
              value={`notif:conversation:${item.conv.data.id}`}
              onSelect={() => {
                getWorkspaceView(item.projectId, item.workspaceId)?.tabManager.openConversation(
                  item.conv.data.id
                );
                if (item.projectId !== currentProjectId || item.workspaceId !== currentTaskId) {
                  navigate('workspace', {
                    projectId: item.projectId,
                    workspaceId: item.workspaceId,
                  });
                }
                onClose();
              }}
            />
          );
        }
        return (
          <PaletteTaskItem
            key={item.taskStore.data.id}
            taskStore={item.taskStore}
            value={`notif:task:${item.taskStore.data.id}`}
            onSelect={() => {
              navigate('workspace', {
                projectId: item.projectId,
                workspaceId: item.taskStore.data.id,
              });
              onClose();
            }}
          />
        );
      })}
    </Command.Group>
  );
}
