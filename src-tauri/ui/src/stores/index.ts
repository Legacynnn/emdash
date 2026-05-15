/**
 * Renderer store registry. A single `ProjectStore` per mount; task
 * stores are scoped per project and live in `tasksByProject` keyed
 * by project id (so the same store survives across observer renders
 * within a project view).
 */
import { makeAutoObservable } from 'mobx';
import { ProjectStore } from './projectStore';
import { TaskStore } from './taskStore';

export class RendererStores {
  projects = new ProjectStore();
  tasksByProject = new Map<string, TaskStore>();

  constructor() {
    // The `tasksByProject` Map is intentionally non-observable: callers
    // observe individual `TaskStore` instances. `getOrCreateTaskStore`
    // mutates the Map, which never triggers UI churn (only the store
    // it returns is observable).
    makeAutoObservable(this, { tasksByProject: false });
  }

  getOrCreateTaskStore(projectId: string): TaskStore {
    let store = this.tasksByProject.get(projectId);
    if (!store) {
      store = new TaskStore(projectId);
      this.tasksByProject.set(projectId, store);
    }
    return store;
  }
}

export function createStores(): RendererStores {
  return new RendererStores();
}
