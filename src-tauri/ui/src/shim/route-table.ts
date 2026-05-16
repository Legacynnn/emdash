// Routing table mapping Electron renderer RPC channels (`namespace.method`)
// to Tauri command names + arg adapters.
//
// The renderer's RPC client serializes calls as `${namespace}.${method}`
// (see `src/shared/ipc/rpc.ts` createRPCClient). The Tauri side uses
// snake_case command names with named-arg objects. This table translates.
//
// Three route shapes:
//   - `invoke`: forward to a Tauri command. `adapt(args)` converts
//     Electron-positional args into Tauri's named-args object.
//   - `static`: return a fixed value without IPC. Used to keep the
//     renderer from crashing on startup for surfaces whose Tauri-side
//     command has not been written yet. Each static entry below has a
//     TODO note pointing to the Rust module that should back it.
//   - `custom`: arbitrary handler. Used when the result needs
//     post-processing (e.g. JSON.parse for view state) or when the
//     renderer expects a different value shape than Tauri returns.
//
// Anything not in this table throws TauriShimNotImplementedError so
// the missing route is loud, not silent.

import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export type ElectronArgs = unknown[];

export type Route =
  | {
      kind: 'invoke';
      command: string;
      adapt?: (args: ElectronArgs) => Record<string, unknown> | undefined;
      // Optional post-processor for the Tauri response. Used when the
      // renderer expects a different shape (camelCase, Result envelope,
      // etc.) than the Tauri command returns.
      transform?: (result: unknown, args: ElectronArgs) => unknown;
    }
  | {
      kind: 'static';
      value: unknown;
    }
  | {
      kind: 'custom';
      handler: (args: ElectronArgs) => Promise<unknown>;
    };

const noArgs = (): undefined => undefined;

const toRecord =
  (...keys: string[]) =>
  (args: ElectronArgs): Record<string, unknown> => {
    const out: Record<string, unknown> = {};
    keys.forEach((key, i) => {
      out[key] = args[i];
    });
    return out;
  };

const STATIC_EMPTY_ARRAY: Route = { kind: 'static', value: [] };
const STATIC_NULL: Route = { kind: 'static', value: null };
const STATIC_VOID: Route = { kind: 'static', value: undefined };
const STATIC_EMPTY_OBJECT: Route = { kind: 'static', value: {} };

// Renderer's standard Result envelope used by many controllers (see
// @shared/result). Wrap a successful empty payload so call sites that
// check `result.ok` don't blow up.
const STATIC_RESULT_OK_NULL: Route = {
  kind: 'static',
  value: { ok: true, value: null },
};

// -- Routes ----------------------------------------------------------

