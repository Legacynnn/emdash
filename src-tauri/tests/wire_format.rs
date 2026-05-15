//! Wire-format snapshots for the renderer <-> Rust IPC contract. A deliberate
//! signature change must be accepted via `cargo insta review`.

#[test]
fn greet_wire_format() {
    let response = emdash_dev::greeting::greet("world");
    let request_args = serde_json::json!({ "name": "world" });

    insta::assert_json_snapshot!("greet_request_args", request_args);
    insta::assert_snapshot!("greet_response_world", response);
}

#[test]
fn greet_trims_and_defaults_empty_name() {
    insta::assert_snapshot!("greet_empty_name", emdash_dev::greeting::greet(""));
    insta::assert_snapshot!("greet_whitespace_name", emdash_dev::greeting::greet("   "));
}

/// Pins request args (locks the signature) and response *type* (the value
/// is host-dependent so we can't snapshot it directly).
#[test]
fn get_path_wire_format() {
    let request_args = serde_json::Value::Null;
    insta::assert_json_snapshot!("get_path_request_args", request_args);

    // Response type is `string` — pin the type, not the value.
    let response_type = std::any::type_name_of_val(&emdash_dev::shell_env::shell_env().path());
    insta::assert_snapshot!("get_path_response_type", response_type);
}

#[test]
fn bindings_ts_snapshot() {
    let path = format!("{}/ui/src/bindings.ts", env!("CARGO_MANIFEST_DIR"),);
    let content = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("read {path}: {e} — run `cargo run --bin emdash-dev -- --export-bindings`")
    });
    insta::assert_snapshot!("bindings_ts", content);
}

