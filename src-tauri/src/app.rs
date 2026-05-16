//! Tauri runtime glue. Domain logic stays in `emdash_dev::*` lib modules.

use std::path::PathBuf;
use std::sync::Arc;

use emdash_dev::agent_hooks::classifier::ClaudeClassifier;
use emdash_dev::agent_hooks::{ClassifierRegistry, HookServer, HookServerHandle};
use emdash_dev::conversations::ConversationsService;
use emdash_dev::db::Db;
use emdash_dev::fs_watcher::WatcherRegistry;
use emdash_dev::projects::ProjectsService;
use emdash_dev::pty::registry::Registry;
use emdash_dev::secrets::{master_key::OsKeyringMasterKey, Secrets};
use emdash_dev::tasks::{TasksService, WorkspaceFsMutationLock};
use emdash_dev::tauri_bindings;
use emdash_dev::terminals::TerminalsService;
use emdash_dev::telemetry::{Telemetry, TelemetryConfig};
use emdash_dev::ui_sync::{UiMutationEvent, UiSyncManager};
use emdash_dev::updater::UpdateManager;
use tauri::{Manager, RunEvent};

pub fn export_bindings_default() -> Result<(), Box<dyn std::error::Error>> {
    let path = format!(
        "{}/{}",
        env!("CARGO_MANIFEST_DIR"),
        tauri_bindings::BINDINGS_PATH
    );
    // Export to a temp path *outside* src-tauri so Tauri's dev watcher
    // doesn't see it, then compare-and-swap into ui/src/bindings.ts.
    // Two-layer fix:
    //   1. Writing in-tree (even a `.tmp` sibling) triggers the watcher.
    //      Use `std::env::temp_dir()` to escape the watched root.
    //   2. mtime-only watching means a content-identical rewrite still
    //      fires the watcher — so only write when the bytes differ.
    let tmp = std::env::temp_dir().join(format!(
        "emdash-dev-bindings-{}-{}.ts",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    tauri_bindings::export_bindings_to(&tmp)?;
    let new = std::fs::read_to_string(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if new != current {
        std::fs::write(&path, new)?;
    }
    Ok(())
}

pub fn run() {
    // Warm shell-env on a background thread so the window opens immediately;
    // the OnceLock blocks any `get_path` caller until capture finishes.
    // `apply_*` mutates `std::env` (not thread-safe) — safe here because
    // Tauri's runtime threads haven't read env yet at this point.
    std::thread::spawn(|| {
        emdash_dev::shell_env::apply_login_shell_env_to_process();
    });

    let specta_builder = tauri_bindings::build_specta();

    #[cfg(debug_assertions)]
    {
        if let Err(err) = export_bindings_default() {
            eprintln!("[emdash-dev] warning: failed to export TS bindings: {err}");
        }
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // EMD-20 memory hygiene: explicitly tear down owned
                // resources on window close. PTY today; SSH /
                // watchers / agent-hook listeners as they land.
                // Mitigates the multi-window leak tracked in
                // tauri-apps/tauri#5397.
                if let Some(registry) = window.app_handle().try_state::<Arc<Registry>>() {
                    registry.drain();
                }
            }
        })
        .setup(move |app| {
            specta_builder.mount_events(app);

            let menu = crate::app_menu::build(app.handle())?;
            app.set_menu(menu)?;

            let db_path = resolve_db_path(app.handle())?;
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let db = Db::open(&db_path)?;
            let master = Arc::new(OsKeyringMasterKey::new());
            let secrets = Arc::new(Secrets::new(master, db.clone()));
            let projects = Arc::new(ProjectsService::new(db.clone()));
            let ui_sync: Arc<UiSyncManager> = Arc::new(UiSyncManager::new());
            let fs_watcher: Arc<WatcherRegistry> = Arc::new(WatcherRegistry::new());
            let workspace_fs_lock: Arc<WorkspaceFsMutationLock> =
                Arc::new(WorkspaceFsMutationLock::new());
            let tasks = Arc::new(TasksService::new(db.clone(), workspace_fs_lock.clone()));
            let conversations = Arc::new(ConversationsService::new(db.clone()));
            let terminals = Arc::new(TerminalsService::new(db.clone()));
            // Telemetry::new spawns a Tokio worker; the Tauri setup
            // callback isn't inside a runtime context, so enter Tauri's
            // own runtime to host the spawn (same pattern as the
            // HookServer::start call below).
            let telemetry = Arc::new(tauri::async_runtime::block_on(async {
                Telemetry::new(db.clone(), TelemetryConfig::from_build_env())
            }));
            let updater: Arc<UpdateManager> = Arc::new(UpdateManager::default());

            // Agent-hook server (EMD-9 / ADR-0021). Bind synchronously
            // via Tokio's runtime so port + token are available before
            // any agent-spawn site (EMD-27) needs them. The handle
            // lives in app state so the broadcaster + Drop-based
            // shutdown both stay live for the app lifetime.
            let classifier_registry = Arc::new(ClassifierRegistry::new());
            classifier_registry.register(ClaudeClassifier);
            let hook_broadcaster_ui_sync = ui_sync.clone();
            let broadcaster: emdash_dev::agent_hooks::server::EventBroadcaster =
                Arc::new(move |event| {
                    hook_broadcaster_ui_sync.broadcast(UiMutationEvent::AgentHookEvent {
                        task_id: event.task_id.clone(),
                        event,
                    });
                });
            let hook_handle = tauri::async_runtime::block_on(HookServer::start(
                classifier_registry.clone(),
                broadcaster,
            ))?;
            let hook_handle: Arc<HookServerHandle> = Arc::new(hook_handle);

            let skills_service: Arc<emdash_dev::skills::SkillsService> =
                Arc::new(emdash_dev::skills::SkillsService::new());
            let mcp_service: Arc<emdash_dev::mcp::McpService> =
                Arc::new(emdash_dev::mcp::McpService::new());

            let pty_registry: Arc<Registry> = Arc::new(Registry::new());
            // EMD-27 / ADR-0024: agent-spawn service. Constructed
            // after the hook server has bound so the port + token
            // injected into the agent env are guaranteed live.
            let agent_service = Arc::new(emdash_dev::agents::AgentService::new(
                db.clone(),
                pty_registry.clone(),
                workspace_fs_lock.clone(),
                hook_handle.port(),
                hook_handle.token().to_string(),
            ));

            app.manage(db);
            app.manage(secrets);
            app.manage(projects);
            app.manage(ui_sync);
            app.manage(fs_watcher);
            app.manage(workspace_fs_lock);
            app.manage(tasks);
            app.manage(conversations);
            app.manage(terminals);
            app.manage(telemetry);
            app.manage(updater);
            app.manage(classifier_registry);
            app.manage(hook_handle);
            app.manage(agent_service);
            app.manage(skills_service);
            app.manage(mcp_service);
            app.manage(pty_registry);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building emdash-dev");

    app.run(|_handle, event| {
        if let RunEvent::Exit = event {
            // EMD-15: install-on-exit hook. If `UpdateManager` is in
            // ReadyToInstall, the bundled `tauri-plugin-updater` will
            // apply the staged update before the process actually
            // terminates. Wiring the call into the plugin's install
            // path lands with EMD-22's packaging pipeline; the hook
            // itself is in place so that follow-up is one-line.
        }
    });
}

/// `EMDASH_DEV_DB_FILE` env var overrides everything (set in dev shells, tests,
/// portable mode). Otherwise the path is `<app_data_dir>/emdash-dev.db`. The
/// app_data_dir on macOS is `~/Library/Application Support/com.emdash.dev/`
/// per the bundle identifier in tauri.conf.json.
fn resolve_db_path(handle: &tauri::AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(explicit) = std::env::var_os("EMDASH_DEV_DB_FILE") {
        let p = PathBuf::from(explicit);
        if p.as_os_str().is_empty() {
            return Err("EMDASH_DEV_DB_FILE is set but empty".into());
        }
        return Ok(p);
    }
    let dir = handle.path().app_data_dir()?;
    Ok(dir.join("emdash-dev.db"))
}
