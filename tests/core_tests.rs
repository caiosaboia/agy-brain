use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[path = "../src/core.rs"]
mod core;

use core::{
    delete_session, extract_role_from_raw_summary, fetch_sessions, fetch_sessions_ext,
    get_brain_stats, get_pinned_id_for_workspace, normalize_workspace_path, prune_sessions,
    remove_pin, rename_session, resolve_brain_target, set_pin,
};

#[test]
fn test_normalize_workspace_path() {
    assert_eq!(
        normalize_workspace_path("file:///C:/Users/test/project/"),
        if cfg!(target_os = "windows") {
            "c:/users/test/project"
        } else {
            "c:/Users/test/project"
        }
    );
    assert_eq!(
        normalize_workspace_path("C:\\Users\\test\\project"),
        if cfg!(target_os = "windows") {
            "c:/users/test/project"
        } else {
            "c:/Users/test/project"
        }
    );
    assert_eq!(
        normalize_workspace_path("file:///home/user/code/"),
        "home/user/code"
    );
    assert_eq!(
        normalize_workspace_path("/home/user/code/"),
        "home/user/code"
    );
    assert_eq!(
        normalize_workspace_path("/private/var/folders/xx/test/"),
        "var/folders/xx/test"
    );
    assert_eq!(
        normalize_workspace_path("file:///private/var/folders/xx/test"),
        "var/folders/xx/test"
    );
    assert_eq!(
        normalize_workspace_path("/var/folders/xx/test"),
        "var/folders/xx/test"
    );
    assert_eq!(normalize_workspace_path(""), "");
}

#[test]
fn test_pin_management() {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    let ws_dir = tmp.path().join("my_project");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&ws_dir).unwrap();

    let brain_uuid = "12345678-1234-1234-1234-123456789abc";

    // Initially no pin
    assert!(get_pinned_id_for_workspace(Some(&ws_dir), Some(&app_dir)).is_none());

    // Set pin
    set_pin(brain_uuid, Some(&ws_dir), Some(&app_dir)).unwrap();
    assert_eq!(
        get_pinned_id_for_workspace(Some(&ws_dir), Some(&app_dir)),
        Some(brain_uuid.to_string())
    );

    // Verify local .brain
    let dot_brain = ws_dir.join(".brain");
    assert!(dot_brain.is_file());
    assert_eq!(fs::read_to_string(&dot_brain).unwrap().trim(), brain_uuid);

    // Remove pin
    assert!(remove_pin(Some(&ws_dir), Some(&app_dir)));
    assert!(get_pinned_id_for_workspace(Some(&ws_dir), Some(&app_dir)).is_none());
    assert!(!dot_brain.is_file());
}

#[test]
fn test_fetch_sessions_with_sqlite() {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    fs::create_dir_all(&app_dir).unwrap();
    let db_path = app_dir.join("conversation_summaries.db");

    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE conversation_summaries (
            conversation_id text PRIMARY KEY,
            title text NOT NULL DEFAULT '',
            preview text NOT NULL DEFAULT '',
            step_count integer NOT NULL DEFAULT 0,
            last_modified_time datetime NOT NULL,
            workspace_uris text NOT NULL,
            status text NOT NULL DEFAULT ''
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('uuid-1111-1111-1111-111111111111', 'Main Session', 'Preview', 500, '2026-11-05 12:00:00', '[\"file:///C:/Users/dev/repo\"]', 'IDLE'),
        ('uuid-2222-2222-2222-222222222222', 'Small Session', 'Preview', 10, '2026-11-05 13:00:00', '[\"file:///C:/Users/dev/repo\"]', 'IDLE'),
        ('uuid-3333-3333-3333-333333333333', 'Other Project', 'Preview', 200, '2026-11-05 14:00:00', '[\"file:///C:/Users/dev/other\"]', 'IDLE')",
        [],
    ).unwrap();
    drop(conn);

    let ws = PathBuf::from("C:/Users/dev/repo");

    // Fetch for workspace
    let sessions = fetch_sessions(Some(&ws), false, Some(&app_dir), Some(&db_path));
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, "uuid-1111-1111-1111-111111111111");
    assert_eq!(sessions[0].steps, 500);
    assert_eq!(sessions[1].id, "uuid-2222-2222-2222-222222222222");

    // Fetch all
    let all = fetch_sessions(Some(&ws), true, Some(&app_dir), Some(&db_path));
    assert_eq!(all.len(), 3);
}