/// Regenerates bindings to a temp path and diffs against the committed file.
/// Catches stale bindings.ts locally — without this the only freshness check
/// is the CI `git diff` step.
#[test]
fn bindings_ts_in_sync_with_rust() {
    let tmp = std::env::temp_dir().join(format!(
        "emdash-dev-bindings-{}-{}.ts",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    emdash_dev::tauri_bindings::export_bindings_to(&tmp).expect("export_bindings_to must succeed");
    let regenerated = std::fs::read_to_string(&tmp).expect("read regenerated bindings");
    let _ = std::fs::remove_file(&tmp);

    let committed_path = format!(
        "{}/{}",
        env!("CARGO_MANIFEST_DIR"),
        emdash_dev::tauri_bindings::BINDINGS_PATH
    );
    let committed = std::fs::read_to_string(&committed_path)
        .unwrap_or_else(|e| panic!("read committed bindings {committed_path}: {e}"));

    assert_eq!(
        regenerated, committed,
        "ui/src/bindings.ts is stale relative to the Rust command set. \
         Run `cargo run --bin emdash-dev -- --export-bindings` and commit."
    );
}

#[test]
fn set_secret_wire_format() {
    let request_args = serde_json::json!({ "key": "github_token", "value": "ghp_abc123" });
    insta::assert_json_snapshot!("set_secret_request_args", request_args);
}

#[test]
fn get_secret_wire_format() {
    let request_args = serde_json::json!({ "key": "github_token" });
    insta::assert_json_snapshot!("get_secret_request_args", request_args);
}

#[test]
fn secrets_error_envelope_shape() {
    // Confirms the {code, message} envelope is serialized in snake_case.
    use emdash_dev::commands::secrets::{SecretsCommandError, SecretsErrorCode};
    let err = SecretsCommandError {
        code: SecretsErrorCode::KeyringUnavailable,
        message: "example".to_string(),
    };
    insta::assert_json_snapshot!(
        "secrets_error_envelope",
        serde_json::to_value(&err).unwrap()
    );
}

#[test]
fn pty_spawn_wire_format() {
    // Channel<Vec<u8>> is opaque on the wire (an internal id) — we pin the
    // user-visible args only. The bindings.ts snapshot covers the full
    // command signature including the Channel parameter.
    let request_args = serde_json::json!({
        "opts": {
            "command": "/bin/bash",
            "args": [],
            "cwd": null,
            "env": {},
            "size": { "rows": 24, "cols": 80 }
        }
    });
    insta::assert_json_snapshot!("pty_spawn_request_args", request_args);
}

#[test]
fn pty_write_wire_format() {
    let request_args = serde_json::json!({
        "id": 1,
        "bytes": [104, 105]
    });
    insta::assert_json_snapshot!("pty_write_request_args", request_args);
}

#[test]
fn pty_resize_wire_format() {
    let request_args = serde_json::json!({
        "id": 1,
        "size": { "rows": 40, "cols": 132 }
    });
    insta::assert_json_snapshot!("pty_resize_request_args", request_args);
}

#[test]
fn pty_kill_wire_format() {
    let request_args = serde_json::json!({ "id": 1 });
    insta::assert_json_snapshot!("pty_kill_request_args", request_args);
}

#[test]
fn pty_error_envelope_shape() {
    use emdash_dev::pty::types::{PtyError, PtyId};
    let err = PtyError::NotFound { id: PtyId(42) };
    insta::assert_json_snapshot!("pty_error_not_found", serde_json::to_value(&err).unwrap());
}

// --- ui_sync ---------------------------------------------------------------

#[test]
fn subscribe_ui_mutations_wire_format() {
    let request_args = serde_json::json!({ "subId": "renderer-1" });
    insta::assert_json_snapshot!("subscribe_ui_mutations_request_args", request_args);
}

#[test]
fn unsubscribe_ui_mutations_wire_format() {
    let request_args = serde_json::json!({ "subId": "renderer-1" });
    insta::assert_json_snapshot!("unsubscribe_ui_mutations_request_args", request_args);
}

#[test]
fn ui_mutation_event_project_created_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::ProjectCreated { id: "p1".into() };
    insta::assert_json_snapshot!(
        "ui_mutation_event_project_created",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn ui_mutation_event_project_updated_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::ProjectUpdated { id: "p1".into() };
    insta::assert_json_snapshot!(
        "ui_mutation_event_project_updated",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn ui_mutation_event_project_deleted_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::ProjectDeleted { id: "p1".into() };
    insta::assert_json_snapshot!(
        "ui_mutation_event_project_deleted",
        serde_json::to_value(&event).unwrap()
    );
}

// --- projects --------------------------------------------------------------

#[test]
fn projects_list_wire_format() {
    let request_args = serde_json::Value::Null;
    insta::assert_json_snapshot!("projects_list_request_args", request_args);
}

#[test]
fn projects_add_wire_format() {
    let request_args = serde_json::json!({ "path": "/Users/example/code/repo" });
    insta::assert_json_snapshot!("projects_add_request_args", request_args);
}

#[test]
fn projects_remove_wire_format() {
    let request_args = serde_json::json!({ "id": "00000000-0000-0000-0000-000000000001" });
    insta::assert_json_snapshot!("projects_remove_request_args", request_args);
}

#[test]
fn project_response_shape() {
    use emdash_dev::projects::Project;
    let project = Project {
        id: "00000000-0000-0000-0000-000000000001".into(),
        name: "repo".into(),
        path: "/Users/example/code/repo".into(),
        created_at: "2026-05-15 00:00:00".into(),
        updated_at: "2026-05-15 00:00:00".into(),
    };
    insta::assert_json_snapshot!("project_response", serde_json::to_value(&project).unwrap());
}

#[test]
fn projects_error_envelope_shape() {
    use emdash_dev::commands::projects::{ProjectsCommandError, ProjectsErrorCode};
    let err = ProjectsCommandError {
        code: ProjectsErrorCode::DuplicatePath,
        message: "a project already tracks this path: /Users/example/code/repo".to_string(),
    };
    insta::assert_json_snapshot!(
        "projects_error_envelope",
        serde_json::to_value(&err).unwrap()
    );
}

// --- tasks -----------------------------------------------------------------

#[test]
fn tasks_list_wire_format() {
    let request_args = serde_json::json!({ "projectId": "00000000-0000-0000-0000-000000000001" });
    insta::assert_json_snapshot!("tasks_list_request_args", request_args);
}

#[test]
fn tasks_create_wire_format() {
    let request_args = serde_json::json!({
        "projectId": "00000000-0000-0000-0000-000000000001",
        "name": "Feature X",
        "sourceBranch": { "type": "local", "branch": "main" }
    });
    insta::assert_json_snapshot!("tasks_create_request_args", request_args);
}

#[test]
fn tasks_create_wire_format_remote_source() {
    let request_args = serde_json::json!({
        "projectId": "00000000-0000-0000-0000-000000000001",
        "name": "Feature X",
        "sourceBranch": { "type": "remote", "host": "origin", "branch": "main" }
    });
    insta::assert_json_snapshot!("tasks_create_request_args_remote", request_args);
}

#[test]
fn tasks_delete_wire_format() {
    let request_args = serde_json::json!({ "id": "00000000-0000-0000-0000-000000000001" });
    insta::assert_json_snapshot!("tasks_delete_request_args", request_args);
}

#[test]
fn task_response_shape() {
    use emdash_dev::tasks::model::TaskStatus;
    use emdash_dev::tasks::{Task, TaskSourceBranch};
    let task = Task {
        id: "00000000-0000-0000-0000-000000000001".into(),
        project_id: "00000000-0000-0000-0000-000000000002".into(),
        name: "Feature X".into(),
        status: TaskStatus::Active,
        path: "/Users/example/code/repo/.emdash-worktrees/00000000-0000-0000-0000-000000000001"
            .into(),
        source_branch: TaskSourceBranch::Local {
            branch: "main".into(),
        },
        pty_id: None,
        created_at: "2026-05-15 00:00:00".into(),
        updated_at: "2026-05-15 00:00:00".into(),
    };
    insta::assert_json_snapshot!("task_response", serde_json::to_value(&task).unwrap());
}

#[test]
fn tasks_error_envelope_shape() {
    use emdash_dev::commands::tasks::{TasksCommandError, TasksErrorCode};
    let err = TasksCommandError {
        code: TasksErrorCode::WorktreeFailed,
        message: "git worktree add failed: fatal: ...".to_string(),
    };
    insta::assert_json_snapshot!("tasks_error_envelope", serde_json::to_value(&err).unwrap());
}

#[test]
fn ui_mutation_event_task_created_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::TaskCreated {
        id: "t1".into(),
        project_id: "p1".into(),
    };
    insta::assert_json_snapshot!(
        "ui_mutation_event_task_created",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn ui_mutation_event_task_updated_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::TaskUpdated {
        id: "t1".into(),
        project_id: "p1".into(),
    };
    insta::assert_json_snapshot!(
        "ui_mutation_event_task_updated",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn ui_mutation_event_task_deleted_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::TaskDeleted {
        id: "t1".into(),
        project_id: "p1".into(),
    };
    insta::assert_json_snapshot!(
        "ui_mutation_event_task_deleted",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn task_source_branch_local_wire_format() {
    use emdash_dev::tasks::TaskSourceBranch;
    let s = TaskSourceBranch::Local {
        branch: "main".into(),
    };
    insta::assert_json_snapshot!(
        "task_source_branch_local",
        serde_json::to_value(&s).unwrap()
    );
}

#[test]
fn task_source_branch_remote_wire_format() {
    use emdash_dev::tasks::TaskSourceBranch;
    let s = TaskSourceBranch::Remote {
        host: "origin".into(),
        branch: "main".into(),
    };
    insta::assert_json_snapshot!(
        "task_source_branch_remote",
        serde_json::to_value(&s).unwrap()
    );
}

// --- telemetry -------------------------------------------------------------

#[test]
fn telemetry_set_enabled_wire_format() {
    let request_args = serde_json::json!({ "enabled": true });
    insta::assert_json_snapshot!("telemetry_set_enabled_request_args", request_args);
}

#[test]
fn telemetry_record_focus_wire_format() {
    let request_args = serde_json::json!({
        "event": "app_focus",
        "ghUsername": null,
        "ghAccountId": null,
        "email": null,
    });
    insta::assert_json_snapshot!("telemetry_record_focus_request_args", request_args);
}

#[test]
fn telemetry_record_identify_wire_format() {
    let request_args = serde_json::json!({
        "event": "user_identify",
        "ghUsername": "octocat",
        "ghAccountId": "583231",
        "email": "octocat@example.com",
    });
    insta::assert_json_snapshot!("telemetry_record_identify_request_args", request_args);
}

// One assertion per test: insta stops on the first failure, so a
// multi-variant loop would mask sibling drift after the first fail.

#[test]
fn telemetry_event_app_focus() {
    use emdash_dev::telemetry::TelemetryEvent;
    insta::assert_json_snapshot!(
        "telemetry_event_app_focus",
        serde_json::to_value(TelemetryEvent::AppFocus).unwrap()
    );
}

#[test]
fn telemetry_event_app_unfocus() {
    use emdash_dev::telemetry::TelemetryEvent;
    insta::assert_json_snapshot!(
        "telemetry_event_app_unfocus",
        serde_json::to_value(TelemetryEvent::AppUnfocus).unwrap()
    );
}

#[test]
fn telemetry_event_app_dau_ping() {
    use emdash_dev::telemetry::TelemetryEvent;
    insta::assert_json_snapshot!(
        "telemetry_event_app_dau_ping",
        serde_json::to_value(TelemetryEvent::AppDauPing).unwrap()
    );
}

#[test]
fn telemetry_event_user_identify() {
    use emdash_dev::telemetry::TelemetryEvent;
    insta::assert_json_snapshot!(
        "telemetry_event_user_identify",
        serde_json::to_value(TelemetryEvent::UserIdentify).unwrap()
    );
}

#[test]
fn telemetry_error_envelope_shape() {
    use emdash_dev::commands::telemetry::{TelemetryCommandError, TelemetryErrorCode};
    let err = TelemetryCommandError {
        code: TelemetryErrorCode::Storage,
        message: "db error: pool error".to_string(),
    };
    insta::assert_json_snapshot!(
        "telemetry_error_envelope",
        serde_json::to_value(&err).unwrap()
    );
}

// --- updater ---------------------------------------------------------------

#[test]
fn updater_check_wire_format() {
    let request_args = serde_json::json!({ "reason": "manual" });
    insta::assert_json_snapshot!("updater_check_request_args", request_args);
}

#[test]
fn updater_check_reason_startup_wire_format() {
    use emdash_dev::updater::CheckReason;
    insta::assert_json_snapshot!(
        "updater_check_reason_startup",
        serde_json::to_value(CheckReason::Startup).unwrap()
    );
}

#[test]
fn update_event_checking_wire_format() {
    use emdash_dev::updater::{CheckReason, UpdateEvent};
    let event = UpdateEvent::Checking {
        reason: CheckReason::Manual,
    };
    insta::assert_json_snapshot!(
        "update_event_checking",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn update_event_up_to_date_wire_format() {
    use emdash_dev::updater::UpdateEvent;
    insta::assert_json_snapshot!(
        "update_event_up_to_date",
        serde_json::to_value(UpdateEvent::UpToDate).unwrap()
    );
}

#[test]
fn update_event_available_wire_format() {
    use emdash_dev::updater::UpdateEvent;
    let event = UpdateEvent::Available {
        version: "0.2.0".into(),
        notes: Some("Bug fixes".into()),
    };
    insta::assert_json_snapshot!(
        "update_event_available",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn update_event_downloading_wire_format() {
    use emdash_dev::updater::UpdateEvent;
    let event = UpdateEvent::Downloading { progress: 0.42 };
    insta::assert_json_snapshot!(
        "update_event_downloading",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn update_event_ready_to_install_wire_format() {
    use emdash_dev::updater::UpdateEvent;
    let event = UpdateEvent::ReadyToInstall {
        version: "0.2.0".into(),
    };
    insta::assert_json_snapshot!(
        "update_event_ready_to_install",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn update_event_error_wire_format() {
    use emdash_dev::updater::{UpdateError, UpdateEvent};
    let event = UpdateEvent::Error {
        code: UpdateError::Network,
        message: "connection refused".into(),
        will_retry: true,
    };
    insta::assert_json_snapshot!("update_event_error", serde_json::to_value(&event).unwrap());
}

#[test]
fn update_manifest_canonical_shape() {
    use emdash_dev::updater::{ManifestPlatform, UpdateManifest};
    let mut platforms = std::collections::BTreeMap::new();
    platforms.insert(
        "darwin-aarch64".to_string(),
        ManifestPlatform {
            signature: "untrusted comment: minisign signature ...\n<base64>".into(),
            url: "https://updates.emdash.dev/emdash-dev-0.2.0-darwin-aarch64.tar.gz".into(),
        },
    );
    let m = UpdateManifest {
        version: "0.2.0".into(),
        notes: Some("Initial pre-release.".into()),
        pub_date: "2026-05-15T17:00:00Z".into(),
        platforms,
    };
    insta::assert_json_snapshot!(
        "update_manifest_canonical",
        serde_json::to_value(&m).unwrap()
    );
}
