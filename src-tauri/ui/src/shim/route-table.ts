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
import { emitToBus } from './event-bridge';
import { ptySessionMap } from './pty-session-map';

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

// -- GitHub auth wiring ---------------------------------------------

// Channel names must match `src/shared/events/githubEvents.ts`.
const GITHUB_AUTH_DEVICE_CODE_CHANNEL = 'github:auth:device-code';
const GITHUB_AUTH_SUCCESS_CHANNEL = 'github:auth:success';
const GITHUB_AUTH_ERROR_CHANNEL = 'github:auth:error';

// Subset of the Rust `DeviceFlowStart` struct we read. Keep snake_case
// so the cast from `tauriInvoke` doesn't need a transform layer.
interface DeviceFlowStartPayload {
  user_code: string;
  verification_uri: string;
  device_code: string;
  polling_interval_seconds: number;
  expires_in_seconds: number;
}

interface IdentityRecord {
  login: string;
  id: string;
  name: string | null;
  email: string | null;
  avatar_url: string | null;
  token_source: 'secure_storage' | 'cli';
}

function identityToGitHubUser(identity: IdentityRecord): {
  id: number;
  login: string;
  name: string;
  email: string;
  avatar_url: string;
} {
  const parsedId = Number(identity.id);
  return {
    id: Number.isFinite(parsedId) ? parsedId : 0,
    login: identity.login,
    name: identity.name ?? identity.login,
    email: identity.email ?? '',
    avatar_url: identity.avatar_url ?? '',
  };
}

