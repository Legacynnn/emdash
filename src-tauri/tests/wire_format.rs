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
    use emdash_dev::providers::github::IdentityRecord;
    let id = IdentityRecord {
        login: "octocat".into(),
        id: "1".into(),
        name: Some("Octo Cat".into()),
        email: Some("octo@example.com".into()),
        avatar_url: Some("https://avatars.githubusercontent.com/u/1?v=4".into()),
    };
    insta::assert_json_snapshot!("github_identity", serde_json::to_value(&id).unwrap());
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
