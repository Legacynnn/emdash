//! Collapsed bootstrap migration. Mirrors the Electron final-state schema
//! (`src/main/db/schema.ts`) except `app_secrets`, which uses the AEAD schema
//! defined by EMD-6.

use rusqlite::Transaction;
use rusqlite_migration::{Migrations, M};

/// Public so `Db::open` and the tests below share the same handle.
pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(BOOTSTRAP_SQL),
        M::up_with_hook(RENAME_DROPS_SQL, |tx: &Transaction| {
            copy_tasks_into_workspaces_if_present(tx)?;
            tx.execute_batch("DROP TABLE IF EXISTS tasks;")?;
            Ok(())
        }),
        M::up_with_hook("", |tx: &Transaction| {
            rebuild_child_tables(tx)?;
            Ok(())
        }),
        // Per-conversation agents: move `pty_id` from `workspaces` onto
        // `conversations` so each conversation tab owns its own `claude`
        // process. The old `workspaces.pty_id` values were always stale
        // (they pointed at PTYs from previous app runs), so dropping the
        // column doesn't lose anything actionable.
        M::up_with_hook("", |tx: &Transaction| {
            add_pty_id_to_conversations(tx)?;
            drop_pty_id_from_workspaces(tx)?;
            Ok(())
        }),
    ])
}

fn add_pty_id_to_conversations(
    tx: &Transaction,
) -> Result<(), rusqlite_migration::HookError> {
    if has_column(tx, "conversations", "pty_id")? {
        return Ok(());
    }
    tx.execute_batch(
        "ALTER TABLE conversations ADD COLUMN pty_id text;
         CREATE INDEX IF NOT EXISTS idx_conversations_pty_id
             ON conversations (pty_id) WHERE pty_id IS NOT NULL;",
    )?;
    Ok(())
}

fn drop_pty_id_from_workspaces(
    tx: &Transaction,
) -> Result<(), rusqlite_migration::HookError> {
    if !has_column(tx, "workspaces", "pty_id")? {
        return Ok(());
    }
    // SQLite 3.35+ supports `ALTER TABLE … DROP COLUMN` directly. The
    // index needs to come down first.
    tx.execute_batch(
        "DROP INDEX IF EXISTS idx_workspaces_pty_id;
         ALTER TABLE workspaces DROP COLUMN pty_id;",
    )?;
    Ok(())
}

fn copy_tasks_into_workspaces_if_present(
    tx: &Transaction,
) -> Result<(), rusqlite_migration::HookError> {
    let task_table_present: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'tasks'",
        [],
        |row| row.get(0),
    )?;
    if task_table_present == 0 {
        return Ok(());
    }
    // Best-effort copy: pre-rename `tasks` had `task_branch` where the
    // new schema has `workspace_branch`, and no `placement` column
    // (everything was a worktree). Map columns explicitly.
    tx.execute_batch(
        "INSERT INTO workspaces (
            id, project_id, name, status, placement, path, pty_id,
            source_branch, workspace_branch, linked_issue, archived_at,
            created_at, updated_at, last_interacted_at,
            status_changed_at, is_pinned
         )
         SELECT
            id, project_id, name, status, 'worktree' AS placement, path, pty_id,
            source_branch, task_branch AS workspace_branch, linked_issue, archived_at,
            created_at, updated_at, last_interacted_at,
            status_changed_at, is_pinned
         FROM tasks;",
    )?;
    Ok(())
}

