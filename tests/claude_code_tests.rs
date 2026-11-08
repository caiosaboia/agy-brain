#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[path = "../src/core.rs"]
mod core;

#[path = "../src/claude_code.rs"]
mod claude_code;

#[path = "../src/cli.rs"]
mod cli;

#[path = "../src/backend.rs"]
mod backend;

use backend::ProviderKind;
use claude_code::{
    delete_session, fetch_sessions, fetch_sessions_ext, get_brain_stats,
    get_pinned_id_for_workspace, prune_sessions, remove_pin, rename_session, resolve_brain_target,
    set_pin,
};

/// Mirrors Claude Code's own project-directory naming scheme (path
/// separators and `:` replaced with `-`). The real function lives private
/// inside src/claude_code.rs, so fixtures replicate it here the same way
/// core_tests.rs hand-builds SQLite rows matching core.rs's expectations.
fn encode_project_dir(path: &Path) -> String {
    path.to_string_lossy().replace(['\\', '/', ':'], "-")
}

fn user_entry(cwd: &str, text: &str, sidechain: bool, ts: &str) -> String {
    format!(
        r#"{{"type":"user","message":{{"role":"user","content":"{text}"}},"cwd":"{cwd}","sessionId":"s","uuid":"u","timestamp":"{ts}","isSidechain":{sidechain}}}"#
    )
}

fn user_entry_blocks(cwd: &str, text: &str, ts: &str) -> String {
    format!(
        r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"{text}"}}]}},"cwd":"{cwd}","timestamp":"{ts}","isSidechain":false}}"#
    )
}

fn assistant_entry(cwd: &str, sidechain: bool, ts: &str) -> String {
    format!(
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"ok"}}]}},"cwd":"{cwd}","timestamp":"{ts}","isSidechain":{sidechain}}}"#
    )
}

fn summary_entry(title: &str) -> String {
    format!(r#"{{"type":"summary","summary":"{title}","leafUuid":"x"}}"#)
}

fn write_session_file(projects_root: &Path, ws: &Path, id: &str, lines: &[String]) -> PathBuf {
    let project_dir = projects_root.join(encode_project_dir(ws));
    fs::create_dir_all(&project_dir).unwrap();
    let file_path = project_dir.join(format!("{id}.jsonl"));
    fs::write(&file_path, lines.join("\n") + "\n").unwrap();
    file_path
}

struct Fixture {
    _tmp: tempfile::TempDir,
    app_dir: PathBuf,
    ws: PathBuf,
}

impl Fixture {
    /// The workspace path as it would appear in a transcript's `cwd` field:
    /// forward slashes only, so it is valid to embed directly into a JSON
    /// string literal regardless of the host OS's path separator.
    fn ws_str(&self) -> String {
        self.ws.to_string_lossy().replace('\\', "/")
    }
}

fn setup() -> Fixture {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("claude_home");
    let ws = tmp.path().join("workspace");
    fs::create_dir_all(app_dir.join("projects")).unwrap();
    fs::create_dir_all(&ws).unwrap();
    Fixture {
        _tmp: tmp,
        app_dir,
        ws,
    }
}

#[test]
fn test_title_cascade_summary_preview_blocks_and_empty() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();

    // 1. Has a `summary` entry -> highest-priority title source.
    write_session_file(
        &projects_root,
        &fx.ws,
        "11111111-1111-1111-1111-111111111111",
        &[
            summary_entry("Fix the login flow"),
            user_entry(&ws_str, "hello", false, "2026-11-06T10:00:00.000Z"),
            assistant_entry(&ws_str, false, "2026-11-06T10:00:05.000Z"),
        ],
    );

    // 2. No summary, plain-string message content -> "preview".
    write_session_file(
        &projects_root,
        &fx.ws,
        "22222222-2222-2222-2222-222222222222",
        &[
            user_entry(
                &ws_str,
                "Refactor the auth module please",
                false,
                "2026-11-06T11:00:00.000Z",
            ),
            assistant_entry(&ws_str, false, "2026-11-06T11:00:05.000Z"),
        ],
    );

    // 3. No summary, array-of-blocks message content -> still resolves to "preview".
    write_session_file(
        &projects_root,
        &fx.ws,
        "33333333-3333-3333-3333-333333333333",
        &[
            user_entry_blocks(
                &ws_str,
                "Investigate the flaky test",
                "2026-11-06T12:00:00.000Z",
            ),
            assistant_entry(&ws_str, false, "2026-11-06T12:00:05.000Z"),
        ],
    );

    // 4. Empty file -> "(sem titulo)" / "empty", filtered out as a ghost.
    write_session_file(
        &projects_root,
        &fx.ws,
        "44444444-4444-4444-4444-444444444444",
        &[],
    );

    let sessions = fetch_sessions(Some(&fx.ws), false, Some(&fx.app_dir));
    assert_eq!(sessions.len(), 3);

    let s1 = sessions
        .iter()
        .find(|s| s.id == "11111111-1111-1111-1111-111111111111")
        .unwrap();
    assert_eq!(s1.title, "Fix the login flow");
    assert_eq!(s1.title_source, "summary");

    let s2 = sessions
        .iter()
        .find(|s| s.id == "22222222-2222-2222-2222-222222222222")
        .unwrap();
    assert_eq!(s2.title, "Refactor the auth module please");
    assert_eq!(s2.title_source, "preview");

    let s3 = sessions
        .iter()
        .find(|s| s.id == "33333333-3333-3333-3333-333333333333")
        .unwrap();
    assert_eq!(s3.title, "Investigate the flaky test");
    assert_eq!(s3.title_source, "preview");

    assert!(sessions
        .iter()
        .all(|s| s.id != "44444444-4444-4444-4444-444444444444"));
}

