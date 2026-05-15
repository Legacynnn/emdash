//! Companion CLI that links only domain modules. Existence-proof that
//! `DOMAIN_MODULES` (see `tests/domain_boundaries.rs`) stay webview-free —
//! a domain module that imports a webview-runtime type fails to link here.

use std::env;
use std::process::ExitCode;

use emdash_dev::{
    bindings_parser, db, greeting, projects,
    secrets::{aead, master_key},
    shell_env,
    telemetry::{
        config as telemetry_config, event as telemetry_event, settings as telemetry_settings,
    },
    ui_sync,
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

    // telemetry: domain-only reference. The runtime requires tokio + Db,
    // which we don't pull in this binary — referencing the build-time
    // config and the SETTINGS_KEY constant keeps the module linked.
    let _ = telemetry_config::TelemetryConfig::from_build_env();
    let _: Option<telemetry_event::TelemetryEvent> = None;
    let _ = telemetry_settings::SETTINGS_KEY;
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
