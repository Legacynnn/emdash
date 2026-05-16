import { MessageSquare } from 'lucide-react';
import { observer } from 'mobx-react-lite';
import { useEffect, useMemo, useRef } from 'react';
import { useIsActiveTask } from '@renderer/features/workspaces/hooks/use-is-active-workspace';
import {
  useConversations,
  useTaskViewContext,
  useWorkspace,
  useWorkspaceViewModel,
} from '@renderer/features/workspaces/workspace-view-context';
import { useShowModal } from '@renderer/lib/modal/modal-provider';
import { PaneSizingProvider } from '@renderer/lib/pty/pane-sizing-context';
import { PtyPane } from '@renderer/lib/pty/pty-pane';
import { TerminalSearchOverlay } from '@renderer/lib/pty/terminal-search-overlay';
import { useTerminalSearch } from '@renderer/lib/pty/use-terminal-search';
import { Button } from '@renderer/lib/ui/button';
import { EmptyState } from '@renderer/lib/ui/empty-state';
import { ShortcutHint } from '@renderer/lib/ui/shortcut-hint';
import { ContextBar } from './context-bar';
import type { ConversationStore } from './conversation-manager';

export const ConversationsPanel = observer(function ConversationsPanel() {
  const { projectId, workspaceId } = useTaskViewContext();
  const workspaceView = useWorkspaceViewModel();
  const conversations = useConversations();
  const workspace = useWorkspace();
  const { tabManager: tm } = workspaceView;
  const showCreateConversationModal = useShowModal('createConversationModal');
  const isActive = useIsActiveTask(workspaceId);
  const remoteConnectionId = workspace.sshConnectionId;
  const shouldSetWorkingOnEnter = !remoteConnectionId;

  const autoFocus = isActive && workspaceView.focusedRegion === 'main';

  const handleCreate = () =>
    showCreateConversationModal({
      projectId,
      workspaceId,
      onSuccess: ({ conversationId }) => {
        tm.openConversation(conversationId);
        workspaceView.setFocusedRegion('main');
      },
    });

  // Build session ID list for PaneSizingProvider (all open conversation tabs).
  const allSessionIds = useMemo(() => {
    return tm.resolvedTabs
      .filter((t) => t.kind === 'conversation')
      .map((t) => conversations.sessions.get(t.store.data.id)?.sessionId)
      .filter((id): id is string => Boolean(id));
  }, [tm.resolvedTabs, conversations.sessions]);

  const activeConversation: ConversationStore | undefined = tm.activeConversation;
  const activeSession = activeConversation
    ? (conversations.sessions.get(activeConversation.data.id) ?? null)
    : null;
  const activeSessionId = activeSession?.sessionId ?? null;
  const hasConversationTabs = tm.resolvedTabs.some((t) => t.kind === 'conversation');

  const containerRef = useRef<HTMLDivElement>(null);
  const terminalContainerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<{ focus: () => void }>(null);
  const focusPendingRef = useRef(false);

  const {
    isSearchOpen,
    searchQuery,
    searchStatus,
    searchInputRef,
    closeSearch,
    handleSearchQueryChange,
    stepSearch,
  } = useTerminalSearch({
    terminal: activeSession?.pty?.terminal,
    containerRef: terminalContainerRef,
    enabled: Boolean(activeSession?.pty),
    onCloseFocus: () => terminalRef.current?.focus(),
  });

  useEffect(() => {
    if (!autoFocus) return;
    if (terminalRef.current) {
      terminalRef.current.focus();
      focusPendingRef.current = false;
    } else {
      containerRef.current?.focus();
      focusPendingRef.current = true;
    }
  }, [autoFocus, activeSessionId]);

  const sessionStatus = activeSession?.status;
  useEffect(() => {
    if (sessionStatus === 'ready' && focusPendingRef.current) {
      focusPendingRef.current = false;
      terminalRef.current?.focus();
    }
  }, [sessionStatus]);

  const onEnterPress =
    shouldSetWorkingOnEnter && activeConversation
      ? () => {
          activeConversation.setWorking();
          void conversations.touchConversation(activeConversation.data.id);
        }
      : undefined;

  const onInterruptPress = activeConversation ? () => activeConversation.clearWorking() : undefined;

  return (
    <div className="flex h-full flex-col">
      <div className="min-h-0 flex-1">
        <div
          ref={containerRef}
          tabIndex={-1}
          className="flex h-full flex-col outline-none"
          onFocus={() => {
            if (isActive) workspaceView.setFocusedRegion('main');
          }}
          onBlur={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node)) {
              // focus left the panel — no region change needed
            }
          }}
        >
          <PaneSizingProvider paneId="conversations" sessionIds={allSessionIds}>
            {!hasConversationTabs ? (
              <EmptyState
                icon={<MessageSquare className="h-5 w-5 text-muted-foreground" />}
                label="No conversations yet"
                description="Create one to open a terminal session for this task and work with an agent."
                action={
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleCreate}
                    className="flex items-center gap-2"
                  >
                    Create conversation
                    <ShortcutHint settingsKey="newConversation" />
                  </Button>
                }
              />
            ) : (
              <div className="flex min-h-0 flex-1 flex-col">
                {activeSessionId && activeSession?.status === 'ready' && activeSession.pty ? (
                  <div ref={terminalContainerRef} className="relative flex h-full min-h-0 flex-1">
                    <TerminalSearchOverlay
                      isOpen={isSearchOpen}
                      fullWidth
                      searchQuery={searchQuery}
                      searchStatus={searchStatus}
                      searchInputRef={searchInputRef}
                      onQueryChange={handleSearchQueryChange}
                      onStep={stepSearch}
                      onClose={closeSearch}
                    />
                    <PtyPane
                      ref={terminalRef}
                      sessionId={activeSessionId}
                      pty={activeSession.pty}
                      className="h-full w-full"
                      onEnterPress={onEnterPress}
                      onInterruptPress={onInterruptPress}
                      mapShiftEnterToCtrlJ
                      remoteConnectionId={remoteConnectionId}
                    />
                  </div>
                ) : null}
              </div>
            )}
          </PaneSizingProvider>
        </div>
      </div>
      <ContextBar />
    </div>
  );
});
