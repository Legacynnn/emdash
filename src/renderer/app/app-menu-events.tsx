import { when } from 'mobx';
import { useEffect } from 'react';
import { menuOpenSettingsChannel, notificationFocusTaskChannel } from '@shared/events/appEvents';
import { getWorkspaceView } from '@renderer/features/workspaces/stores/workspace-selectors';
import { events } from '@renderer/lib/ipc';
import { useNavigate, useWorkspaceSlots } from '@renderer/lib/layout/navigation-provider';
import { toggleSettingsView } from '@renderer/lib/layout/settings-toggle';

export function AppMenuEvents({ onOpenSettings }: { onOpenSettings?: () => boolean | void }) {
  const { navigate } = useNavigate();
  const { currentView, lastNonSettingsView } = useWorkspaceSlots();

  useEffect(() => {
    return events.on(menuOpenSettingsChannel, () => {
      if (currentView !== 'settings') {
        const shouldOpen = onOpenSettings?.() ?? true;
        if (shouldOpen === false) return;
      }

      toggleSettingsView(navigate, currentView, lastNonSettingsView);
    });
  }, [navigate, onOpenSettings, currentView, lastNonSettingsView]);

  useEffect(() => {
    const disposers = new Set<() => void>();

    const unlisten = events.on(
      notificationFocusTaskChannel,
      ({ projectId, workspaceId, conversationId }) => {
        navigate('workspace', { projectId, workspaceId });
        if (!conversationId) return;

        // Workspace view may not be provisioned yet — wait for it before opening the conversation tab.
        const dispose = when(
          () => !!getWorkspaceView(projectId, workspaceId),
          () => {
            getWorkspaceView(projectId, workspaceId)?.tabManager.openConversation(conversationId);
          },
          {
            timeout: 10_000,
          }
        );

        disposers.add(dispose);
      }
    );

    return () => {
      unlisten();
      disposers.forEach((dispose) => dispose());
      disposers.clear();
    };
  }, [navigate]);

  return null;
}