#[test]
fn test_step_counting_excludes_sidechains() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();

    write_session_file(
        &projects_root,
        &fx.ws,
        "55555555-5555-5555-5555-555555555555",
        &[
            summary_entry("Session with a subagent branch"),
            user_entry(&ws_str, "do the thing", false, "2026-11-06T09:00:00.000Z"),
            assistant_entry(&ws_str, false, "2026-11-06T09:00:05.000Z"),
            user_entry(
                &ws_str,
                "sidechain user turn",
                true,
                "2026-11-06T09:00:06.000Z",
            ),
            assistant_entry(&ws_str, true, "2026-11-06T09:00:07.000Z"),
            user_entry(
                &ws_str,
                "another sidechain turn",
                true,
                "2026-11-06T09:00:08.000Z",
            ),
            user_entry(&ws_str, "back to root", false, "2026-11-06T09:00:09.000Z"),
        ],
    );

    let sessions = fetch_sessions(Some(&fx.ws), false, Some(&fx.app_dir));
    let s = sessions
        .iter()
        .find(|s| s.id == "55555555-5555-5555-5555-555555555555")
        .unwrap();

    // 4 root-level entries (user, assistant, user) -> steps == 3, sidechains excluded.
    assert_eq!(s.steps, 3);
    assert!(!s.is_subagent);
    // Two sidechain entries form a single contiguous branch -> subagent_count == 1.
    assert_eq!(s.subagent_count, 1);
}

#[test]
fn test_mtime_from_last_root_timestamp() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();

    write_session_file(
        &projects_root,
        &fx.ws,
        "66666666-6666-6666-6666-666666666666",
        &[
            summary_entry("Timestamped session"),
            user_entry(&ws_str, "first", false, "2026-11-06T08:00:00.000Z"),
            assistant_entry(&ws_str, false, "2026-11-06T08:30:45.000Z"),
        ],
    );

    let sessions = fetch_sessions(Some(&fx.ws), false, Some(&fx.app_dir));
    let s = sessions
        .iter()
        .find(|s| s.id == "66666666-6666-6666-6666-666666666666")
        .unwrap();
    assert_eq!(s.mtime, "2026-11-06 08:30:45");
    assert_eq!(s.status, "IDLE");
}

