use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[path = "../src/core.rs"]
mod core;

use core::{
    fetch_sessions, get_brain_stats, get_pinned_id_for_workspace, normalize_workspace_path,
    remove_pin, resolve_brain_target, set_pin,
};

#[test]
fn test_normalize_workspace_path() {
    assert_eq!(
        normalize_workspace_path("file:///C:/Users/test/project/"),
        if cfg!(target_os = "windows") {
            "c:/users/test/project"
        } else {
            "C:/Users/test/project"
        }
    );
    assert_eq!(
        normalize_workspace_path("C:\\Users\\test\\project"),
        if cfg!(target_os = "windows") {
            "c:/users/test/project"
        } else {
            "C:/Users/test/project"
        }
    );
    assert_eq!(
        normalize_workspace_path("file:///home/user/code/"),
        "home/user/code"
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
