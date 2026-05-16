export type InfraProvider = 'local' | 'project-ssh' | 'byoi';

export type InfraResolution =
  | { kind: 'ready' }
  | { kind: 'needs_create' }
  | {
      kind: 'branch_elsewhere';
      workspaceBranch: string;
      candidatePath: string;
      previousPath: string;
    }
  | { kind: 'path_missing'; previousPath: string; workspaceBranch: string | null };
