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

// --- fs_watcher (EMD-11 / ADR-0020) ----------------------------------------

#[test]
fn fs_watcher_subscribe_wire_format() {
    let request_args = serde_json::json!({
        "id": "project-1",
        "path": "/Users/example/code/repo"
    });
    insta::assert_json_snapshot!("fs_watcher_subscribe_request_args", request_args);
}

#[test]
fn fs_watcher_unsubscribe_wire_format() {
    let request_args = serde_json::json!({ "id": "project-1" });
    insta::assert_json_snapshot!("fs_watcher_unsubscribe_request_args", request_args);
}

#[test]
fn watch_event_created_wire_format() {
    use emdash_dev::fs_watcher::{WatchEvent, WatchEventKind};
    let event = WatchEvent {
        paths: vec!["/Users/example/code/repo/src/main.rs".into()],
        event: WatchEventKind::Created,
    };
    insta::assert_json_snapshot!("watch_event_created", serde_json::to_value(&event).unwrap());
}

#[test]
fn watch_event_renamed_wire_format() {
    use emdash_dev::fs_watcher::{WatchEvent, WatchEventKind};
    let event = WatchEvent {
        paths: vec![],
        event: WatchEventKind::Renamed {
            from: "/Users/example/code/repo/a.rs".into(),
            to: "/Users/example/code/repo/b.rs".into(),
        },
    };
    insta::assert_json_snapshot!("watch_event_renamed", serde_json::to_value(&event).unwrap());
}

#[test]
fn watcher_fallback_inotify_enospc_wire_format() {
    use emdash_dev::fs_watcher::WatcherFallback;
    insta::assert_json_snapshot!(
        "watcher_fallback_inotify_enospc",
        serde_json::to_value(WatcherFallback::InotifyEnospcDepthOne).unwrap()
    );
}

// --- workspaces ------------------------------------------------------------

#[test]
fn workspaces_list_wire_format() {
    let request_args = serde_json::json!({ "projectId": "00000000-0000-0000-0000-000000000001" });
    insta::assert_json_snapshot!("workspaces_list_request_args", request_args);
}

#[test]
fn workspaces_create_wire_format() {
    let request_args = serde_json::json!({
        "projectId": "00000000-0000-0000-0000-000000000001",
        "name": "Feature X",
        "sourceBranch": { "type": "local", "branch": "main" },
        "workspaceBranch": null,
        "placement": "worktree",
        "existingBranch": false,
    });
    insta::assert_json_snapshot!("workspaces_create_request_args", request_args);
}

#[test]
fn workspaces_create_wire_format_remote_source() {
    let request_args = serde_json::json!({
        "projectId": "00000000-0000-0000-0000-000000000001",
        "name": "Feature X",
        "sourceBranch": { "type": "remote", "host": "origin", "branch": "main" },
        "workspaceBranch": null,
        "placement": "worktree",
        "existingBranch": false,
    });
    insta::assert_json_snapshot!("workspaces_create_request_args_remote", request_args);
}

#[test]
fn workspaces_create_wire_format_local_existing() {
    let request_args = serde_json::json!({
        "projectId": "00000000-0000-0000-0000-000000000001",
        "name": "ongoing work",
        "sourceBranch": { "type": "local", "branch": "feat/in-progress" },
        "workspaceBranch": "feat/in-progress",
        "placement": "local",
        "existingBranch": true,
    });
    insta::assert_json_snapshot!(
        "workspaces_create_request_args_local_existing",
        request_args
    );
}

#[test]
fn workspaces_delete_wire_format() {
    let request_args = serde_json::json!({ "id": "00000000-0000-0000-0000-000000000001" });
    insta::assert_json_snapshot!("workspaces_delete_request_args", request_args);
}

#[test]
fn workspace_response_shape() {
    use emdash_dev::workspaces::model::WorkspaceStatus;
    use emdash_dev::workspaces::{Workspace, WorkspacePlacement, WorkspaceSourceBranch};
    let workspace = Workspace {
        id: "00000000-0000-0000-0000-000000000001".into(),
        project_id: "00000000-0000-0000-0000-000000000002".into(),
        name: "Feature X".into(),
        status: WorkspaceStatus::Active,
        placement: WorkspacePlacement::Worktree,
        path: "/Users/example/code/repo/.emdash-worktrees/feature-x".into(),
        source_branch: WorkspaceSourceBranch::Local {
            branch: "main".into(),
        },
        pty_id: None,
        created_at: "2026-05-15 00:00:00".into(),
        updated_at: "2026-05-15 00:00:00".into(),
    };
    insta::assert_json_snapshot!(
        "workspace_response",
        serde_json::to_value(&workspace).unwrap()
    );
}