#[test]
fn test_get_brain_stats() {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    let target_brain = app_dir.join("brain").join("test-brain-uuid");
    fs::create_dir_all(&target_brain).unwrap();

    fs::write(target_brain.join("plan.md"), "# Plan").unwrap();
    fs::write(target_brain.join("plan.md.metadata.json"), "{}").unwrap();
    fs::write(target_brain.join("screen.png"), b"\x89PNG").unwrap();

    let scratch = target_brain.join("scratch");
    fs::create_dir_all(&scratch).unwrap();
    fs::write(scratch.join("temp.py"), "print(1)").unwrap();

    let stats = get_brain_stats("test-brain-uuid", Some(&app_dir)).unwrap();
    assert!(stats.exists);
    assert_eq!(stats.md_count, 1);
    assert_eq!(stats.png_count, 1);
    assert_eq!(stats.scratch_count, 1);
    assert_eq!(stats.recent_artifacts.len(), 1);
    assert_eq!(stats.recent_artifacts[0].name, "plan.md");
}

#[test]
fn test_resolve_brain_target() {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    fs::create_dir_all(&app_dir).unwrap();
    let db_path = app_dir.join("conversation_summaries.db");

    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE conversation_summaries (
            conversation_id text PRIMARY KEY,
            title text NOT NULL DEFAULT '',
            preview text NOT NULL DEFAULT '',
            step_count integer NOT NULL DEFAULT 0,
            last_modified_time datetime NOT NULL,
            workspace_uris text NOT NULL,
            status text NOT NULL DEFAULT ''
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('uuid-1111-1111-1111-111111111111', 'First', '', 100, '2026-11-05 12:00:00', '[\"file:///C:/Users/dev/repo\"]', 'IDLE'),
        ('uuid-2222-2222-2222-222222222222', 'Second', '', 50, '2026-11-05 13:00:00', '[\"file:///C:/Users/dev/repo\"]', 'IDLE')",
        [],
    ).unwrap();
    drop(conn);

    let ws = PathBuf::from("C:/Users/dev/repo");

    // Default target
    let resolved = resolve_brain_target(Some("default"), Some(&ws), Some(&app_dir));
    assert_eq!(
        resolved,
        Some("uuid-1111-1111-1111-111111111111".to_string())
    );

    // Index target
    let resolved_idx = resolve_brain_target(Some("1"), Some(&ws), Some(&app_dir));
    assert_eq!(
        resolved_idx,
        Some("uuid-2222-2222-2222-222222222222".to_string())
    );

    // UUID target
    let direct_uuid = "99999999-9999-9999-9999-999999999999";
    let resolved_uuid = resolve_brain_target(Some(direct_uuid), Some(&ws), Some(&app_dir));
    assert_eq!(resolved_uuid, Some(direct_uuid.to_string()));
}

#[test]
fn test_delete_session() {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    let ws_dir = tmp.path().join("my_project");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&ws_dir).unwrap();

    let db_path = app_dir.join("conversation_summaries.db");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE conversation_summaries (
            conversation_id text PRIMARY KEY,
            title text NOT NULL DEFAULT '',
            preview text NOT NULL DEFAULT '',
            step_count integer NOT NULL DEFAULT 0,
            last_modified_time datetime NOT NULL,
            workspace_uris text NOT NULL,
            status text NOT NULL DEFAULT ''
        )",
        [],
    )
    .unwrap();

    let target_uuid = "44444444-4444-4444-4444-444444444444";
    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('44444444-4444-4444-4444-444444444444', 'Session to Delete', '', 42, '2026-11-05 12:00:00', '[\"file:///C:/Users/dev/repo\"]', 'IDLE')",
        [],
    )
    .unwrap();
    drop(conn);

    // Create brain folder on disk
    let brain_dir = app_dir.join("brain").join(target_uuid);
    fs::create_dir_all(&brain_dir).unwrap();
    fs::write(brain_dir.join("artifact.md"), "# Delete me").unwrap();

    // Create local .brain pin
    fs::write(ws_dir.join(".brain"), target_uuid).unwrap();

    // Perform deletion
    let res = delete_session(target_uuid, Some(&ws_dir), Some(&app_dir), Some(&db_path)).unwrap();

    assert_eq!(res.conversation_id, target_uuid);
    assert_eq!(res.title, "Session to Delete");
    assert!(res.db_deleted);
    assert!(res.folder_deleted);
    assert!(res.pins_removed);

    // Verify database record is gone
    let conn = Connection::open(&db_path).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversation_summaries WHERE conversation_id = ?1",
            [target_uuid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);

    // Verify folder on disk is deleted
    assert!(!brain_dir.exists());

    // Verify pin was removed
    assert!(!ws_dir.join(".brain").exists());
}