#[test]
fn test_status_waiting_when_last_entry_is_user() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();

    write_session_file(
        &projects_root,
        &fx.ws,
        "77777777-7777-7777-7777-777777777777",
        &[
            summary_entry("Awaiting a reply"),
            assistant_entry(&ws_str, false, "2026-11-06T08:00:00.000Z"),
            user_entry(&ws_str, "one more thing", false, "2026-11-06T08:05:00.000Z"),
        ],
    );

    let sessions = fetch_sessions(Some(&fx.ws), false, Some(&fx.app_dir));
    let s = sessions
        .iter()
        .find(|s| s.id == "77777777-7777-7777-7777-777777777777")
        .unwrap();
    assert_eq!(s.status, "WAITING");
}

#[test]
fn test_strict_workspace_isolation_via_recorded_cwd() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();
    let other_ws = PathBuf::from("C:/Users/dev/other");

    write_session_file(
        &projects_root,
        &fx.ws,
        "88888888-8888-8888-8888-888888888888",
        &[
            summary_entry("In the target workspace"),
            user_entry(&ws_str, "hi", false, "2026-11-06T10:00:00.000Z"),
        ],
    );
    write_session_file(
        &projects_root,
        &other_ws,
        "99999999-9999-9999-9999-999999999999",
        &[
            summary_entry("In a different workspace"),
            user_entry(
                "C:/Users/dev/other",
                "hi",
                false,
                "2026-11-06T10:00:00.000Z",
            ),
        ],
    );

    let sessions = fetch_sessions(Some(&fx.ws), false, Some(&fx.app_dir));
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "88888888-8888-8888-8888-888888888888");

    let all = fetch_sessions(Some(&fx.ws), true, Some(&fx.app_dir));
    assert_eq!(all.len(), 2);
}

#[test]
fn test_ghost_filtering_and_prune_dry_run_and_real() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();

    write_session_file(
        &projects_root,
        &fx.ws,
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        &[
            summary_entry("Real session"),
            user_entry(&ws_str, "do real work", false, "2026-11-06T10:00:00.000Z"),
            assistant_entry(&ws_str, false, "2026-11-06T10:00:05.000Z"),
        ],
    );
    let ghost_path = write_session_file(
        &projects_root,
        &fx.ws,
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        &[],
    );

    let dry = prune_sessions(true, Some(&fx.app_dir)).unwrap();
    assert_eq!(dry.len(), 1);
    assert_eq!(dry[0].id, "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    assert!(ghost_path.exists());

    let real = prune_sessions(false, Some(&fx.app_dir)).unwrap();
    assert_eq!(real.len(), 1);
    assert!(!ghost_path.exists());

    let sessions = fetch_sessions(Some(&fx.ws), false, Some(&fx.app_dir));
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
}

#[test]
fn test_pin_unpin_round_trip() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");

    write_session_file(
        &projects_root,
        &fx.ws,
        "cccccccc-cccc-cccc-cccc-cccccccccccc",
        &[summary_entry("Pin me")],
    );

    assert!(get_pinned_id_for_workspace(Some(&fx.ws), Some(&fx.app_dir)).is_none());

    set_pin(
        "cccccccc-cccc-cccc-cccc-cccccccccccc",
        Some(&fx.ws),
        Some(&fx.app_dir),
    )
    .unwrap();

    assert_eq!(
        get_pinned_id_for_workspace(Some(&fx.ws), Some(&fx.app_dir)),
        Some("cccccccc-cccc-cccc-cccc-cccccccccccc".to_string())
    );
    assert!(fx.ws.join(".claude-brain").is_file());
    assert!(fx.app_dir.join("claude_workspace_pins.json").is_file());
    // Distinct from Antigravity's own pin files - no collision.
    assert!(!fx.ws.join(".brain").exists());
    assert!(!fx.app_dir.join("workspace_pins.json").exists());

    assert!(remove_pin(Some(&fx.ws), Some(&fx.app_dir)));
    assert!(get_pinned_id_for_workspace(Some(&fx.ws), Some(&fx.app_dir)).is_none());
    assert!(!fx.ws.join(".claude-brain").exists());
}

