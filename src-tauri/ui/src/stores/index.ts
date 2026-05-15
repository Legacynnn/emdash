/**
 * Renderer store registry. A single instance per app-root mount is
 * sufficient — there's no need for React context until a second
 * consumer needs the same store from a sibling tree.
 */
import { ProjectStore } from './projectStore';

export interface RendererStores {
  projects: ProjectStore;
}

export function createStores(): RendererStores {
  return {
    projects: new ProjectStore(),
  };
}