#[test]
fn test_extract_role_from_raw_summary() {
    let mut raw = Vec::new();
    raw.extend_from_slice(
        b"prefix_bytes_B-\n\x04self\x12\x1eWorkspace Parity UI Specialist8\xa6\n",
    );
    let role = extract_role_from_raw_summary(&raw, "self");
    assert_eq!(role, Some("Workspace Parity UI Specialist".to_string()));

    // Also works when agent_name is empty by scanning for the B..\n..\x12 pattern
    let role_auto = extract_role_from_raw_summary(&raw, "");
    assert_eq!(
        role_auto,
        Some("Workspace Parity UI Specialist".to_string())
    );
}

#[test]
fn test_title_cascade_subagents_and_prune() {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    fs::create_dir_all(&app_dir).unwrap();
    let db_path = app_dir.join("conversation_summaries.db");

    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE conversation_summaries (
            conversation_id text PRIMARY KEY,
            title text NOT NULL DEFAULT '',
            preview text NOT NULL DEFAULT '',
            step_count integer NOT NULL DEFAULT 0,
            last_modified_time datetime NOT NULL,
            workspace_uris text NOT NULL,
            status text NOT NULL DEFAULT '',
            agent_name text NOT NULL DEFAULT '',
            parent_conversation_id text NOT NULL DEFAULT '',
            nesting_depth integer NOT NULL DEFAULT 0,
            raw_summary BLOB
        )",
        [],
    )
    .unwrap();

    // 1. Root session with preview only (title = '')
    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('root-1111-1111-1111-111111111111', '', 'Extracao de Dados Pipefy CSV\nLinha extra', 500, '2026-11-05 12:00:00', '[\"file:///C:/Users/dev/repo\"]', 'IDLE', '', '', 0, NULL)",
        [],
    ).unwrap();

    // 2. Subagent child of root-1111 with Role inside raw_summary blob
    let raw_blob: Vec<u8> = b"headerB)\n\x04self\x12\x1aCLI Doublecheck Specialistfooter".to_vec();
    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('sub-2222-2222-2222-222222222222', '', '', 80, '2026-11-05 12:30:00', '[\"file:///C:/Users/dev/repo\"]', 'IDLE', 'self', 'root-1111-1111-1111-111111111111', 1, ?1)",
        rusqlite::params![raw_blob],
    ).unwrap();

    // 3. Root session with transcript fallback
    let trans_id = "root-3333-3333-3333-333333333333";
    let logs_dir = app_dir
        .join("brain")
        .join(trans_id)
        .join(".system_generated")
        .join("logs");
    fs::create_dir_all(&logs_dir).unwrap();
    fs::write(
        logs_dir.join("transcript.jsonl"),
        "{\"type\":\"USER_INPUT\",\"content\":\"<USER_REQUEST>\\nConfigurar ambiente Rust\\n</USER_REQUEST>\"}\n",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        (?1, '', '', 25, '2026-11-05 13:00:00', '[\"file:///C:/Users/dev/repo\"]', 'IDLE', '', '', 0, NULL)",
        rusqlite::params![trans_id],
    ).unwrap();

    // 4. Ghost session (0 steps, no files)
    let ghost_id = "ghost-4444-4444-4444-444444444444";
    fs::create_dir_all(app_dir.join("brain").join(ghost_id)).unwrap();
    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        (?1, '', '', 0, '0001-01-01 00:00:00', '[\"file:///C:/Users/dev/repo\"]', '', '', '', 0, NULL)",
        rusqlite::params![ghost_id],
    ).unwrap();
    drop(conn);

    let ws = PathBuf::from("C:/Users/dev/repo");

    // Default fetch: hides subagent and ghost session
    let root_sessions = fetch_sessions(Some(&ws), false, Some(&app_dir), Some(&db_path));
    assert_eq!(root_sessions.len(), 2);
    assert_eq!(root_sessions[0].id, "root-1111-1111-1111-111111111111");
    assert_eq!(root_sessions[0].title, "Extracao de Dados Pipefy CSV");
    assert_eq!(root_sessions[0].title_source, "preview");
    assert_eq!(root_sessions[0].subagent_count, 1);

    assert_eq!(root_sessions[1].id, trans_id);
    assert_eq!(root_sessions[1].title, "Configurar ambiente Rust");
    assert_eq!(root_sessions[1].title_source, "transcript");

    // Fetch including subagents
    let all_with_sub = fetch_sessions_ext(Some(&ws), false, true, Some(&app_dir), Some(&db_path));
    assert_eq!(all_with_sub.len(), 3);
    let sub = all_with_sub
        .iter()
        .find(|s| s.id == "sub-2222-2222-2222-222222222222")
        .unwrap();
    assert!(sub.is_subagent);
    assert_eq!(sub.title, "CLI Doublecheck Specialist");
    assert_eq!(sub.title_source, "raw_summary");
    assert_eq!(
        sub.parent_title.as_deref(),
        Some("Extracao de Dados Pipefy CSV")
    );

    // Rename session
    let (renamed_id, new_title) = rename_session(
        "root-1111-1111-1111-111111111111",
        "Pipeline Pipefy Oficial",
        Some(&ws),
        Some(&app_dir),
        Some(&db_path),
    )
    .unwrap();
    assert_eq!(renamed_id, "root-1111-1111-1111-111111111111");
    assert_eq!(new_title, "Pipeline Pipefy Oficial");

    let updated = fetch_sessions(Some(&ws), false, Some(&app_dir), Some(&db_path));
    assert_eq!(updated[0].title, "Pipeline Pipefy Oficial");
    assert_eq!(updated[0].title_source, "title");

    // Prune ghost sessions (dry_run first, then actual)
    let dry = prune_sessions(true, Some(&app_dir), Some(&db_path)).unwrap();
    assert_eq!(dry.len(), 1);
    assert_eq!(dry[0].id, ghost_id);
    assert!(app_dir.join("brain").join(ghost_id).exists());

    let pruned = prune_sessions(false, Some(&app_dir), Some(&db_path)).unwrap();
    assert_eq!(pruned.len(), 1);
    assert!(!app_dir.join("brain").join(ghost_id).exists());
}