#[test]
fn test_rename_via_sidecar_overrides_summary() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();

    write_session_file(
        &projects_root,
        &fx.ws,
        "dddddddd-dddd-dddd-dddd-dddddddddddd",
        &[
            summary_entry("Original Claude Code summary"),
            user_entry(&ws_str, "hello", false, "2026-11-06T10:00:00.000Z"),
        ],
    );

    let sessions = fetch_sessions(Some(&fx.ws), false, Some(&fx.app_dir));
    assert_eq!(sessions[0].title, "Original Claude Code summary");
    assert_eq!(sessions[0].title_source, "summary");

    let (resolved_id, saved_title) = rename_session(
        "dddddddd-dddd-dddd-dddd-dddddddddddd",
        "My Custom Title",
        Some(&fx.ws),
        Some(&fx.app_dir),
    )
    .unwrap();
    assert_eq!(resolved_id, "dddddddd-dddd-dddd-dddd-dddddddddddd");
    assert_eq!(saved_title, "My Custom Title");
    assert!(fx.app_dir.join("agy_titles.json").is_file());

    let updated = fetch_sessions(Some(&fx.ws), false, Some(&fx.app_dir));
    assert_eq!(updated[0].title, "My Custom Title");
    assert_eq!(updated[0].title_source, "title");

    // Never mutates Claude Code's own transcript file.
    let raw = fs::read_to_string(
        projects_root
            .join(encode_project_dir(&fx.ws))
            .join("dddddddd-dddd-dddd-dddd-dddddddddddd.jsonl"),
    )
    .unwrap();
    assert!(raw.contains("Original Claude Code summary"));
    assert!(!raw.contains("My Custom Title"));
}

#[test]
fn test_delete_removes_transcript_todos_and_sidecars() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let id = "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee";

    let transcript = write_session_file(&projects_root, &fx.ws, id, &[summary_entry("To delete")]);

    let todos_dir = fx.app_dir.join("todos");
    fs::create_dir_all(&todos_dir).unwrap();
    let todo_file = todos_dir.join(format!("{id}-agent-1.json"));
    fs::write(&todo_file, "[]").unwrap();

    set_pin(id, Some(&fx.ws), Some(&fx.app_dir)).unwrap();
    rename_session("default", "Doomed Session", Some(&fx.ws), Some(&fx.app_dir)).unwrap();

    let res = delete_session(id, Some(&fx.ws), Some(&fx.app_dir)).unwrap();

    assert_eq!(res.conversation_id, id);
    assert_eq!(res.title, "Doomed Session");
    assert!(res.db_deleted);
    assert!(res.folder_deleted);
    assert!(res.pins_removed);

    assert!(!transcript.exists());
    assert!(!todo_file.exists());
    assert!(!fx.ws.join(".claude-brain").exists());
    assert!(get_pinned_id_for_workspace(Some(&fx.ws), Some(&fx.app_dir)).is_none());
}

#[test]
fn test_resolve_target_index_uuid_default_and_prefix() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();

    write_session_file(
        &projects_root,
        &fx.ws,
        "ffffffff-ffff-ffff-ffff-ffffffffffff",
        &[
            summary_entry("First"),
            user_entry(&ws_str, "x", false, "2026-11-06T10:00:00.000Z"),
            user_entry(&ws_str, "y", false, "2026-11-06T10:01:00.000Z"),
        ],
    );
    write_session_file(
        &projects_root,
        &fx.ws,
        "00000001-0001-0001-0001-000000000001",
        &[
            summary_entry("Second"),
            user_entry(&ws_str, "x", false, "2026-11-06T09:00:00.000Z"),
        ],
    );

    // Default -> highest step-count session (no pin set).
    let resolved = resolve_brain_target(Some("default"), Some(&fx.ws), Some(&fx.app_dir));
    assert_eq!(
        resolved,
        Some("ffffffff-ffff-ffff-ffff-ffffffffffff".to_string())
    );

    // Index -> second entry in the sorted list.
    let resolved_idx = resolve_brain_target(Some("1"), Some(&fx.ws), Some(&fx.app_dir));
    assert_eq!(
        resolved_idx,
        Some("00000001-0001-0001-0001-000000000001".to_string())
    );

    // Already-UUID-shaped target is returned as-is.
    let direct = "12345678-1234-1234-1234-123456789abc";
    let resolved_uuid = resolve_brain_target(Some(direct), Some(&fx.ws), Some(&fx.app_dir));
    assert_eq!(resolved_uuid, Some(direct.to_string()));

    // Prefix match (non-numeric-looking prefix, so it isn't treated as an index).
    let resolved_prefix =
        resolve_brain_target(Some("00000001-0001"), Some(&fx.ws), Some(&fx.app_dir));
    assert_eq!(
        resolved_prefix,
        Some("00000001-0001-0001-0001-000000000001".to_string())
    );
}