#[test]
fn workspace_response_local_shape() {
    use emdash_dev::workspaces::model::WorkspaceStatus;
    use emdash_dev::workspaces::{Workspace, WorkspacePlacement, WorkspaceSourceBranch};
    let workspace = Workspace {
        id: "00000000-0000-0000-0000-000000000003".into(),
        project_id: "00000000-0000-0000-0000-000000000002".into(),
        name: "in-place".into(),
        status: WorkspaceStatus::Active,
        placement: WorkspacePlacement::Local,
        path: "/Users/example/code/repo".into(),
        source_branch: WorkspaceSourceBranch::Local {
            branch: "main".into(),
        },
        pty_id: None,
        created_at: "2026-05-15 00:00:00".into(),
        updated_at: "2026-05-15 00:00:00".into(),
    };
    insta::assert_json_snapshot!(
        "workspace_response_local",
        serde_json::to_value(&workspace).unwrap()
    );
}

#[test]
fn workspaces_error_envelope_shape() {
    use emdash_dev::commands::workspaces::{WorkspacesCommandError, WorkspacesErrorCode};
    let err = WorkspacesCommandError {
        code: WorkspacesErrorCode::WorktreeFailed,
        message: "git worktree add failed: fatal: ...".to_string(),
        existing_workspace_id: None,
        changed_files: None,
    };
    insta::assert_json_snapshot!(
        "workspaces_error_envelope",
        serde_json::to_value(&err).unwrap()
    );
}

#[test]
fn workspaces_error_envelope_local_slot_taken() {
    use emdash_dev::commands::workspaces::{WorkspacesCommandError, WorkspacesErrorCode};
    let err = WorkspacesCommandError {
        code: WorkspacesErrorCode::LocalSlotTaken,
        message: "project already has an active local workspace: feature-x".to_string(),
        existing_workspace_id: Some("00000000-0000-0000-0000-000000000001".into()),
        changed_files: None,
    };
    insta::assert_json_snapshot!(
        "workspaces_error_envelope_local_slot_taken",
        serde_json::to_value(&err).unwrap()
    );
}

#[test]
fn workspaces_error_envelope_dirty_tree() {
    use emdash_dev::commands::workspaces::{WorkspacesCommandError, WorkspacesErrorCode};
    let err = WorkspacesCommandError {
        code: WorkspacesErrorCode::DirtyTree,
        message: "project working tree is dirty".to_string(),
        existing_workspace_id: None,
        changed_files: Some(vec!["src/lib.rs".into(), "Cargo.lock".into()]),
    };
    insta::assert_json_snapshot!(
        "workspaces_error_envelope_dirty_tree",
        serde_json::to_value(&err).unwrap()
    );
}

#[test]
fn ui_mutation_event_workspace_created_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::WorkspaceCreated {
        id: "w1".into(),
        project_id: "p1".into(),
    };
    insta::assert_json_snapshot!(
        "ui_mutation_event_workspace_created",
        serde_json::to_value(&event).unwrap()
    );
}

// --- github provider (EMD-13 / ADR-0022) -----------------------------------

#[test]
fn github_sign_in_device_flow_start_response_shape() {
    use emdash_dev::providers::github::DeviceFlowStart;
    let start = DeviceFlowStart {
        user_code: "WDJB-MJHT".into(),
        verification_uri: "https://github.com/login/device".into(),
        device_code: "abc-device".into(),
        polling_interval_seconds: 5,
        expires_in_seconds: 900,
    };
    insta::assert_json_snapshot!(
        "github_device_flow_start",
        serde_json::to_value(&start).unwrap()
    );
}

#[test]
fn github_identity_response_shape() {
    use emdash_dev::providers::github::{IdentityRecord, TokenSource};
    let id = IdentityRecord {
        login: "octocat".into(),
        id: "1".into(),
        name: Some("Octo Cat".into()),
        email: Some("octo@example.com".into()),
        avatar_url: Some("https://avatars.githubusercontent.com/u/1?v=4".into()),
        token_source: TokenSource::SecureStorage,
    };
    insta::assert_json_snapshot!("github_identity", serde_json::to_value(&id).unwrap());
}

#[test]
fn github_identity_cli_token_source_shape() {
    use emdash_dev::providers::github::{IdentityRecord, TokenSource};
    let id = IdentityRecord {
        login: "octocat".into(),
        id: "1".into(),
        name: None,
        email: None,
        avatar_url: None,
        token_source: TokenSource::Cli,
    };
    insta::assert_json_snapshot!(
        "github_identity_cli",
        serde_json::to_value(&id).unwrap()
    );
}