#[test]
fn test_normalize_url_encoded_workspace_path() {
    assert_eq!(
        normalize_workspace_path("file:///c%3A/Users/dev/Meu%20Projeto/"),
        if cfg!(target_os = "windows") {
            "c:/users/dev/meu projeto"
        } else {
            "c:/Users/dev/Meu Projeto"
        }
    );
}

#[test]
fn test_strict_workspace_isolation() {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    fs::create_dir_all(&app_dir).unwrap();
    let db_path = app_dir.join("conversation_summaries.db");

    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE conversation_summaries (
            conversation_id text PRIMARY KEY,
            title text NOT NULL DEFAULT '',
            preview text NOT NULL DEFAULT '',
            step_count integer NOT NULL DEFAULT 0,
            last_modified_time datetime NOT NULL,
            workspace_uris text NOT NULL,
            status text NOT NULL DEFAULT ''
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('parent-1111-1111-1111-111111111111', 'Home Session', '', 500, '2026-11-05 12:00:00', '[\"file:///C:/Users/cocad\"]', 'IDLE'),
        ('exact-2222-2222-2222-222222222222', 'Project Session', '', 100, '2026-11-05 13:00:00', '[\"file:///c%3A/Users/cocad/projetos/my-app\"]', 'IDLE'),
        ('child-3333-3333-3333-333333333333', 'Subfolder Session', '', 50, '2026-11-05 14:00:00', '[\"file:///C:/Users/cocad/projetos/my-app/sub\"]', 'IDLE')",
        [],
    )
    .unwrap();
    drop(conn);

    let ws = PathBuf::from("C:/Users/cocad/projetos/my-app");
    let sessions = fetch_sessions(Some(&ws), false, Some(&app_dir), Some(&db_path));

    // Must ONLY return the exact directory session (and decode %3A), never parent or child folders
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "exact-2222-2222-2222-222222222222");
}

#[test]
fn test_set_pin_updates_sqlite_workspace_uris() {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    let ws_dir = tmp.path().join("new_repo");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&ws_dir).unwrap();

    let db_path = app_dir.join("conversation_summaries.db");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE conversation_summaries (
            conversation_id text PRIMARY KEY,
            title text NOT NULL DEFAULT '',
            preview text NOT NULL DEFAULT '',
            step_count integer NOT NULL DEFAULT 0,
            last_modified_time datetime NOT NULL,
            workspace_uris text NOT NULL,
            status text NOT NULL DEFAULT ''
        )",
        [],
    )
    .unwrap();

    let target_uuid = "88888888-8888-8888-8888-888888888888";
    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        (?1, 'Cross Workspace Session', '', 120, '2026-11-05 12:00:00', '[\"file:///C:/Users/cocad/projetos/other\"]', 'IDLE')",
        rusqlite::params![target_uuid],
    )
    .unwrap();
    drop(conn);

    // Pin session to ws_dir
    set_pin(target_uuid, Some(&ws_dir), Some(&app_dir)).unwrap();

    // Even after removing the pin file, the session now has ws_dir linked in SQLite workspace_uris
    remove_pin(Some(&ws_dir), Some(&app_dir));

    let sessions = fetch_sessions(Some(&ws_dir), false, Some(&app_dir), Some(&db_path));
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, target_uuid);
}