const ROUTES: Record<string, Route> = {
  // == viewState ====================================================
  // Tauri stores values as JSON strings; the renderer expects parsed
  // values back. Custom handlers parse on read, stringify on write.
  'viewState.get': {
    kind: 'custom',
    handler: async ([key]) => {
      const raw = (await tauriInvoke('view_state_get', { key })) as string | null;
      if (raw == null) return null;
      try {
        return JSON.parse(raw);
      } catch {
        console.warn('[tauri-shim] viewState.get: malformed JSON, returning raw string', { key });
        return raw;
      }
    },
  },
  'viewState.getAll': {
    kind: 'custom',
    handler: async () => {
      const raw = (await tauriInvoke('view_state_get_all')) as Record<string, string>;
      const out: Record<string, unknown> = {};
      for (const [key, value] of Object.entries(raw ?? {})) {
        try {
          out[key] = JSON.parse(value);
        } catch {
          out[key] = value;
        }
      }
      return out;
    },
  },
  'viewState.save': {
    kind: 'invoke',
    command: 'view_state_save',
    adapt: ([key, value]) => ({ key, valueJson: JSON.stringify(value) }),
  },
  'viewState.reset': { kind: 'invoke', command: 'view_state_reset', adapt: noArgs },

  // == projects =====================================================
  // Renderer uses verbose method names (getProjects, createProject) —
  // map to Tauri's narrower v1 surface. The Tauri Project shape
  // (snake_case + bare id/name/path/timestamps) is mapped onto the
  // renderer's LocalProject discriminated union here so call sites
  // (`createUnmountedProject`, `isMountedProject`, etc.) keep working.
  'projects.getProjects': {
    kind: 'invoke',
    command: 'projects_list',
    transform: (result) => (Array.isArray(result) ? result.map(toRendererProject) : []),
  },
  'projects.createProject': {
    kind: 'invoke',
    command: 'projects_add',
    adapt: ([input]) => {
      // Electron's createProject likely takes an object with at least
      // `path`. Accept either positional path or input.path.
      if (typeof input === 'string') return { path: input };
      const obj = input as { path?: string } | undefined;
      return { path: obj?.path ?? '' };
    },
    transform: (result) => toRendererProject(result as TauriProject),
  },
  'projects.deleteProject': { kind: 'invoke', command: 'projects_remove', adapt: toRecord('id') },
  // The rest of the projects surface (settings, inspection, sharing,
  // open-in-new-window, connection updates) needs Rust ports. Stubs
  // keep the renderer alive when these are called; UI may degrade.
  'projects.openProject': STATIC_VOID,
  'projects.inspectProjectPath': STATIC_RESULT_OK_NULL,
  'projects.getProjectSettingsPage': STATIC_NULL,
  'projects.updateProjectSettings': STATIC_RESULT_OK_NULL,
  'projects.updateProjectConnection': STATIC_RESULT_OK_NULL,
  'projects.shareProjectSettingsToConfig': STATIC_RESULT_OK_NULL,

  // == tasks ========================================================
  'tasks.createTask': {
    kind: 'invoke',
    command: 'tasks_create',
    adapt: ([input]) => {
      // Electron likely passes { projectId, name, sourceBranch }.
      const obj = (input as Record<string, unknown>) ?? {};
      return {
        projectId: obj.projectId,
        name: obj.name,
        sourceBranch: obj.sourceBranch ?? { type: 'local', branch: 'main' },
      };
    },
  },
  'tasks.deleteTask': { kind: 'invoke', command: 'tasks_delete', adapt: toRecord('id') },
  'tasks.archiveTask': {
    kind: 'invoke',
    command: 'tasks_archive',
    adapt: ([id]) => ({ id }),
    transform: (value) => ({ ok: true, value }),
  },
  'tasks.restoreTask': {
    kind: 'invoke',
    command: 'tasks_restore',
    adapt: ([id]) => ({ id }),
    transform: (value) => ({ ok: true, value }),
  },
  'tasks.renameTask': {
    kind: 'invoke',
    command: 'tasks_rename',
    adapt: ([id, name]) => ({ id, name }),
    transform: (value) => ({ ok: true, value }),
  },
  'tasks.generateTaskName': {
    kind: 'invoke',
    command: 'tasks_generate_name',
    adapt: ([description]) => ({ description: description ?? '' }),
    transform: (value) => ({ ok: true, value }),
  },
  // Provisioning is implicit in Tauri's tasks_create (worktree is
  // created atomically). The renderer's provisionTask is a separate
  // step on Electron; here it's a no-op success.
  'tasks.provisionTask': STATIC_RESULT_OK_NULL,
  'tasks.setTaskPinned': {
    kind: 'invoke',
    command: 'tasks_set_pinned',
    adapt: ([id, pinned]) => ({ id, pinned }),
  },
  // updateTaskStatus on Electron mutates a richer status enum
  // (provisioning/running/etc.); Tauri's tasks have only active /
  // archived, so we route this to archive/restore based on the
  // status string the caller passes.
  'tasks.updateTaskStatus': {
    kind: 'custom',
    handler: async ([id, status]) => {
      const next = typeof status === 'string' ? status : '';
      try {
        const value = await tauriInvoke(
          next === 'archived' ? 'tasks_archive' : 'tasks_restore',
          { id }
        );
        return { ok: true, value };
      } catch (e) {
        return { ok: false, error: e };
      }
    },
  },
  'tasks.updateLinkedIssue': {
    kind: 'invoke',
    command: 'tasks_update_linked_issue',
    adapt: ([id, linkedIssue]) => ({ id, linkedIssue }),
  },
  // No project_settings.workspace_settings layer in Tauri yet; the
  // renderer's call site coalesces null into defaults.
  'tasks.getWorkspaceSettings': STATIC_NULL,

  // == app ==========================================================
  // Real Rust implementations under `src-tauri/src/commands/app.rs`.
  // The renderer historically returned plain values (no Result
  // envelope) from these, so we pass invoke results straight through.
  'app.getAppVersion': { kind: 'invoke', command: 'app_get_version', adapt: noArgs },
  'app.getPlatform': { kind: 'invoke', command: 'app_get_platform', adapt: noArgs },
  'app.checkInstalledApps': {
    kind: 'invoke',
    command: 'app_check_installed_apps',
    adapt: ([candidates]) => ({ candidates: Array.isArray(candidates) ? candidates : [] }),
  },
  'app.listInstalledFonts': { kind: 'invoke', command: 'app_list_installed_fonts', adapt: noArgs },
  'app.openExternal': {
    kind: 'invoke',
    command: 'app_open_external',
    adapt: ([target]) => ({ target }),
  },
  'app.openIn': {
    kind: 'invoke',
    command: 'app_open_in',
    adapt: ([path, app]) => ({ path, app }),
  },
  'app.openSelectDirectoryDialog': {
    kind: 'invoke',
    command: 'app_open_select_directory_dialog',
    adapt: noArgs,
  },

  // == update =======================================================
  'update.check': {
    kind: 'invoke',
    command: 'updater_check',
    adapt: ([reason]) => ({ reason: (reason as string) ?? 'manual' }),
  },
  // Tauri's updater plugin isn't fully wired (EMD-22). Fall back to
  // no-ops so the update store can complete its lifecycle without
  // throwing.
  'update.download': STATIC_VOID,
  'update.quitAndInstall': STATIC_VOID,
  'update.openLatest': STATIC_VOID,

  // == resourceMonitor ==============================================
  // TODO: port from src/main/core/resource-monitor. The store unwraps
  // `{ success: true, data }` and reads cpuCount/app/entries; an
  // unsuccessful Result is treated as "no sample yet" by the store.
  'resourceMonitor.getSnapshot': {
    kind: 'static',
    value: { success: false },
  },

  // == telemetry ====================================================
  'telemetry.getStatus': {
    kind: 'custom',
    handler: async () => {
      try {
        const enabled = (await tauriInvoke('telemetry_get_enabled')) as boolean;
        return { enabled };
      } catch {
        return { enabled: false };
      }
    },
  },
  'telemetry.setEnabled': {
    kind: 'invoke',
    command: 'telemetry_set_enabled',
    adapt: toRecord('enabled'),
  },
  'telemetry.getFeatureFlags': STATIC_EMPTY_OBJECT,

  // == editorBuffer =================================================
  'editorBuffer.saveBuffer': {
    kind: 'invoke',
    command: 'editor_buffer_save',
    adapt: ([projectId, workspaceId, filePath, content]) => ({
      projectId,
      workspaceId,
      filePath,
      content,
    }),
  },
  'editorBuffer.clearBuffer': {
    kind: 'invoke',
    command: 'editor_buffer_clear',
    adapt: ([projectId, workspaceId, filePath]) => ({ projectId, workspaceId, filePath }),
  },
  'editorBuffer.listBuffers': {
    kind: 'invoke',
    command: 'editor_buffer_list',
    adapt: ([projectId, workspaceId]) => ({ projectId, workspaceId }),
  },

  // == github =======================================================
  // The Tauri side ships a narrower API than the Electron one. Map
  // what's available; stub the rest so the GitHub surface degrades to
  // "signed out".
  'github.getStatus': {
    kind: 'custom',
    handler: async () => {
      try {
        const me = (await tauriInvoke('github_me')) as Record<string, unknown> | null;
        return me ? { signedIn: true, user: me } : { signedIn: false };
      } catch {
        return { signedIn: false };
      }
    },
  },
  'github.logout': { kind: 'invoke', command: 'github_sign_out' },
  'github.getOwners': STATIC_EMPTY_ARRAY,
  'github.cloneRepository': STATIC_RESULT_OK_NULL,
  'github.createRepository': STATIC_RESULT_OK_NULL,
  'github.initializeProject': STATIC_RESULT_OK_NULL,
  'github.connectOAuth': STATIC_RESULT_OK_NULL,
  'github.auth': STATIC_RESULT_OK_NULL,
  'github.authCancel': STATIC_VOID,

  // == linear =======================================================
  'linear.saveToken': {
    kind: 'invoke',
    command: 'linear_sign_in',
    adapt: toRecord('token'),
  },
  'linear.clearToken': { kind: 'invoke', command: 'linear_sign_out' },

  // == account ======================================================
  'account.getSession': STATIC_NULL,
  'account.checkHealth': { kind: 'static', value: { ok: true } },
  'account.signIn': STATIC_RESULT_OK_NULL,
  'account.signOut': STATIC_VOID,

  // == appSettings ==================================================
  // TODO: port from src/main/core/settings. Returning an empty
  // settings bag is safe — most call sites coalesce with defaults.
  'appSettings.get': STATIC_NULL,
  'appSettings.getWithMeta': STATIC_NULL,
  'appSettings.update': STATIC_RESULT_OK_NULL,
  'appSettings.reset': STATIC_VOID,
  'appSettings.resetField': STATIC_VOID,

  // == providerSettings =============================================
  'providerSettings.getItemWithMeta': STATIC_NULL,
  'providerSettings.updateItem': STATIC_RESULT_OK_NULL,
  'providerSettings.resetItem': STATIC_VOID,

  // == workspaces ===================================================
  // Tauri's `tasks_create` is atomic: the worktree is created as part
  // of the same call. From the renderer's standpoint, every task we
  // can navigate to is `{ kind: 'ready' }` — there's no separate
  // bootstrap step. adoptWorktree/createWorktree become no-ops because
  // the renderer would only call them on an unprovisioned task and
  // Tauri doesn't surface that state.
  'workspaces.resolveBootstrap': { kind: 'static', value: { kind: 'ready' } },
  'workspaces.adoptWorktree': STATIC_RESULT_OK_NULL,
  'workspaces.createWorktree': STATIC_RESULT_OK_NULL,

  // == conversations ================================================
  // Tauri-side service in `src/conversations` + glue in
  // `commands/conversations`. Renderer's call shapes:
  //   getConversationsForTask(taskId) → Result<Conversation[]>
  //   createConversation(params) → Result<Conversation>
  // The Tauri error envelope is exposed verbatim; the renderer's
  // `rpc` helper normalizes `{code, message}` rejections into the
  // standard Result envelope, so no transform is needed here.
  'conversations.getConversationsForTask': {
    kind: 'invoke',
    command: 'conversations_list_for_task',
    adapt: ([taskId]) => ({ taskId }),
    transform: (value) => ({ ok: true, value }),
  },
  'conversations.createConversation': {
    kind: 'invoke',
    command: 'conversations_create',
    adapt: ([params]) => ({ input: params }),
    transform: (value) => ({ ok: true, value }),
  },
  'conversations.deleteConversation': {
    kind: 'invoke',
    command: 'conversations_delete',
    adapt: ([id]) => ({ id }),
  },
  'conversations.renameConversation': {
    kind: 'invoke',
    command: 'conversations_rename',
    adapt: ([id, title]) => ({ id, title }),
  },
  'conversations.touchConversation': {
    kind: 'invoke',
    command: 'conversations_touch',
    adapt: ([id]) => ({ id }),
  },

  // == terminals ====================================================
  // Tauri-side service in `src/terminals` + glue in
  // `commands/terminals`.
  'terminals.getTerminalsForTask': {
    kind: 'invoke',
    command: 'terminals_list_for_task',
    adapt: ([taskId]) => ({ taskId }),
    transform: (value) => ({ ok: true, value }),
  },
  'terminals.createTerminal': {
    kind: 'invoke',
    command: 'terminals_create',
    adapt: ([params]) => ({ input: params }),
    transform: (value) => ({ ok: true, value }),
  },
  'terminals.deleteTerminal': {
    kind: 'invoke',
    command: 'terminals_delete',
    adapt: ([id]) => ({ id }),
  },
  'terminals.renameTerminal': {
    kind: 'invoke',
    command: 'terminals_rename',
    adapt: ([id, name]) => ({ id, name }),
  },

  // == git ==========================================================
  // TODO: port from src/main/core/git. Returning empty/idle states
  // means the diff/status panels will look "clean" until ports land.
  'git.getFullStatus': STATIC_NULL,
  'git.getChangedFiles': STATIC_EMPTY_ARRAY,
  'git.getLog': STATIC_EMPTY_ARRAY,
  'git.getFileAtIndex': STATIC_NULL,
  'git.getFileAtRef': STATIC_NULL,
  'git.getImageAtIndex': STATIC_NULL,
  'git.getImageAtRef': STATIC_NULL,
  'git.commit': STATIC_RESULT_OK_NULL,
  'git.push': STATIC_RESULT_OK_NULL,
  'git.pull': STATIC_RESULT_OK_NULL,
  'git.publishBranch': STATIC_RESULT_OK_NULL,
  'git.stageFiles': STATIC_VOID,
  'git.stageAllFiles': STATIC_VOID,
  'git.unstageFiles': STATIC_VOID,
  'git.unstageAllFiles': STATIC_VOID,
  'git.revertFiles': STATIC_VOID,
  'git.revertAllFiles': STATIC_VOID,

  // == repository ===================================================
  'repository.fetch': STATIC_RESULT_OK_NULL,
  'repository.fetchPrForReview': STATIC_RESULT_OK_NULL,
  'repository.addRemote': STATIC_RESULT_OK_NULL,
  'repository.getLocalBranches': STATIC_EMPTY_ARRAY,
  'repository.getRemoteBranches': STATIC_EMPTY_ARRAY,

  // == pullRequests =================================================
  'pullRequests.listPullRequests': STATIC_EMPTY_ARRAY,
  'pullRequests.getPullRequestsForTask': STATIC_EMPTY_ARRAY,
  'pullRequests.getPullRequestComments': STATIC_EMPTY_ARRAY,
  'pullRequests.getFilterOptions': STATIC_EMPTY_OBJECT,
  'pullRequests.refreshPullRequest': STATIC_VOID,
  'pullRequests.syncPullRequests': STATIC_VOID,
  'pullRequests.forceFullSyncPullRequests': STATIC_VOID,
  'pullRequests.cancelSync': STATIC_VOID,
  'pullRequests.syncChecks': STATIC_VOID,
  'pullRequests.createPullRequest': STATIC_RESULT_OK_NULL,
  'pullRequests.mergePullRequest': STATIC_RESULT_OK_NULL,
  'pullRequests.markReadyForReview': STATIC_RESULT_OK_NULL,

  // == fs ===========================================================
  // TODO: port reads through tauri-plugin-fs; for now stubs keep the
  // renderer's file-tree / image-loaders from throwing.
  'fs.readFile': STATIC_NULL,
  'fs.writeFile': STATIC_VOID,
  'fs.readImage': STATIC_NULL,
  'fs.listFiles': STATIC_EMPTY_ARRAY,
  'fs.watchSetPaths': STATIC_VOID,
  'fs.watchStop': STATIC_VOID,

  // == pty ==========================================================
  // The renderer's PTY layer subscribes via events and writes via
  // sendInput/resize; for now stub to avoid hard crashes. PTY end-to-end
  // wiring is its own follow-up (Tauri exposes pty_spawn/write/resize
  // as direct commands which the renderer's PTY controller will need
  // to be re-pointed at — separate task).
  'pty.subscribe': STATIC_NULL,
  'pty.unsubscribe': STATIC_VOID,
  'pty.sendInput': STATIC_VOID,
  'pty.resize': STATIC_VOID,
  'pty.uploadFiles': STATIC_RESULT_OK_NULL,

  // == search =======================================================
  'search.commandPalette': STATIC_EMPTY_ARRAY,

  // == skills =======================================================
  'skills.getCatalog': STATIC_EMPTY_ARRAY,
  'skills.getDetail': STATIC_NULL,
  'skills.refreshCatalog': STATIC_VOID,
  'skills.install': STATIC_RESULT_OK_NULL,
  'skills.uninstall': STATIC_RESULT_OK_NULL,
  'skills.create': STATIC_RESULT_OK_NULL,

  // == mcp ==========================================================
  'mcp.loadAll': STATIC_EMPTY_ARRAY,
  'mcp.getProviders': STATIC_EMPTY_ARRAY,
  'mcp.refreshProviders': STATIC_VOID,
  'mcp.saveServer': STATIC_RESULT_OK_NULL,
  'mcp.removeServer': STATIC_VOID,

  // == ssh ==========================================================
  // TODO: port from src/main/core/ssh. SSH connection store calls
  // these on bootstrap; returning empties keeps the start() chain
  // happy until the Rust ssh module lands.
  'ssh.getConnections': STATIC_EMPTY_ARRAY,
  'ssh.getConnectionState': STATIC_EMPTY_OBJECT,
  'ssh.getConnectionUsage': STATIC_EMPTY_OBJECT,
  'ssh.getHealthStates': STATIC_EMPTY_OBJECT,
  'ssh.connect': STATIC_RESULT_OK_NULL,
  'ssh.testConnection': STATIC_RESULT_OK_NULL,
  'ssh.saveConnection': STATIC_RESULT_OK_NULL,
  'ssh.renameConnection': STATIC_VOID,
  'ssh.deleteConnection': STATIC_VOID,

  // == dependencies =================================================
  // The dependencies store probes which agent CLIs are installed on
  // PATH. Returning an empty record means "none detected"; the UI
  // will surface install prompts.
  'dependencies.getAll': STATIC_EMPTY_OBJECT,
  'dependencies.probeAll': STATIC_VOID,
  'dependencies.probeCategory': STATIC_VOID,
  'dependencies.install': STATIC_RESULT_OK_NULL,

  // == issues / forgejo / gitlab / jira / plain / featurebase =======
  'issues.listIssues': STATIC_EMPTY_ARRAY,
  'issues.searchIssues': STATIC_EMPTY_ARRAY,
  'issues.checkAllConnections': STATIC_EMPTY_OBJECT,
  'forgejo.saveCredentials': STATIC_RESULT_OK_NULL,
  'forgejo.clearCredentials': STATIC_VOID,
  'gitlab.saveCredentials': STATIC_RESULT_OK_NULL,
  'gitlab.clearCredentials': STATIC_VOID,
  'jira.saveCredentials': STATIC_RESULT_OK_NULL,
  'jira.clearCredentials': STATIC_VOID,
  'plain.saveToken': STATIC_RESULT_OK_NULL,
  'plain.clearToken': STATIC_VOID,
  'featurebase.saveToken': STATIC_RESULT_OK_NULL,
  'featurebase.clearToken': STATIC_VOID,

  // == legacyPort ===================================================
  // Electron-only DB migration helper. Tauri builds use a different
  // db file path so a no-op is correct here.
  'legacyPort.checkStatus': { kind: 'static', value: { needed: false } },
  'legacyPort.getPreview': { kind: 'static', value: { projects: [], tasks: [] } },
  'legacyPort.runImport': STATIC_RESULT_OK_NULL,
};

// Tauri's Project shape (from src-tauri/ui/src/bindings.ts) uses
// snake_case and only carries the bare CRUD columns. The renderer
// expects a discriminated union with `type: 'local' | 'ssh'` and
// camelCase timestamps. Until the Rust side adopts the richer shape,
// this adapter projects every Tauri row into a LocalProject so the
// renderer's project store stops at the v1 surface.
interface TauriProject {
  id: string;
  name: string;
  path: string;
  created_at: string;
  updated_at: string;
}

function toRendererProject(p: TauriProject): Record<string, unknown> {
  return {
    type: 'local',
    id: p.id,
    name: p.name,
    path: p.path,
    baseRef: 'main',
    createdAt: p.created_at,
    updatedAt: p.updated_at,
  };
}

export function resolveRoute(channel: string): Route | null {
  return ROUTES[channel] ?? null;
}

export function registerRoute(channel: string, route: Route): void {
  ROUTES[channel] = route;
}