const BOOTSTRAP_SQL: &str = r#"
-- ssh_connections ----------------------------------------------------------
CREATE TABLE ssh_connections (
    id text PRIMARY KEY NOT NULL,
    name text NOT NULL,
    host text NOT NULL,
    port integer NOT NULL DEFAULT 22,
    username text NOT NULL,
    auth_type text NOT NULL DEFAULT 'agent',
    private_key_path text,
    use_agent integer NOT NULL DEFAULT 0,
    metadata text,
    created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX idx_ssh_connections_name ON ssh_connections (name);
CREATE INDEX idx_ssh_connections_host ON ssh_connections (host);

-- projects -----------------------------------------------------------------
CREATE TABLE projects (
    id text PRIMARY KEY NOT NULL,
    name text NOT NULL,
    path text NOT NULL,
    infra_provider text NOT NULL DEFAULT 'local',
    base_ref text,
    ssh_connection_id text REFERENCES ssh_connections(id) ON DELETE SET NULL,
    created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX idx_projects_path ON projects (path);
CREATE INDEX idx_projects_ssh_connection_id ON projects (ssh_connection_id);

-- project_remotes ----------------------------------------------------------
CREATE TABLE project_remotes (
    project_id text NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    remote_name text NOT NULL,
    remote_url text NOT NULL,
    PRIMARY KEY (project_id, remote_name)
);

-- project_settings ---------------------------------------------------------
CREATE TABLE project_settings (
    project_id text PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    base_project_settings_json text NOT NULL DEFAULT '{}',
    shareable_project_settings_json text NOT NULL DEFAULT '{}',
    legacy_config_migrated_at text,
    created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- app_settings -------------------------------------------------------------
CREATE TABLE app_settings (
    key text PRIMARY KEY NOT NULL,
    value text NOT NULL,
    updated_at integer NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX idx_app_settings_key ON app_settings (key);

-- workspaces ---------------------------------------------------------------
-- A workspace is one unit of work inside a project. `placement` controls
-- whether the work lives in a git worktree under `.emdash-worktrees/<branch>/`
-- ('worktree') or in the project directory itself ('local'). `path` is the
-- on-disk root: worktree dir for 'worktree', project root for 'local'.
-- `source_branch` is a JSON discriminator: `{"type":"local","branch":"..."}`
-- or `{"type":"remote","host":"...","branch":"..."}`. The domain layer
-- decodes/encodes on read/write.
CREATE TABLE workspaces (
    id text PRIMARY KEY NOT NULL,
    project_id text NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name text NOT NULL,
    status text NOT NULL,
    placement text NOT NULL DEFAULT 'worktree',
    path text NOT NULL,
    pty_id text,
    source_branch text,
    workspace_branch text,
    linked_issue text,
    archived_at text,
    created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_interacted_at text,
    status_changed_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_pinned integer NOT NULL DEFAULT 0
);
CREATE INDEX idx_workspaces_project_id ON workspaces (project_id);
CREATE INDEX idx_workspaces_pty_id ON workspaces (pty_id) WHERE pty_id IS NOT NULL;
CREATE UNIQUE INDEX idx_workspaces_one_local_per_project
    ON workspaces (project_id)
    WHERE placement = 'local' AND archived_at IS NULL;

-- pull_request_users -------------------------------------------------------
CREATE TABLE pull_request_users (
    user_id text PRIMARY KEY NOT NULL,
    user_name text NOT NULL,
    display_name text,
    avatar_url text,
    url text,
    user_updated_at text,
    user_created_at text
);

-- pull_requests ------------------------------------------------------------
CREATE TABLE pull_requests (
    url text PRIMARY KEY NOT NULL,
    provider text NOT NULL DEFAULT 'github',
    repository_url text NOT NULL,
    base_ref_name text NOT NULL,
    base_ref_oid text NOT NULL,
    head_repository_url text NOT NULL,
    head_ref_name text NOT NULL,
    head_ref_oid text NOT NULL,
    identifier text,
    title text NOT NULL,
    description text,
    status text NOT NULL DEFAULT 'open',
    is_draft integer,
    author_user_id text REFERENCES pull_request_users(user_id) ON DELETE SET NULL,
    additions integer,
    deletions integer,
    changed_files integer,
    commit_count integer,
    mergeable_status text,
    merge_state_status text,
    review_decision text,
    pull_request_created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    pull_request_updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX idx_pull_requests_url ON pull_requests (url);
CREATE INDEX idx_pull_requests_repository_url ON pull_requests (repository_url);
CREATE INDEX idx_pull_requests_head_repository_url ON pull_requests (head_repository_url);

-- pull_request_labels ------------------------------------------------------
CREATE TABLE pull_request_labels (
    pull_request_id text NOT NULL REFERENCES pull_requests(url) ON DELETE CASCADE,
    name text NOT NULL,
    color text,
    PRIMARY KEY (pull_request_id, name)
);
CREATE INDEX idx_prl_name ON pull_request_labels (name);

-- pull_request_assignees ---------------------------------------------------
CREATE TABLE pull_request_assignees (
    pull_request_url text NOT NULL REFERENCES pull_requests(url) ON DELETE CASCADE,
    user_id text NOT NULL REFERENCES pull_request_users(user_id) ON DELETE CASCADE,
    PRIMARY KEY (pull_request_url, user_id)
);
CREATE INDEX idx_pra_pull_request_url ON pull_request_assignees (pull_request_url);
CREATE INDEX idx_pra_user_id ON pull_request_assignees (user_id);

-- pull_request_checks ------------------------------------------------------
CREATE TABLE pull_request_checks (
    id text PRIMARY KEY NOT NULL,
    pull_request_url text NOT NULL REFERENCES pull_requests(url) ON DELETE CASCADE,
    commit_sha text NOT NULL,
    name text NOT NULL,
    status text NOT NULL,
    conclusion text NOT NULL,
    details_url text,
    started_at text,
    completed_at text,
    workflow_name text,
    app_name text,
    app_logo_url text
);
CREATE INDEX idx_prc_pull_request_url ON pull_request_checks (pull_request_url);

-- conversations ------------------------------------------------------------
CREATE TABLE conversations (
    id text PRIMARY KEY NOT NULL,
    project_id text NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    workspace_id text NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title text NOT NULL,
    provider text,
    config text,
    created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_interacted_at text,
    is_initial_conversation integer
);
CREATE INDEX idx_conversations_workspace_id ON conversations (workspace_id);

-- terminals ----------------------------------------------------------------
CREATE TABLE terminals (
    id text PRIMARY KEY NOT NULL,
    project_id text NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    workspace_id text NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    ssh integer NOT NULL DEFAULT 0,
    name text NOT NULL,
    created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_terminals_workspace_id ON terminals (workspace_id);

-- messages -----------------------------------------------------------------
CREATE TABLE messages (
    id text PRIMARY KEY NOT NULL,
    conversation_id text NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    content text NOT NULL,
    sender text NOT NULL,
    timestamp text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    metadata text
);
CREATE INDEX idx_messages_conversation_id ON messages (conversation_id);
CREATE INDEX idx_messages_timestamp ON messages (timestamp);

-- editor_buffers -----------------------------------------------------------
CREATE TABLE editor_buffers (
    id text PRIMARY KEY NOT NULL,
    project_id text NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    workspace_id text NOT NULL,
    file_path text NOT NULL,
    content text NOT NULL,
    updated_at integer NOT NULL
);
CREATE INDEX idx_editor_buffers_workspace_file ON editor_buffers (workspace_id, file_path);

-- kv -----------------------------------------------------------------------
CREATE TABLE kv (
    key text PRIMARY KEY NOT NULL,
    value text NOT NULL,
    updated_at integer NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX idx_kv_key ON kv (key);

-- app_secrets (EMD-6 AEAD schema — replaces the Electron app_secrets) ------
CREATE TABLE app_secrets (
    key text PRIMARY KEY NOT NULL,
    nonce blob NOT NULL,
    ciphertext blob NOT NULL,
    aad blob NOT NULL,
    created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX idx_app_secrets_key ON app_secrets (key);
"#;

/// EMD-XX: the `tasks → workspaces` rename. Earlier dev builds shipped a
/// `workspaces` table with a legacy `(id, key, type, data, ...)` shape
/// (no relation to the current workspace domain) alongside the `tasks`
/// table that actually held workspace rows. After the rename, the Rust
/// service `INSERT`s into `workspaces (id, project_id, name, status,
/// placement, path, ...)` which the legacy schema can't satisfy
/// ("table workspaces has no column named project_id"). This migration
/// drops the legacy `workspaces`, recreates it with the new schema, and
/// (in the paired Rust hook) best-effort copies surviving rows out of
/// `tasks` before dropping that too.
///
/// Safe for both fresh DBs (no `tasks` table → the hook no-ops) and
/// pre-rename DBs with populated `tasks` (rows preserved via column
/// remap).
const RENAME_DROPS_SQL: &str = r#"
DROP INDEX IF EXISTS idx_workspaces_key;
DROP INDEX IF EXISTS idx_workspaces_project_id;
DROP INDEX IF EXISTS idx_workspaces_pty_id;
DROP INDEX IF EXISTS idx_workspaces_one_local_per_project;
DROP TABLE IF EXISTS workspaces;

CREATE TABLE workspaces (
    id text PRIMARY KEY NOT NULL,
    project_id text NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name text NOT NULL,
    status text NOT NULL,
    placement text NOT NULL DEFAULT 'worktree',
    path text NOT NULL,
    pty_id text,
    source_branch text,
    workspace_branch text,
    linked_issue text,
    archived_at text,
    created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_interacted_at text,
    status_changed_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_pinned integer NOT NULL DEFAULT 0
);
CREATE INDEX idx_workspaces_project_id ON workspaces (project_id);
CREATE INDEX idx_workspaces_pty_id ON workspaces (pty_id) WHERE pty_id IS NOT NULL;
CREATE UNIQUE INDEX idx_workspaces_one_local_per_project
    ON workspaces (project_id)
    WHERE placement = 'local' AND archived_at IS NULL;
"#;

/// Companion to `RENAME_DROPS_SQL`: the `tasks → workspaces` rename
/// also moved the FK from `tasks(id)` to `workspaces(id)` and renamed
/// the column on `conversations` and `terminals` from `task_id` to
/// `workspace_id`. Plus `conversations` gained the
/// `is_initial_conversation` column. SQLite can't rename columns *and*
/// re-target FKs in place, so we rebuild each table. The rebuild is
/// only needed if the table still has the legacy `task_id` column —
/// fresh DBs already get the post-rename shape from the bootstrap and
/// the hook below skips them.
fn rebuild_child_tables(
    tx: &Transaction,
) -> Result<(), rusqlite_migration::HookError> {
    rebuild_conversations(tx)?;
    rebuild_terminals(tx)?;
    Ok(())
}

fn has_column(
    tx: &Transaction,
    table: &str,
    column: &str,
) -> Result<bool, rusqlite_migration::HookError> {
    // `PRAGMA table_info(x)` returns one row per column; column[1] is
    // the name. No-op if the table doesn't exist.
    let mut stmt = tx.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in names {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rebuild_conversations(
    tx: &Transaction,
) -> Result<(), rusqlite_migration::HookError> {
    if !has_column(tx, "conversations", "task_id")? {
        // Already on the post-rename shape (fresh DBs).
        return Ok(());
    }
    // `is_initial_conversation` was added in the post-rename schema; legacy
    // DBs may or may not have it. Tolerate both.
    let has_initial = has_column(tx, "conversations", "is_initial_conversation")?;
    let initial_select = if has_initial {
        "is_initial_conversation"
    } else {
        "NULL"
    };

    tx.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP INDEX IF EXISTS idx_conversations_task_id;
         DROP INDEX IF EXISTS idx_conversations_workspace_id;
         ALTER TABLE conversations RENAME TO conversations_legacy;
         CREATE TABLE conversations (
             id text PRIMARY KEY NOT NULL,
             project_id text NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
             workspace_id text NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
             title text NOT NULL,
             provider text,
             config text,
             created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
             updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
             last_interacted_at text,
             is_initial_conversation integer
         );",
    )?;
    tx.execute_batch(&format!(
        "INSERT INTO conversations (
             id, project_id, workspace_id, title, provider, config,
             created_at, updated_at, last_interacted_at, is_initial_conversation
         )
         SELECT
             id, project_id, task_id AS workspace_id, title, provider, config,
             created_at, updated_at, last_interacted_at, {initial_select}
         FROM conversations_legacy;
         DROP TABLE conversations_legacy;
         CREATE INDEX idx_conversations_workspace_id ON conversations (workspace_id);
         PRAGMA foreign_keys = ON;"
    ))?;
    Ok(())
}

fn rebuild_terminals(
    tx: &Transaction,
) -> Result<(), rusqlite_migration::HookError> {
    if !has_column(tx, "terminals", "task_id")? {
        return Ok(());
    }
    tx.execute_batch(
        "PRAGMA foreign_keys = OFF;
         DROP INDEX IF EXISTS idx_terminals_task_id;
         DROP INDEX IF EXISTS idx_terminals_workspace_id;
         ALTER TABLE terminals RENAME TO terminals_legacy;
         CREATE TABLE terminals (
             id text PRIMARY KEY NOT NULL,
             project_id text NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
             workspace_id text NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
             ssh integer NOT NULL DEFAULT 0,
             name text NOT NULL,
             created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
             updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP
         );
         INSERT INTO terminals (
             id, project_id, workspace_id, ssh, name, created_at, updated_at
         )
         SELECT
             id, project_id, task_id AS workspace_id, ssh, name, created_at, updated_at
         FROM terminals_legacy;
         DROP TABLE terminals_legacy;
         CREATE INDEX idx_terminals_workspace_id ON terminals (workspace_id);
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("open in-memory");
        migrations()
            .to_latest(&mut conn)
            .expect("apply migrations to latest");
        conn
    }

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
                 ORDER BY name",
            )
            .unwrap();
        let names: rusqlite::Result<Vec<String>> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect();
        names.unwrap()
    }

    #[test]
    fn fresh_apply_creates_expected_tables() {
        let conn = fresh_db();
        let names = table_names(&conn);

        // Every table the Electron final-state schema defines, plus the new
        // AEAD app_secrets. No more, no less.
        let expected: &[&str] = &[
            "app_secrets",
            "app_settings",
            "conversations",
            "editor_buffers",
            "kv",
            "messages",
            "project_remotes",
            "project_settings",
            "projects",
            "pull_request_assignees",
            "pull_request_checks",
            "pull_request_labels",
            "pull_request_users",
            "pull_requests",
            "ssh_connections",
            "terminals",
            "workspaces",
        ];
        let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
        assert_eq!(names, expected, "table set drift — update collapsed SQL");
    }

    #[test]
    fn user_version_matches_migration_count() {
        let conn = fresh_db();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            v, 4,
            "PRAGMA user_version must reflect migration count (bootstrap + rename-tasks + rebuild-children + per-conversation-pty)"
        );
    }

    #[test]
    fn rebuild_child_tables_migrates_task_id_to_workspace_id() {
        use rusqlite::params;
        // Simulate a pre-rename DB: bootstrap only, then hand-create the
        // legacy `tasks`, `conversations`, and `terminals` tables and seed
        // rows that reference task_id. Then re-apply the full migration set
        // and verify the columns + data come out the other side intact.
        let mut conn = Connection::open_in_memory().unwrap();
        Migrations::new(vec![M::up(BOOTSTRAP_SQL)])
            .to_latest(&mut conn)
            .unwrap();

        // Need a project to satisfy FK in tasks/conversations/terminals.
        conn.execute(
            "INSERT INTO projects (id, name, path) VALUES (?, ?, ?)",
            params!["p1", "demo", "/tmp/demo"],
        )
        .unwrap();

        // Bootstrap created `conversations` and `terminals` with workspace_id
        // FK → workspaces(id). To simulate the legacy shape we have to drop
        // them and re-create with task_id.
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE conversations;
             DROP TABLE terminals;
             CREATE TABLE tasks (
                 id text PRIMARY KEY NOT NULL,
                 project_id text NOT NULL,
                 name text NOT NULL,
                 status text NOT NULL,
                 path text NOT NULL,
                 pty_id text,
                 source_branch text,
                 task_branch text,
                 linked_issue text,
                 archived_at text,
                 created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 last_interacted_at text,
                 status_changed_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 is_pinned integer NOT NULL DEFAULT 0
             );
             CREATE TABLE conversations (
                 id text PRIMARY KEY NOT NULL,
                 project_id text NOT NULL,
                 task_id text NOT NULL,
                 title text NOT NULL,
                 provider text,
                 config text,
                 created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 last_interacted_at text
             );
             CREATE TABLE terminals (
                 id text PRIMARY KEY NOT NULL,
                 project_id text NOT NULL,
                 task_id text NOT NULL,
                 ssh integer NOT NULL DEFAULT 0,
                 name text NOT NULL,
                 created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, project_id, name, status, path) \
             VALUES (?, ?, ?, ?, ?)",
            params!["t1", "p1", "task", "active", "/tmp/wt"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversations (id, project_id, task_id, title, provider) \
             VALUES (?, ?, ?, ?, ?)",
            params!["c1", "p1", "t1", "chat", "codex"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO terminals (id, project_id, task_id, name) \
             VALUES (?, ?, ?, ?)",
            params!["term1", "p1", "t1", "shell"],
        )
        .unwrap();
        // Re-enable FKs before applying migrations.
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

        migrations().to_latest(&mut conn).unwrap();

        // workspace_id replaced task_id, rows preserved, indexes recreated.
        let (cid, ws_id): (String, String) = conn
            .query_row(
                "SELECT id, workspace_id FROM conversations WHERE id = ?",
                params!["c1"],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(cid, "c1");
        assert_eq!(ws_id, "t1");
        let (tid, tws): (String, String) = conn
            .query_row(
                "SELECT id, workspace_id FROM terminals WHERE id = ?",
                params!["term1"],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(tid, "term1");
        assert_eq!(tws, "t1");
    }

    #[test]
    fn rename_migration_drops_tasks_table() {
        let conn = fresh_db();
        // After both migrations apply, the legacy `tasks` table is gone.
        let names = table_names(&conn);
        assert!(!names.contains(&"tasks".to_string()));
        assert!(names.contains(&"workspaces".to_string()));
    }

    #[test]
    fn rename_migration_carries_workspaces_columns() {
        let conn = fresh_db();
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(workspaces)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for required in [
            "id",
            "project_id",
            "name",
            "status",
            "placement",
            "path",
            "source_branch",
            "workspace_branch",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "workspaces missing required column `{required}` (got {cols:?})"
            );
        }
    }

    #[test]
    fn rename_migration_preserves_task_rows() {
        use rusqlite::params;
        // Simulate a pre-rename DB: apply bootstrap only, then hand-create
        // the legacy `tasks` table and seed a row, then re-apply migrations.
        let mut conn = Connection::open_in_memory().unwrap();
        let bootstrap_only = Migrations::new(vec![M::up(BOOTSTRAP_SQL)]);
        bootstrap_only.to_latest(&mut conn).unwrap();

        // Seed a project so the FK on workspaces.project_id has a target.
        conn.execute(
            "INSERT INTO projects (id, name, path) VALUES (?, ?, ?)",
            params!["p1", "demo", "/tmp/demo"],
        )
        .unwrap();

        // Hand-create the legacy `tasks` table (mirrors the pre-rename shape).
        conn.execute_batch(
            "CREATE TABLE tasks (
                id text PRIMARY KEY NOT NULL,
                project_id text NOT NULL,
                name text NOT NULL,
                status text NOT NULL,
                path text NOT NULL,
                pty_id text,
                source_branch text,
                task_branch text,
                linked_issue text,
                archived_at text,
                created_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                last_interacted_at text,
                status_changed_at text NOT NULL DEFAULT CURRENT_TIMESTAMP,
                is_pinned integer NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, project_id, name, status, path, task_branch) \
             VALUES (?, ?, ?, ?, ?, ?)",
            params!["t1", "p1", "feature", "active", "/tmp/wt", "task/foo"],
        )
        .unwrap();

        // Now apply the FULL migration set — this includes the rename hook.
        migrations().to_latest(&mut conn).unwrap();

        let names = table_names(&conn);
        assert!(!names.contains(&"tasks".to_string()), "tasks should be dropped");
        let (id, name, branch): (String, String, String) = conn
            .query_row(
                "SELECT id, name, workspace_branch FROM workspaces WHERE id = ?",
                params!["t1"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(id, "t1");
        assert_eq!(name, "feature");
        assert_eq!(branch, "task/foo");
    }

    #[test]
    fn app_secrets_has_aead_columns() {
        let conn = fresh_db();
        let cols: Vec<(String, String, i64)> = conn
            .prepare("PRAGMA table_info(app_secrets)")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?, // name
                    row.get::<_, String>(2)?, // type
                    row.get::<_, i64>(3)?,    // notnull
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();

        let by_name: std::collections::HashMap<_, _> = cols
            .iter()
            .map(|(n, t, nn)| (n.as_str(), (t.as_str(), *nn)))
            .collect();

        assert_eq!(by_name.get("key"), Some(&("TEXT", 1)));
        assert_eq!(by_name.get("nonce"), Some(&("BLOB", 1)));
        assert_eq!(by_name.get("ciphertext"), Some(&("BLOB", 1)));
        assert_eq!(by_name.get("aad"), Some(&("BLOB", 1)));
        assert!(by_name.contains_key("created_at"));
        assert!(by_name.contains_key("updated_at"));
    }

    #[test]
    fn app_secrets_key_is_unique() {
        let conn = fresh_db();
        conn.execute(
            "INSERT INTO app_secrets (key, nonce, ciphertext, aad, created_at, updated_at) \
             VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            rusqlite::params!["k", &[0u8; 12][..], &[0u8; 16][..], b"aad".as_slice()],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO app_secrets (key, nonce, ciphertext, aad, created_at, updated_at) \
             VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            rusqlite::params!["k", &[0u8; 12][..], &[0u8; 16][..], b"aad".as_slice()],
        );
        assert!(
            result.is_err(),
            "duplicate key must be rejected by uniqueness"
        );
    }

    #[test]
    fn migrations_are_idempotent_when_reapplied() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations().to_latest(&mut conn).unwrap();
        migrations()
            .to_latest(&mut conn)
            .expect("re-running to_latest on an up-to-date db is a no-op");
    }
}