function normalizeGithubCommandError(err: unknown): { code: string; message: string } {
  if (err && typeof err === 'object') {
    const e = err as { code?: unknown; message?: unknown };
    if (typeof e.code === 'string' || typeof e.message === 'string') {
      return {
        code: typeof e.code === 'string' ? e.code : 'unknown',
        message: typeof e.message === 'string' ? e.message : String(err),
      };
    }
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

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
      // Electron's createProject takes either a bare path string or an
      // object with at least `path` (and usually `id` — the renderer's
      // optimistic-UI uuid). Forward `id` so the DB primary key matches
      // the renderer's map key; without that, the renderer stores the
      // project store under the local uuid while `data.id` carries the
      // server-allocated uuid, and downstream lookups by `data.id`
      // (sidebar navigation, task store keys) miss the entry.
      if (typeof input === 'string') return { path: input, id: null };
      const obj = (input as { path?: string; id?: string } | undefined) ?? {};
      return { path: obj.path ?? '', id: obj.id ?? null };
    },
    transform: (result) => toRendererProject(result as TauriProject),
  },
  'projects.deleteProject': { kind: 'invoke', command: 'projects_remove', adapt: toRecord('id') },
  // The rest of the projects surface (settings, inspection, sharing,
  // open-in-new-window, connection updates) needs Rust ports. Stubs
  // keep the renderer alive when these are called; UI may degrade.
  // openProject uses a `{ success, error }` envelope (not the standard
  // Result); the renderer mounts the project optimistically and reads
  // no other fields off the success branch, so an empty success is
  // safe until a real Tauri-side open/validate command lands.
  'projects.openProject': { kind: 'static', value: { success: true } },
  'projects.inspectProjectPath': STATIC_RESULT_OK_NULL,
  'projects.getProjectSettingsPage': STATIC_NULL,
  'projects.updateProjectSettings': STATIC_RESULT_OK_NULL,
  'projects.updateProjectConnection': STATIC_RESULT_OK_NULL,
  'projects.shareProjectSettingsToConfig': STATIC_RESULT_OK_NULL,

  // == workspaces ===================================================
  'workspaces.createWorkspace': {
    kind: 'invoke',
    command: 'workspaces_create',
    adapt: ([input]) => {
      // The renderer passes a strategy object that encodes both the
      // branch name and the placement mode (worktree vs. local).
      // `new-local` / `existing-local` route to `placement: 'local'`
      // in the Rust backend; `new-branch` / `from-pull-request` stay
      // on `placement: 'worktree'`.
      const obj = (input as Record<string, unknown>) ?? {};
      const strategy = obj.strategy as { kind?: string; workspaceBranch?: string } | undefined;
      const kind = strategy?.kind;
      const placement: 'local' | 'worktree' =
        kind === 'new-local' || kind === 'existing-local' ? 'local' : 'worktree';
      const existingBranch = kind === 'existing-local';
      const workspaceBranch =
        kind === 'new-branch' ||
        kind === 'from-pull-request' ||
        kind === 'new-local' ||
        kind === 'existing-local'
          ? (strategy?.workspaceBranch ?? null)
          : null;
      return {
        projectId: obj.projectId,
        name: obj.name,
        sourceBranch: obj.sourceBranch ?? { type: 'local', branch: 'main' },
        workspaceBranch,
        placement,
        existingBranch,
      };
    },
  },
  'workspaces.deleteWorkspace': {
    kind: 'invoke',
    command: 'workspaces_delete',
    adapt: toRecord('id'),
  },
  'workspaces.archiveWorkspace': {
    kind: 'invoke',
    command: 'workspaces_archive',
    adapt: ([, id]) => ({ id }),
    transform: (value) => ({ ok: true, value }),
  },
  'workspaces.restoreWorkspace': {
    kind: 'invoke',
    command: 'workspaces_restore',
    adapt: ([id]) => ({ id }),
    transform: (value) => ({ ok: true, value }),
  },
  'workspaces.renameWorkspace': {
    kind: 'invoke',
    command: 'workspaces_rename',
    adapt: ([, id, name]) => ({ id, name }),
    transform: (value) => ({ ok: true, value }),
  },
  'workspaces.generateWorkspaceName': {
    kind: 'invoke',
    command: 'workspaces_generate_name',
    adapt: ([description]) => ({ description: description ?? '' }),
    transform: (value) => ({ ok: true, value }),
  },
  // Provisioning is implicit in Tauri's workspaces_create (worktree is
  // created atomically). The renderer's provisionWorkspace is a separate
  // step on Electron; here it's a no-op success.
  'workspaces.provisionWorkspace': STATIC_RESULT_OK_NULL,
  'workspaces.setWorkspacePinned': {
    kind: 'invoke',
    command: 'workspaces_set_pinned',
    adapt: ([id, pinned]) => ({ id, pinned }),
  },
  // updateWorkspaceStatus on Electron mutates a richer status enum
  // (provisioning/running/etc.); Tauri's workspaces have only active /
  // archived, so we route this to archive/restore based on the
  // status string the caller passes.
  'workspaces.updateWorkspaceStatus': {
    kind: 'custom',
    handler: async ([id, status]) => {
      const next = typeof status === 'string' ? status : '';
      try {
        const value = await tauriInvoke(
          next === 'archived' ? 'workspaces_archive' : 'workspaces_restore',
          { id }
        );
        return { ok: true, value };
      } catch (e) {
        return { ok: false, error: e };
      }
    },
  },
  'workspaces.updateLinkedIssue': {
    kind: 'invoke',
    command: 'workspaces_update_linked_issue',
    adapt: ([id, linkedIssue]) => ({ id, linkedIssue }),
  },
  // No project_settings.workspace_settings layer in Tauri yet; the
  // renderer's call site coalesces null into defaults.
  'workspaces.getWorkspaceSettings': STATIC_NULL,

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
  // Host returns `{ success: false }` until a real per-PTY sampler
  // lands. The renderer's store treats that as "no sample yet" and
  // keeps the badge idle.
  'resourceMonitor.getSnapshot': {
    kind: 'invoke',
    command: 'resource_monitor_get_snapshot',
    adapt: noArgs,
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
  // Two auth paths are wired: device flow (`github.auth`) and the
  // gh-CLI fast path (`github.signInViaGhCli`). OAuth via Emdash
  // account stays stubbed per ADR-0010 — the backend doesn't exist.
  'github.getStatus': {
    kind: 'custom',
    handler: async () => {
      try {
        const me = (await tauriInvoke('github_me')) as IdentityRecord | null;
        if (!me) {
          return { authenticated: false, user: null, tokenSource: null };
        }
        return {
          authenticated: true,
          user: identityToGitHubUser(me),
          tokenSource: me.token_source,
        };
      } catch {
        return { authenticated: false, user: null, tokenSource: null };
      }
    },
  },
  'github.auth': {
    kind: 'custom',
    handler: async () => {
      // Device-flow driver. The Rust poll command blocks until token
      // or terminal error, so the renderer awaits one call instead of
      // running its own timer. We emit synthetic events so the modal +
      // context provider can react via their existing channel
      // subscriptions.
      try {
        const start = (await tauriInvoke(
          'github_sign_in_device_flow_start'
        )) as DeviceFlowStartPayload;
        emitToBus(GITHUB_AUTH_DEVICE_CODE_CHANNEL, {
          userCode: start.user_code,
          verificationUri: start.verification_uri,
          expiresIn: start.expires_in_seconds,
          interval: start.polling_interval_seconds,
        });
        const identity = (await tauriInvoke('github_sign_in_device_flow_poll', {
          flow: {
            device_code: start.device_code,
            polling_interval_seconds: start.polling_interval_seconds,
          },
        })) as IdentityRecord;
        emitToBus(GITHUB_AUTH_SUCCESS_CHANNEL, {
          token: '',
          user: identityToGitHubUser(identity),
        });
        return { ok: true, value: null };
      } catch (err) {
        const { code, message } = normalizeGithubCommandError(err);
        emitToBus(GITHUB_AUTH_ERROR_CHANNEL, { error: code, message });
        return { ok: false, error: { code, message } };
      }
    },
  },
  'github.signInViaGhCli': { kind: 'invoke', command: 'github_sign_in_via_gh_cli' },
  'github.logout': { kind: 'invoke', command: 'github_sign_out' },
  'github.getOwners': STATIC_EMPTY_ARRAY,
  'github.cloneRepository': STATIC_RESULT_OK_NULL,
  'github.createRepository': STATIC_RESULT_OK_NULL,
  'github.initializeProject': STATIC_RESULT_OK_NULL,
  // Cancel is renderer-only: the device-flow modal stops listening on
  // unmount and the Rust poll runs harmlessly until the code expires.
  'github.authCancel': STATIC_VOID,

  // == linear =======================================================
  'linear.saveToken': {
    kind: 'invoke',
    command: 'linear_sign_in',
    adapt: toRecord('token'),
  },
  'linear.clearToken': { kind: 'invoke', command: 'linear_sign_out' },

  // The `account.*` family is intentionally absent — ADR-0010 deferred
  // the Emdash account subsystem indefinitely. Renderer talks to GitHub
  // directly via the device-flow and gh-CLI paths above.

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
  //   getConversationsForWorkspace(projectId, workspaceId) → Result<Conversation[]>
  //   createConversation(params) → Result<Conversation>
  // The Tauri error envelope is exposed verbatim; the renderer's
  // `rpc` helper normalizes `{code, message}` rejections into the
  // standard Result envelope, so no transform is needed here.
  'conversations.getConversationsForWorkspace': {
    kind: 'invoke',
    command: 'conversations_list_for_workspace',
    adapt: ([, workspaceId]) => ({ workspaceId }),
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
  'terminals.getTerminalsForWorkspace': {
    kind: 'invoke',
    command: 'terminals_list_for_workspace',
    adapt: ([, workspaceId]) => ({ workspaceId }),
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
  // Read-only ops backed by `commands::git` (libgit2 via `git2`).
  // Workspace IDs resolve to a worktree path in Rust via the tasks
  // table, so the shim only needs to forward the workspace_id.
  // Mutating ops (commit/push/pull/stage/revert) still stubbed —
  // see docs/migration/tauri-renderer-wire.md.
  'git.getFullStatus': {
    kind: 'invoke',
    command: 'git_full_status',
    adapt: ([, workspaceId]) => ({ workspaceId }),
    transform: (value) => ({ ok: true, value }),
  },
  'git.getChangedFiles': {
    kind: 'invoke',
    command: 'git_changed_files',
    adapt: ([, workspaceId]) => ({ workspaceId }),
  },
  // The renderer's git log surface expects {oid, author, message, ...}
  // entries; the bare `commit_message(oid)` accessor isn't enough on
  // its own. Wire log in a follow-up once a domain `git::log` helper
  // ships.
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
  'repository.getLocalBranches': {
    kind: 'invoke',
    command: 'git_local_branches',
    adapt: ([projectId, workspaceId]) => ({ projectId, workspaceId }),
  },
  'repository.getRemoteBranches': {
    kind: 'invoke',
    command: 'git_remote_branches',
    adapt: ([projectId, workspaceId]) => ({ projectId, workspaceId }),
  },

  // == pullRequests =================================================
  // Read-side wired to real (currently empty) Rust commands; mutation
  // ops stay stubbed until the github commands grow create/merge.
  'pullRequests.listPullRequests': { kind: 'invoke', command: 'pull_requests_list', adapt: noArgs },
  'pullRequests.getPullRequestsForWorkspace': {
    kind: 'invoke',
    command: 'pull_requests_for_workspace',
    adapt: ([, workspaceId]) => ({ workspaceId }),
  },
  'pullRequests.getPullRequestComments': {
    kind: 'invoke',
    command: 'pull_requests_get_comments',
    adapt: ([repo, number]) => ({ repo, number }),
  },
  'pullRequests.getFilterOptions': {
    kind: 'invoke',
    command: 'pull_requests_get_filter_options',
    adapt: noArgs,
  },
  'pullRequests.refreshPullRequest': STATIC_VOID,
  'pullRequests.syncPullRequests': STATIC_VOID,
  'pullRequests.forceFullSyncPullRequests': STATIC_VOID,
  'pullRequests.cancelSync': STATIC_VOID,
  'pullRequests.syncChecks': STATIC_VOID,
  'pullRequests.createPullRequest': STATIC_RESULT_OK_NULL,
  'pullRequests.mergePullRequest': STATIC_RESULT_OK_NULL,
  'pullRequests.markReadyForReview': STATIC_RESULT_OK_NULL,

  // == fs ===========================================================
  // Tauri commands resolve (projectId, workspaceId) → absolute path
  // via the tasks table (workspace_id == task_id in v1), then read
  // the underlying file. Path-escape (`..` / absolute) is rejected
  // server-side.
  'fs.readFile': {
    kind: 'invoke',
    command: 'fs_ws_read_file',
    adapt: ([projectId, workspaceId, filePath, maxBytes]) => ({
      projectId,
      workspaceId,
      filePath,
      maxBytes: typeof maxBytes === 'number' ? maxBytes : null,
    }),
    transform: (value) => ({ ok: true, value }),
  },
  'fs.writeFile': {
    kind: 'invoke',
    command: 'fs_ws_write_file',
    adapt: ([projectId, workspaceId, filePath, content]) => ({
      projectId,
      workspaceId,
      filePath,
      content,
    }),
    transform: () => ({ ok: true, value: undefined }),
  },
  'fs.readImage': {
    kind: 'invoke',
    command: 'fs_ws_read_image',
    adapt: ([projectId, workspaceId, filePath]) => ({ projectId, workspaceId, filePath }),
    transform: (value) => ({ ok: true, value }),
  },
  'fs.listFiles': {
    kind: 'invoke',
    command: 'fs_ws_list_files',
    adapt: ([projectId, workspaceId, dirPath, options]) => ({
      projectId,
      workspaceId,
      dirPath: dirPath ?? '',
      includeHidden:
        options && typeof options === 'object' && 'includeHidden' in options
          ? Boolean((options as { includeHidden?: boolean }).includeHidden)
          : null,
    }),
    transform: (value) => ({ ok: true, value }),
  },
  'fs.fileExists': {
    kind: 'invoke',
    command: 'fs_ws_file_exists',
    adapt: ([projectId, workspaceId, filePath]) => ({ projectId, workspaceId, filePath }),
    transform: (exists) => ({ ok: true, value: { exists } }),
  },
  'fs.statFile': {
    kind: 'invoke',
    command: 'fs_ws_stat_file',
    adapt: ([projectId, workspaceId, filePath]) => ({ projectId, workspaceId, filePath }),
    transform: (entry) => ({ ok: true, value: { entry } }),
  },
  'fs.removeFile': {
    kind: 'invoke',
    command: 'fs_ws_remove_file',
    adapt: ([projectId, workspaceId, filePath]) => ({ projectId, workspaceId, filePath }),
    transform: () => ({ ok: true, value: undefined }),
  },
  // searchFiles / saveAttachment have no Tauri equivalents yet —
  // empty results keep the file-tree / attachment-picker quiet.
  'fs.searchFiles': { kind: 'static', value: { ok: true, value: [] } },
  'fs.saveAttachment': { kind: 'static', value: { ok: true, value: null } },
  // fs_watcher is wired but the renderer-facing watchSetPaths/watchStop
  // path expects a (projectId, workspaceId, paths[], label) shape — a
  // proper bridge lives in a follow-up; the no-op keeps the renderer's
  // file-tree from throwing.
  'fs.watchSetPaths': { kind: 'static', value: { ok: true, value: { supported: false } } },
  'fs.watchStop': { kind: 'static', value: { ok: true, value: {} } },

  // == pty ==========================================================
  // The renderer's PTY layer subscribes via events and writes via
  // sendInput/resize. Tauri exposes pty_spawn / pty_write / pty_resize
  // / pty_kill as direct commands with a `Channel<Vec<u8>>` for
  // streaming, plus `agents_start` for spawning an agent into a PTY.
  //
  // We maintain a sessionId → PtyId map in the shim so the renderer's
  // existing rpc.pty.* call sites work unchanged. subscribe returns an
  // empty initial ring buffer (Tauri doesn't keep one) — the live byte
  // stream arrives via the `pty:data.<sessionId>` event topic.
  'pty.subscribe': {
    kind: 'custom',
    handler: async ([sessionId]) => {
      // Idempotent: if the session was already registered by an
      // agent-spawn path we keep the map entry. Otherwise we record
      // the sessionId so subsequent sendInput / resize calls can
      // resolve to a PtyId once one is associated.
      if (typeof sessionId === 'string') ptySessionMap.ensure(sessionId);
      return { ok: true, value: { buffer: '' } };
    },
  },
  'pty.unsubscribe': {
    kind: 'custom',
    handler: async ([sessionId]) => {
      if (typeof sessionId === 'string') ptySessionMap.forget(sessionId);
      return { ok: true };
    },
  },
  'pty.sendInput': {
    kind: 'custom',
    handler: async ([sessionId, data]) => {
      const ptyId = typeof sessionId === 'string' ? ptySessionMap.get(sessionId) : undefined;
      if (!ptyId) return { ok: false, error: { type: 'not_found' } };
      const bytes = typeof data === 'string' ? Array.from(new TextEncoder().encode(data)) : data;
      try {
        await tauriInvoke('pty_write', { id: ptyId, bytes });
        return { ok: true };
      } catch (e) {
        return { ok: false, error: e };
      }
    },
  },
  'pty.resize': {
    kind: 'custom',
    handler: async ([sessionId, cols, rows]) => {
      const ptyId = typeof sessionId === 'string' ? ptySessionMap.get(sessionId) : undefined;
      if (!ptyId) return { ok: false, error: { type: 'not_found' } };
      try {
        await tauriInvoke('pty_resize', { id: ptyId, size: { cols, rows } });
        return { ok: true };
      } catch (e) {
        return { ok: false, error: e };
      }
    },
  },
  'pty.kill': {
    kind: 'custom',
    handler: async ([sessionId]) => {
      const ptyId = typeof sessionId === 'string' ? ptySessionMap.get(sessionId) : undefined;
      if (!ptyId) return { ok: true };
      try {
        await tauriInvoke('pty_kill', { id: ptyId });
        ptySessionMap.forget(sessionId as string);
        return { ok: true };
      } catch (e) {
        return { ok: false, error: e };
      }
    },
  },
  // SSH PTY upload — no Tauri equivalent yet. Returning ok([]) lets
  // the conversation flow continue with zero attached files.
  'pty.uploadFiles': { kind: 'static', value: { ok: true, value: [] } },

  // == search =======================================================
  // Host ranks the renderer-provided candidates by substring +
  // start-of-token match. Full cross-corpus search is a follow-up.
  'search.commandPalette': {
    kind: 'invoke',
    command: 'search_command_palette',
    adapt: ([query, items]) => ({
      query: typeof query === 'string' ? query : '',
      items: Array.isArray(items) ? items : [],
    }),
  },

  // == skills =======================================================
  // The renderer (`useSkills.ts`) expects a `{ success, data?, error? }`
  // envelope; Tauri commands return plain values or throw an error
  // envelope, so the handlers below normalise both shapes.
  'skills.getCatalog': {
    kind: 'custom',
    handler: () => skillsEnvelope(() => tauriInvoke('skills_get_catalog')),
  },
  'skills.refreshCatalog': {
    kind: 'custom',
    handler: () => skillsEnvelope(() => tauriInvoke('skills_refresh_catalog')),
  },
  'skills.getDetail': {
    kind: 'custom',
    handler: ([arg]) => {
      const skillId =
        typeof arg === 'string' ? arg : (arg as { skillId?: string } | undefined)?.skillId;
      return skillsEnvelope(() => tauriInvoke('skills_get_detail', { id: skillId ?? '' }));
    },
  },
  'skills.getDetectedAgents': {
    kind: 'invoke',
    command: 'skills_get_detected_agents',
    adapt: noArgs,
  },
  'skills.install': {
    kind: 'custom',
    handler: ([arg]) => {
      const skillId = (arg as { skillId?: string } | undefined)?.skillId ?? '';
      return skillsEnvelope(() => tauriInvoke('skills_install', { skillId }));
    },
  },
  'skills.uninstall': {
    kind: 'custom',
    handler: ([arg]) => {
      const skillId = (arg as { skillId?: string } | undefined)?.skillId ?? '';
      return skillsEnvelope(() => tauriInvoke('skills_uninstall', { skillId }));
    },
  },
  'skills.create': {
    kind: 'custom',
    handler: ([arg]) => {
      const input = (arg as { name?: string; description?: string; content?: string }) ?? {};
      return skillsEnvelope(() =>
        tauriInvoke('skills_create', {
          name: input.name ?? '',
          description: input.description ?? '',
          content: input.content ?? null,
        })
      );
    },
  },

  // == mcp ==========================================================
  // The renderer (`useMcps.ts`) consumes the `{ success, data?, error? }`
  // envelope. The Rust `McpCatalogEntry.defaultConfig` is JSON-stringified
  // (see `mcp::model::McpCatalogEntry`) because specta can't express
  // `serde_json::Value` without tripping its BigInt guard; the loader
  // here parses each entry back into a `Record<string, unknown>`.
  'mcp.loadAll': {
    kind: 'custom',
    handler: () =>
      skillsEnvelope(async () => {
        const raw = (await tauriInvoke('mcp_load_all')) as {
          installed: unknown[];
          catalog: Array<{ defaultConfig: string; [k: string]: unknown }>;
        };
        return {
          installed: raw.installed,
          catalog: raw.catalog.map((entry) => ({
            ...entry,
            defaultConfig: safeParseJsonObject(entry.defaultConfig),
          })),
        };
      }),
  },
  'mcp.getProviders': {
    kind: 'custom',
    handler: () => skillsEnvelope(() => tauriInvoke('mcp_get_providers')),
  },
  'mcp.refreshProviders': {
    kind: 'custom',
    handler: () => skillsEnvelope(() => tauriInvoke('mcp_refresh_providers')),
  },
  'mcp.saveServer': {
    kind: 'custom',
    handler: ([server]) => skillsEnvelope(() => tauriInvoke('mcp_save_server', { server })),
  },
  'mcp.removeServer': {
    kind: 'custom',
    handler: ([serverName]) =>
      skillsEnvelope(() => tauriInvoke('mcp_remove_server', { serverName })),
  },

  // == ssh ==========================================================
  // Tauri's SSH layer is unported — host returns empty registries so
  // the renderer's SSH view degrades to "no remote connections".
  // The local-only flow is fully functional without this.
  'ssh.getConnections': { kind: 'invoke', command: 'ssh_get_connections', adapt: noArgs },
  'ssh.getConnectionState': {
    kind: 'invoke',
    command: 'ssh_get_connection_state',
    adapt: noArgs,
  },
  'ssh.getConnectionUsage': STATIC_EMPTY_OBJECT,
  'ssh.getHealthStates': { kind: 'invoke', command: 'ssh_get_health_states', adapt: noArgs },
  'ssh.connect': STATIC_RESULT_OK_NULL,
  'ssh.testConnection': STATIC_RESULT_OK_NULL,
  'ssh.saveConnection': STATIC_RESULT_OK_NULL,
  'ssh.renameConnection': STATIC_VOID,
  'ssh.deleteConnection': STATIC_VOID,

  // == dependencies =================================================
  // PATH-based probe of known agent CLIs. The Rust DependencyEntry
  // shape (`{ id, installed, resolvedPath }`) is narrower than the
  // renderer's DependencyState (`{ id, category, status, version,
  // path, checkedAt, error? }`), so the transform reshapes each entry
  // into a DependencyState. Rust doesn't probe versions yet, so
  // `version` is always null. Category is derived from a small set of
  // known core deps; anything else is treated as an agent CLI.
  'dependencies.getAll': {
    kind: 'invoke',
    command: 'dependencies_get_all',
    adapt: noArgs,
    transform: (entries) => mapDependencyEntries(entries),
  },
  'dependencies.probeAll': {
    kind: 'invoke',
    command: 'dependencies_get_all',
    adapt: noArgs,
    transform: () => undefined,
  },
  'dependencies.probeCategory': STATIC_VOID,
  // The host has no auto-installer; surface a clean failure so the
  // renderer points the user at install docs.
  'dependencies.install': {
    kind: 'static',
    value: { ok: false, error: { type: 'not_supported' } },
  },

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

// Renderer's DependencyState (see @shared/dependencies). Keep in sync
// with src/shared/dependencies.ts:DependencyCategory.
const CORE_DEPENDENCY_IDS = new Set(['git', 'gh', 'tmux', 'ssh', 'node']);

interface TauriDependencyEntry {
  id: string;
  installed: boolean;
  resolvedPath: string | null;
}

function safeParseJsonObject(raw: unknown): Record<string, unknown> {
  if (typeof raw !== 'string' || raw.length === 0) return {};
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

// Wrap a Tauri invoke into the `{ success, data, error }` envelope the
// skills/mcp surfaces expect. Tauri-side error envelopes carry a
// `{ code, message }` shape (see `commands/skills.rs::SkillsCommandError`);
// other throws are stringified.
async function skillsEnvelope(
  call: () => Promise<unknown>
): Promise<{ success: true; data: unknown } | { success: false; error: string }> {
  try {
    const data = await call();
    return { success: true, data };
  } catch (e) {
    if (e && typeof e === 'object') {
      const env = e as { message?: string; code?: string };
      if (typeof env.message === 'string') {
        return { success: false, error: env.message };
      }
    }
    return { success: false, error: e instanceof Error ? e.message : String(e) };
  }
}

export function mapDependencyEntries(entries: unknown): Record<string, unknown> {
  const arr = Array.isArray(entries) ? entries : [];
  const checkedAt = Date.now();
  const out: Record<string, unknown> = {};
  for (const raw of arr) {
    const entry = raw as Partial<TauriDependencyEntry> | null;
    if (!entry || typeof entry.id !== 'string') continue;
    out[entry.id] = {
      id: entry.id,
      category: CORE_DEPENDENCY_IDS.has(entry.id) ? 'core' : 'agent',
      status: entry.installed ? 'available' : 'missing',
      version: null,
      path: entry.resolvedPath ?? null,
      checkedAt,
    };
  }
  return out;
}

export function resolveRoute(channel: string): Route | null {
  return ROUTES[channel] ?? null;
}

export function registerRoute(channel: string, route: Route): void {
  ROUTES[channel] = route;
}
