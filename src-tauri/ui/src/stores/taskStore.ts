/**
 * MobX store for the tasks of one project. One `TaskStore` instance
 * per project; the parent renderer (e.g. a project detail view) is
 * responsible for `dispose()`ing it when the user navigates away —
 * `dispose()` is cheap (clear the array) and idempotent.
 *
 * State-guard conventions mirror `ProjectStore`: callers use the
 * `taskStoreKind` / `asReady` selectors instead of poking at the
 * variant fields directly.
 */
import { makeAutoObservable, runInAction } from 'mobx';
import { commands, type Task, type TasksCommandError, type TaskSourceBranch } from '../bindings';

export type TasksStoreState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; tasks: Task[] }
  | { kind: 'error'; error: TasksCommandError };

export class TaskStore {
  state: TasksStoreState = { kind: 'idle' };
  readonly projectId: string;

  constructor(projectId: string) {
    this.projectId = projectId;
    makeAutoObservable(this);
  }

  async load(): Promise<void> {
    if (this.state.kind === 'loading') return;
    this.state = { kind: 'loading' };
    const result = await commands.tasksList(this.projectId);
    runInAction(() => {
      if (result.status === 'ok') {
        this.state = { kind: 'ready', tasks: result.data };
      } else {
        this.state = { kind: 'error', error: result.error };
      }
    });
  }

  async create(name: string, sourceBranch: TaskSourceBranch): Promise<Task | TasksCommandError> {
    const result = await commands.tasksCreate(this.projectId, name, sourceBranch);
    return result.status === 'ok' ? result.data : result.error;
  }

  async delete(id: string): Promise<null | TasksCommandError> {
    const result = await commands.tasksDelete(id);
    return result.status === 'ok' ? null : result.error;
  }

  /**
   * Drop a task from the in-memory list. The bridge calls this on
   * `task_deleted` so the UI updates without a server round-trip.
   */
  applyDeleted(id: string): void {
    if (this.state.kind !== 'ready') return;
    this.state = {
      kind: 'ready',
      tasks: this.state.tasks.filter((t) => t.id !== id),
    };
  }
}

export function asReady(store: TaskStore): { tasks: Task[] } | undefined {
  return store.state.kind === 'ready' ? { tasks: store.state.tasks } : undefined;
}

export function taskStoreKind(store: TaskStore): TasksStoreState['kind'] {
  return store.state.kind;
}
