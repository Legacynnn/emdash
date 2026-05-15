//! Tauri runtime glue. Domain logic stays in `emdash_dev::*` lib modules.

use std::path::PathBuf;
use std::sync::Arc;

use emdash_dev::db::Db;
use emdash_dev::projects::ProjectsService;
use emdash_dev::pty::registry::Registry;
use emdash_dev::secrets::{master_key::OsKeyringMasterKey, Secrets};
use emdash_dev::tasks::{TasksService, WorkspaceFsMutationLock};
use emdash_dev::tauri_bindings;
use emdash_dev::telemetry::{Telemetry, TelemetryConfig};
use emdash_dev::ui_sync::UiSyncManager;
use emdash_dev::updater::UpdateManager;
use tauri::{Manager, RunEvent};

pub fn export_bindings_default() -> Result<(), Box<dyn std::error::Error>> {
    let path = format!(
        "{}/{}",
        env!("CARGO_MANIFEST_DIR"),
        tauri_bindings::BINDINGS_PATH
    );
    tauri_bindings::export_bindings_to(&path)
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
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            let workspace_fs_lock: Arc<WorkspaceFsMutationLock> =
                Arc::new(WorkspaceFsMutationLock::new());
            let tasks = Arc::new(TasksService::new(db.clone(), workspace_fs_lock.clone()));
            let telemetry = Arc::new(Telemetry::new(
                db.clone(),
                TelemetryConfig::from_build_env(),
            ));
            let updater: Arc<UpdateManager> = Arc::new(UpdateManager::default());

            app.manage(db);
            app.manage(secrets);
            app.manage(projects);
            app.manage(ui_sync);
            app.manage(workspace_fs_lock);
            app.manage(tasks);
            app.manage(telemetry);
            app.manage(updater);
            let pty_registry: Arc<Registry> = Arc::new(Registry::new());
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
