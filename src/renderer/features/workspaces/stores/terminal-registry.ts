import { TerminalManagerStore } from '@renderer/features/workspaces/terminals/terminal-manager';

export class TerminalRegistry {
  private readonly entries = new Map<string, TerminalManagerStore>();

  acquire(workspaceId: string, projectId: string): TerminalManagerStore {
    const existing = this.entries.get(workspaceId);
    if (existing) return existing;
    const store = new TerminalManagerStore(projectId, workspaceId);
    this.entries.set(workspaceId, store);
    return store;
  }

  get(workspaceId: string): TerminalManagerStore | undefined {
    return this.entries.get(workspaceId);
  }

  release(workspaceId: string): void {
    const store = this.entries.get(workspaceId);
    if (!store) return;
    store.dispose();
    this.entries.delete(workspaceId);
  }
}

export const terminalRegistry = new TerminalRegistry();