#[test]
fn test_get_brain_stats_reports_transcript_and_todo_files() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let id = "12121212-1212-1212-1212-121212121212";

    write_session_file(
        &projects_root,
        &fx.ws,
        id,
        &[summary_entry("Has artifacts")],
    );
    let todos_dir = fx.app_dir.join("todos");
    fs::create_dir_all(&todos_dir).unwrap();
    fs::write(todos_dir.join(format!("{id}-agent-1.json")), "[]").unwrap();

    let stats = get_brain_stats(id, Some(&fx.app_dir)).unwrap();
    assert!(stats.exists);
    assert_eq!(stats.md_count, 0);
    assert_eq!(stats.png_count, 0);
    assert_eq!(stats.total_files, 2);
    assert_eq!(stats.recent_artifacts.len(), 2);
    assert!(stats
        .recent_artifacts
        .iter()
        .any(|a| a.category == "transcript"));
    assert!(stats.recent_artifacts.iter().any(|a| a.category == "todo"));

    assert!(get_brain_stats("no-such-session", Some(&fx.app_dir)).is_none());
}

#[test]
fn test_provider_auto_detection_from_app_dir_shape() {
    let tmp = tempdir().unwrap();

    let antigravity_dir = tmp.path().join("antigravity_home");
    fs::create_dir_all(&antigravity_dir).unwrap();
    fs::write(antigravity_dir.join("conversation_summaries.db"), b"").unwrap();
    assert_eq!(
        backend::detect_provider(None, Some(&antigravity_dir)),
        ProviderKind::Antigravity
    );

    let claude_dir = tmp.path().join("claude_home");
    fs::create_dir_all(claude_dir.join("projects")).unwrap();
    assert_eq!(
        backend::detect_provider(None, Some(&claude_dir)),
        ProviderKind::ClaudeCode
    );

    let empty_dir = tmp.path().join("empty_home");
    fs::create_dir_all(&empty_dir).unwrap();
    assert_eq!(
        backend::detect_provider(None, Some(&empty_dir)),
        ProviderKind::Antigravity
    );

    // An explicit provider always wins over the shape of --app-dir.
    assert_eq!(
        backend::detect_provider(Some(ProviderKind::ClaudeCode), Some(&antigravity_dir)),
        ProviderKind::ClaudeCode
    );
}

#[test]
fn test_fetch_sessions_ext_subagents_flag_is_a_no_op() {
    let fx = setup();
    let projects_root = fx.app_dir.join("projects");
    let ws_str = fx.ws_str();

    write_session_file(
        &projects_root,
        &fx.ws,
        "13131313-1313-1313-1313-131313131313",
        &[
            summary_entry("Has a sidechain"),
            user_entry(&ws_str, "x", false, "2026-11-06T10:00:00.000Z"),
            user_entry(&ws_str, "sub", true, "2026-11-06T10:00:01.000Z"),
        ],
    );

    let without_flag = fetch_sessions_ext(Some(&fx.ws), false, false, Some(&fx.app_dir));
    let with_flag = fetch_sessions_ext(Some(&fx.ws), false, true, Some(&fx.app_dir));
    assert_eq!(without_flag.len(), with_flag.len());
    assert_eq!(without_flag[0].id, with_flag[0].id);
}
