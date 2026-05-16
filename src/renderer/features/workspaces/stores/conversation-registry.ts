import type { Conversation } from '@shared/conversations';
import { ConversationManagerStore } from '@renderer/features/workspaces/conversations/conversation-manager';

export class ConversationRegistry {
  private readonly entries = new Map<string, ConversationManagerStore>();

  acquire(
    workspaceId: string,
    projectId: string,
    preloaded?: Conversation[]
  ): ConversationManagerStore {
    const existing = this.entries.get(workspaceId);
    if (existing) return existing;
    const store = new ConversationManagerStore(projectId, workspaceId, preloaded);
    this.entries.set(workspaceId, store);
    return store;
  }

  get(workspaceId: string): ConversationManagerStore | undefined {
    return this.entries.get(workspaceId);
  }

  release(workspaceId: string): void {
    const store = this.entries.get(workspaceId);
    if (!store) return;
    store.dispose();
    this.entries.delete(workspaceId);
  }
}

export const conversationRegistry = new ConversationRegistry();
