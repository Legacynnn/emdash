/**
 * Renderer store registry. A single `ProjectStore` per mount; workspace
 * stores are scoped per project and live in `workspacesByProject` keyed
 * by project id (so the same store survives across observer renders
 * within a project view).
 */
import { makeAutoObservable } from 'mobx';
import { ProjectStore } from './projectStore';
import { WorkspaceStore } from './taskStore';

export class RendererStores {
  projects = new ProjectStore();
  workspacesByProject = new Map<string, WorkspaceStore>();

  constructor() {
    // The `workspacesByProject` Map is intentionally non-observable: callers
    // observe individual `WorkspaceStore` instances. `getOrCreateWorkspaceStore`
    // mutates the Map, which never triggers UI churn (only the store
    // it returns is observable).
    makeAutoObservable(this, { workspacesByProject: false });
  }

  getOrCreateWorkspaceStore(projectId: string): WorkspaceStore {
    let store = this.workspacesByProject.get(projectId);
    if (!store) {
      store = new WorkspaceStore(projectId);
      this.workspacesByProject.set(projectId, store);
    }
    return store;
  }
}

export function createStores(): RendererStores {
  return new RendererStores();
}