#[test]
fn github_pull_request_summary_shape() {
    use emdash_dev::providers::github::PullRequestSummary;
    let pr = PullRequestSummary {
        number: 42,
        title: "Tidy README".into(),
        state: "open".into(),
        draft: false,
        html_url: "https://github.com/example/repo/pull/42".into(),
        author: Some("octocat".into()),
        base_ref: "main".into(),
        head_ref: "feature/tidy-readme".into(),
        created_at: "2026-05-15T17:00:00+00:00".into(),
        updated_at: "2026-05-15T17:30:00+00:00".into(),
    };
    insta::assert_json_snapshot!(
        "github_pull_request_summary",
        serde_json::to_value(&pr).unwrap()
    );
}

#[test]
fn github_list_pulls_request_args() {
    let args = serde_json::json!({ "owner": "example", "repo": "repo" });
    insta::assert_json_snapshot!("github_list_pulls_request_args", args);
}

#[test]
fn github_get_pull_request_args() {
    let args = serde_json::json!({ "owner": "example", "repo": "repo", "number": 42 });
    insta::assert_json_snapshot!("github_get_pull_request_args", args);
}

#[test]
fn ui_mutation_github_identity_changed_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    insta::assert_json_snapshot!(
        "ui_mutation_github_identity_changed",
        serde_json::to_value(UiMutationEvent::GithubIdentityChanged).unwrap()
    );
}

