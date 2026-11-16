use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn brain_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_brain"))
}

fn setup_antigravity_env() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    let ws_dir = tmp.path().join("workspace_main");
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
            status text NOT NULL DEFAULT '',
            agent_name text NOT NULL DEFAULT '',
            parent_conversation_id text NOT NULL DEFAULT '',
            nesting_depth integer NOT NULL DEFAULT 0,
            raw_summary BLOB
        )",
        [],
    )
    .unwrap();

    let ws_uri = format!(
        "[\"file:///{}\"]",
        ws_dir
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );

    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('11111111-1111-1111-1111-111111111111', 'Target Session', 'Preview', 50, '2026-11-16 10:00:00', ?1, 'IDLE', '', '', 0, NULL)",
        rusqlite::params![ws_uri],
    )
    .unwrap();
    drop(conn);

    let brain_dir = app_dir
        .join("brain")
        .join("11111111-1111-1111-1111-111111111111");
    fs::create_dir_all(&brain_dir).unwrap();
    fs::write(brain_dir.join("test_plan.md"), "# Antigravity Plan\nContent here.").unwrap();
    fs::write(brain_dir.join("other_artifact.txt"), "not a plan").unwrap();
    fs::write(brain_dir.join("image.png"), b"\x89PNG").unwrap();

    // Also write a plan directly in workspace
    fs::write(ws_dir.join("workspace_plan.md"), "# Workspace Plan").unwrap();

    (tmp, app_dir, ws_dir)
}

fn setup_claude_code_env() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("claude_home");
    let ws_dir = tmp.path().join("workspace_claude");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&ws_dir).unwrap();

    let projects_dir = app_dir.join("projects").join("test_project");
    fs::create_dir_all(&projects_dir).unwrap();

    let plans_dir = app_dir.join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    fs::write(plans_dir.join("agile-sprint-plan.md"), "# Claude Plan Slug").unwrap();

    let transcript_path = projects_dir.join("cccccccc-cccc-cccc-cccc-cccccccccccc.jsonl");
    let line1 = serde_json::json!({
        "type": "user",
        "cwd": ws_dir.to_string_lossy(),
        "slug": "agile-sprint-plan",
        "timestamp": "2026-11-16T12:00:00Z"
    });
    fs::write(&transcript_path, format!("{}\n", line1)).unwrap();

    fs::write(ws_dir.join("local_task_plan.md"), "# Local Task Plan").unwrap();

    (tmp, app_dir, ws_dir)
}

#[test]
fn test_harness_antigravity_plan_paths_and_json() {
    let (_tmp, app_dir, ws_dir) = setup_antigravity_env();

    // 1. Test `brain plan --paths`
    let out_paths = Command::new(brain_bin())
        .arg("--app-dir")
        .arg(&app_dir)
        .current_dir(&ws_dir)
        .args(["plan", "--paths"])
        .output()
        .unwrap();

    assert!(out_paths.status.success());
    let stdout_paths = String::from_utf8_lossy(&out_paths.stdout);
    assert!(stdout_paths.contains("test_plan.md"));
    assert!(!stdout_paths.contains("image.png"));

    // 2. Test `brain plan --json`
    let out_json = Command::new(brain_bin())
        .arg("--app-dir")
        .arg(&app_dir)
        .current_dir(&ws_dir)
        .args(["plan", "--json"])
        .output()
        .unwrap();

    assert!(out_json.status.success());
    let stdout_json = String::from_utf8_lossy(&out_json.stdout);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(stdout_json.trim()).unwrap();
    assert!(!parsed.is_empty());
    assert!(parsed.iter().any(|p| p["filename"] == "test_plan.md"));
    assert!(parsed.iter().any(|p| p["provider"] == "antigravity"));
}

#[test]
fn test_harness_antigravity_plan_cat() {
    let (_tmp, app_dir, ws_dir) = setup_antigravity_env();

    let out = Command::new(brain_bin())
        .arg("--app-dir")
        .arg(&app_dir)
        .current_dir(&ws_dir)
        .args(["plan", "11111111-1111-1111-1111-111111111111", "--cat"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("# Antigravity Plan"));
    assert!(stdout.contains("Content here."));
}

#[test]
fn test_harness_artifacts_plan_filter() {
    let (_tmp, app_dir, ws_dir) = setup_antigravity_env();

    let out = Command::new(brain_bin())
        .arg("--app-dir")
        .arg(&app_dir)
        .current_dir(&ws_dir)
        .args(["artifacts", "11111111-1111-1111-1111-111111111111", "--plan", "--paths"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("test_plan.md"));
    assert!(!stdout.contains("image.png"));
    assert!(!stdout.contains("other_artifact.txt"));
}

#[test]
fn test_harness_list_json_and_paths() {
    let (_tmp, app_dir, ws_dir) = setup_antigravity_env();

    // List with --json
    let out_json = Command::new(brain_bin())
        .arg("--app-dir")
        .arg(&app_dir)
        .current_dir(&ws_dir)
        .args(["list", "--json", "--all"])
        .output()
        .unwrap();

    assert!(out_json.status.success());
    let stdout_json = String::from_utf8_lossy(&out_json.stdout);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(stdout_json.trim()).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["id"], "11111111-1111-1111-1111-111111111111");

    // List with --paths
    let out_paths = Command::new(brain_bin())
        .arg("--app-dir")
        .arg(&app_dir)
        .current_dir(&ws_dir)
        .args(["list", "--paths", "--all"])
        .output()
        .unwrap();

    assert!(out_paths.status.success());
    let stdout_paths = String::from_utf8_lossy(&out_paths.stdout);
    assert!(stdout_paths.contains("11111111-1111-1111-1111-111111111111"));
}

#[test]
fn test_harness_info_json() {
    let (_tmp, app_dir, ws_dir) = setup_antigravity_env();

    let out = Command::new(brain_bin())
        .arg("--app-dir")
        .arg(&app_dir)
        .current_dir(&ws_dir)
        .args(["info", "11111111-1111-1111-1111-111111111111", "--json"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["id"], "11111111-1111-1111-1111-111111111111");
    assert_eq!(parsed["session"]["title"], "Target Session");
    assert!(parsed["stats"]["exists"].as_bool().unwrap());
}

#[test]
fn test_harness_claude_code_plan_hybrid() {
    let (_tmp, app_dir, ws_dir) = setup_claude_code_env();

    let out = Command::new(brain_bin())
        .arg("--app-dir")
        .arg(&app_dir)
        .arg("--provider")
        .arg("claude-code")
        .current_dir(&ws_dir)
        .args(["plan", "--all", "--paths"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("agile-sprint-plan.md"));
    assert!(stdout.contains("local_task_plan.md"));
}
