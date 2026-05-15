//! Companion CLI that links only domain modules. Existence-proof that
//! `DOMAIN_MODULES` (see `tests/domain_boundaries.rs`) stay webview-free —
//! a domain module that imports a webview-runtime type fails to link here.

use std::env;
use std::process::ExitCode;

use emdash_dev::{
    agent_hooks, agents, bindings_parser, db, editor_buffers, fs_watcher, git, greeting, projects,
    providers,
    secrets::{aead, master_key},
    shell_env, tasks,
    telemetry::{
        config as telemetry_config, event as telemetry_event, settings as telemetry_settings,
    },
    ui_sync, updater, view_state,
};

const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    link_domain_modules();

    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("{NAME} {VERSION}");
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("error: unknown argument `{other}`");
            print_help();
            ExitCode::FAILURE
        }
    }
}

/// Pull every `DOMAIN_MODULES` entry into the link graph. The
/// `domain_boundaries` test enforces classification; this function enforces
/// actual linkage. Must include one symbol per domain module.
fn link_domain_modules() {
    let _ = greeting::greet("");
    let _ = shell_env::merge_path("", "");
    let _ = bindings_parser::extract_invoke_channels("");

    // db: reference the type and the migrations function so the symbols link.
    let _: Option<std::sync::Arc<db::Db>> = None;
    let _ = db::migrations::migrations();

    // secrets: aead + master_key are pure domain; reference one fn from each
    // so a webview-runtime leak would fail this binary's link step.
    let _ = aead::aad_for("");
    let _: Option<Box<dyn master_key::MasterKeyProvider>> = None;

    // ui_sync: state-sync domain primitive (no Tauri runtime — Channel<T>
    // wrapping happens in commands::ui_sync, not here).
    let _mgr = ui_sync::UiSyncManager::new();

    // projects: CRUD service reference. `ProjectsService::new` requires a Db,
    // which we already pull above; referencing the type keeps the symbol live.
    let _: Option<projects::ProjectsService> = None;

    // fs_watcher: domain-only registry (Tauri-runtime-free; Channel wrapping
    // lives in commands::fs_watcher).
    let _registry = fs_watcher::WatcherRegistry::new();
    let _: Option<fs_watcher::WatchEvent> = None;
    // tasks + git: type references so the symbols link.
    let _ = tasks::WorkspaceFsMutationLock::new();
    let _: Option<tasks::TasksService> = None;
    let _: Result<Option<String>, git::GitError> = Ok(None);
    // telemetry: domain-only reference. The runtime requires tokio + Db,
    // which we don't pull in this binary — referencing the build-time
    // config and the SETTINGS_KEY constant keeps the module linked.
    let _ = telemetry_config::TelemetryConfig::from_build_env();
    let _: Option<telemetry_event::TelemetryEvent> = None;
    let _ = telemetry_settings::SETTINGS_KEY;
    // updater: state machine + backoff are Tauri-runtime-free. Plugin glue
    // (the actual tauri-plugin-updater hookup) lives in commands::updater
    // and app.rs.
    let _updater_mgr = updater::UpdateManager::default();
    let _: Option<updater::UpdateEvent> = None;
    // editor_buffers + view_state: tiny KV-shaped domain modules ported from
    // Electron (ADR-0012 / ADR-0018). Referencing one function each keeps
    // the symbol live for the linker.
    let _: Option<editor_buffers::EditorBuffer> = None;
    let _: Option<serde_json::Value> = None;
    let _ = view_state::ViewStateError::MalformedJson(String::new());
    // agent_hooks: classifier + registry are Tauri-runtime-free. The
    // HookServer itself lives behind tokio/axum and is excluded from this
    // bin's link graph (no server start here — domain modules only).
    let _ah_registry = agent_hooks::ClassifierRegistry::new();
    let _: Option<agent_hooks::AgentEvent> = None;

    // agents: provider enum + spec helper are domain-only; the
    // AgentService runtime requires tokio + PTY, not exercised here.
    let _: agents::ProviderSpec = agents::provider_spec(agents::AgentProvider::Codex);

    // providers::github: scope enum + identity record are domain-only; the
    // async oauth + client paths come along but never run from this bin.
    let _: Option<providers::github::IdentityRecord> = None;
    let _: &[providers::github::OauthScope] = providers::github::auth::DEFAULT_SCOPES;

    // providers::linear: API-key paste only in v1; identity record is
    // domain-only so the bin links cleanly without pulling reqwest.
    let _: Option<providers::linear::LinearIdentityRecord> = None;
    let _: Option<providers::linear::LinearError> = None;
}

fn print_help() {
    println!(
        "{NAME} {VERSION}
emdash-dev companion CLI

USAGE:
    {NAME} [OPTIONS]

OPTIONS:
    -V, --version    Print version
    -h, --help       Print this help"
    );
}