#[test]
fn ui_mutation_event_workspace_updated_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::WorkspaceUpdated {
        id: "w1".into(),
        project_id: "p1".into(),
    };
    insta::assert_json_snapshot!(
        "ui_mutation_event_workspace_updated",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn ui_mutation_event_workspace_deleted_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::WorkspaceDeleted {
        id: "w1".into(),
        project_id: "p1".into(),
    };
    insta::assert_json_snapshot!(
        "ui_mutation_event_workspace_deleted",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn workspace_source_branch_local_wire_format() {
    use emdash_dev::workspaces::WorkspaceSourceBranch;
    let s = WorkspaceSourceBranch::Local {
        branch: "main".into(),
    };
    insta::assert_json_snapshot!(
        "workspace_source_branch_local",
        serde_json::to_value(&s).unwrap()
    );
}

#[test]
fn workspace_source_branch_remote_wire_format() {
    use emdash_dev::workspaces::WorkspaceSourceBranch;
    let s = WorkspaceSourceBranch::Remote {
        host: "origin".into(),
        branch: "main".into(),
    };
    insta::assert_json_snapshot!(
        "workspace_source_branch_remote",
        serde_json::to_value(&s).unwrap()
    );
}

#[test]
fn workspace_placement_worktree_wire_format() {
    use emdash_dev::workspaces::WorkspacePlacement;
    insta::assert_json_snapshot!(
        "workspace_placement_worktree",
        serde_json::to_value(WorkspacePlacement::Worktree).unwrap()
    );
}

#[test]
fn workspace_placement_local_wire_format() {
    use emdash_dev::workspaces::WorkspacePlacement;
    insta::assert_json_snapshot!(
        "workspace_placement_local",
        serde_json::to_value(WorkspacePlacement::Local).unwrap()
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

// --- editor_buffers + view_state (EMD-21 / ADRs 0012, 0018) ----------------

#[test]
fn editor_buffer_save_wire_format() {
    let request_args = serde_json::json!({
        "projectId": "p1",
        "workspaceId": "ws1",
        "filePath": "src/main.rs",
        "content": "fn main() {}\n"
    });
    insta::assert_json_snapshot!("editor_buffer_save_request_args", request_args);
}

#[test]
fn editor_buffer_list_wire_format() {
    let request_args = serde_json::json!({
        "projectId": "p1",
        "workspaceId": "ws1",
    });
    insta::assert_json_snapshot!("editor_buffer_list_request_args", request_args);
}

#[test]
fn editor_buffer_response_shape() {
    use emdash_dev::editor_buffers::EditorBuffer;
    let buffer = EditorBuffer {
        id: "p1|ws1|src/main.rs".to_string(),
        project_id: "p1".to_string(),
        workspace_id: "ws1".to_string(),
        file_path: "src/main.rs".to_string(),
        content: "fn main() {}\n".to_string(),
        updated_at_ms: "1763216400000".to_string(),
    };
    insta::assert_json_snapshot!(
        "editor_buffer_response",
        serde_json::to_value(&buffer).unwrap()
    );
}

#[test]
fn view_state_save_wire_format() {
    let request_args = serde_json::json!({
        "key": "layout.tabs",
        "valueJson": "{\"tabs\":[\"a\",\"b\"]}"
    });
    insta::assert_json_snapshot!("view_state_save_request_args", request_args);
}

#[test]
fn view_state_get_wire_format() {
    let request_args = serde_json::json!({ "key": "layout.tabs" });
    insta::assert_json_snapshot!("view_state_get_request_args", request_args);
}

// --- agent_hooks (EMD-9 / ADR-0021) ----------------------------------------

#[test]
fn agent_event_stop_wire_format() {
    use emdash_dev::agent_hooks::{AgentEvent, AgentEventKind};
    let event = AgentEvent {
        agent: "claude".into(),
        classifier: "claude".into(),
        kind: AgentEventKind::Stop,
        message: Some("session ended".into()),
        timestamp: "2026-05-15T17:00:00+00:00".into(),
        workspace_id: None,
        project_id: None,
    };
    insta::assert_json_snapshot!("agent_event_stop", serde_json::to_value(&event).unwrap());
}

#[test]
fn agent_event_notification_tool_use_wire_format() {
    use emdash_dev::agent_hooks::event::NotificationKind;
    use emdash_dev::agent_hooks::{AgentEvent, AgentEventKind};
    let event = AgentEvent {
        agent: "claude".into(),
        classifier: "claude".into(),
        kind: AgentEventKind::Notification {
            notification_kind: NotificationKind::ToolUse,
        },
        message: Some("tool: Bash".into()),
        timestamp: "2026-05-15T17:00:00+00:00".into(),
        workspace_id: None,
        project_id: None,
    };
    insta::assert_json_snapshot!(
        "agent_event_notification_tool_use",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn agent_event_unknown_wire_format() {
    use emdash_dev::agent_hooks::{AgentEvent, AgentEventKind};
    let event = AgentEvent {
        agent: "future-x".into(),
        classifier: "<unknown>".into(),
        kind: AgentEventKind::Unknown,
        message: None,
        timestamp: "2026-05-15T17:00:00+00:00".into(),
        workspace_id: None,
        project_id: None,
    };
    insta::assert_json_snapshot!("agent_event_unknown", serde_json::to_value(&event).unwrap());
}

#[test]
fn ui_mutation_agent_hook_event_wire_format() {
    use emdash_dev::agent_hooks::{AgentEvent, AgentEventKind};
    use emdash_dev::ui_sync::UiMutationEvent;
    let inner = AgentEvent {
        agent: "claude".into(),
        classifier: "claude".into(),
        kind: AgentEventKind::Stop,
        message: None,
        timestamp: "2026-05-15T17:00:00+00:00".into(),
        workspace_id: None,
        project_id: None,
    };
    let event = UiMutationEvent::AgentHookEvent {
        workspace_id: Some("ws-1".into()),
        event: inner,
    };
    insta::assert_json_snapshot!(
        "ui_mutation_agent_hook_event",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn ui_mutation_github_data_changed_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::GithubDataChanged {
        repo: "octocat/Hello-World".into(),
    };
    insta::assert_json_snapshot!(
        "ui_mutation_github_data_changed",
        serde_json::to_value(&event).unwrap()
    );
}

// --- agents (EMD-27 / ADR-0024) --------------------------------------------

#[test]
fn agents_start_wire_format() {
    let args = serde_json::json!({
        "workspaceId": "ws-1",
        "provider": "claude",
        "size": { "rows": 24, "cols": 80 },
    });
    insta::assert_json_snapshot!("agents_start_request_args", args);
}

#[test]
fn agents_stop_wire_format() {
    let args = serde_json::json!({ "workspaceId": "ws-1" });
    insta::assert_json_snapshot!("agents_stop_request_args", args);
}

#[test]
fn agents_provider_codex_wire_format() {
    use emdash_dev::agents::AgentProvider;
    insta::assert_json_snapshot!(
        "agents_provider_codex",
        serde_json::to_value(AgentProvider::Codex).unwrap()
    );
}

#[test]
fn ui_mutation_agent_started_wire_format() {
    use emdash_dev::agents::AgentProvider;
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::AgentStarted {
        workspace_id: "ws-1".into(),
        provider: AgentProvider::Claude,
    };
    insta::assert_json_snapshot!(
        "ui_mutation_agent_started",
        serde_json::to_value(&event).unwrap()
    );
}

#[test]
fn ui_mutation_agent_exited_wire_format() {
    use emdash_dev::ui_sync::UiMutationEvent;
    let event = UiMutationEvent::AgentExited {
        workspace_id: "ws-1".into(),
        exit_code: Some(0),
    };
    insta::assert_json_snapshot!(
        "ui_mutation_agent_exited",
        serde_json::to_value(&event).unwrap()
    );
}
