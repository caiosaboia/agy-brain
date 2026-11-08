use rusqlite::Connection;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn agy_brain_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_agy-brain"))
}

fn setup_e2e_env() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    let ws_dir = tmp.path().join("workspace_main");
    let other_ws = tmp.path().join("workspace_other");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&ws_dir).unwrap();
    fs::create_dir_all(&other_ws).unwrap();

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
    let other_uri = format!(
        "[\"file:///{}\"]",
        other_ws
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );

    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('11111111-1111-1111-1111-111111111111', 'Main Workspace Session', 'Preview 1', 150, '2026-11-06 10:00:00', ?1, 'IDLE', '', '', 0, NULL),
        ('22222222-2222-2222-2222-222222222222', 'Subagent Session', '', 40, '2026-11-06 10:30:00', ?1, 'IDLE', 'self', '11111111-1111-1111-1111-111111111111', 1, NULL),
        ('33333333-3333-3333-3333-333333333333', 'Other Repo Session', 'Preview 3', 90, '2026-11-06 11:00:00', ?2, 'IDLE', '', '', 0, NULL),
        ('44444444-4444-4444-4444-444444444444', '', '', 0, '0001-01-01 00:00:00', ?1, '', '', '', 0, NULL)",
        rusqlite::params![ws_uri, other_uri],
    )
    .unwrap();
    drop(conn);

    let main_brain = app_dir
        .join("brain")
        .join("11111111-1111-1111-1111-111111111111");
    fs::create_dir_all(&main_brain).unwrap();
    fs::write(main_brain.join("implementation_plan.md"), "# Plan").unwrap();
    fs::write(main_brain.join("screenshot.png"), b"\x89PNG").unwrap();

    (tmp, app_dir, ws_dir)
}

#[test]
fn test_cli_help_and_version() {
    let out = Command::new(agy_brain_bin())
        .arg("--help")
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("agy-brain"));
    assert!(stdout.contains("--target-file"));
    assert!(stdout.contains("--subagents"));

    let ver = Command::new(agy_brain_bin())
        .arg("--version")
        .output()
        .unwrap();
    assert!(ver.status.success());
    assert!(String::from_utf8_lossy(&ver.stdout).contains("agy-brain"));
}

#[test]
fn test_cli_list_local_global_and_subagents() {
    let (_tmp, app_dir, ws_dir) = setup_e2e_env();

    // Local list (excludes subagent, ghost, and other workspace)
    let out = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Main Workspace Session"));
    assert!(!stdout.contains("Subagent Session"));
    assert!(!stdout.contains("Other Repo Session"));

    // Local list with --subagents
    let out_sub = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--subagents",
            "list",
        ])
        .output()
        .unwrap();
    assert!(out_sub.status.success());
    let stdout_sub = String::from_utf8_lossy(&out_sub.stdout);
    assert!(stdout_sub.contains("Main Workspace Session"));
    assert!(stdout_sub.contains("Subagent Session"));

    // Global list (--all)
    let out_all = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "list", "--all"])
        .output()
        .unwrap();
    assert!(out_all.status.success());
    let stdout_all = String::from_utf8_lossy(&out_all.stdout);
    assert!(stdout_all.contains("Main Workspace Session"));
    assert!(stdout_all.contains("Other Repo Session"));
}

#[test]
fn test_cli_resolve_go_info_and_artifacts() {
    let (_tmp, app_dir, ws_dir) = setup_e2e_env();

    // resolve default (index 0)
    let res = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "resolve"])
        .output()
        .unwrap();
    assert!(res.status.success());
    assert!(String::from_utf8_lossy(&res.stdout).contains("11111111-1111-1111-1111-111111111111"));

    // go 0
    let go = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "go", "0"])
        .output()
        .unwrap();
    assert!(go.status.success());
    assert!(String::from_utf8_lossy(&go.stdout).contains("11111111-1111-1111-1111-111111111111"));

    // info
    let info = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "info", "11111111"])
        .output()
        .unwrap();
    assert!(info.status.success());
    let info_out = String::from_utf8_lossy(&info.stdout);
    assert!(info_out.contains("Main Workspace Session"));
    assert!(info_out.contains("implementation_plan.md"));

    // artifacts
    let art = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "artifacts",
            "11111111",
        ])
        .output()
        .unwrap();
    assert!(art.status.success());
    assert!(String::from_utf8_lossy(&art.stdout).contains("implementation_plan.md"));
}

#[test]
fn test_cli_pin_unpin_rename_prune_and_delete() {
    let (_tmp, app_dir, ws_dir) = setup_e2e_env();

    // Pin session 33333333 to ws_dir
    let pin = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "pin",
            "33333333-3333-3333-3333-333333333333",
        ])
        .output()
        .unwrap();
    assert!(pin.status.success());
    assert!(ws_dir.join(".brain").is_file());

    // Unpin from ws_dir
    let unpin = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "unpin"])
        .output()
        .unwrap();
    assert!(unpin.status.success());
    assert!(!ws_dir.join(".brain").exists());

    // Rename session 11111111
    let ren = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "rename",
            "11111111",
            "Renamed via CLI E2E",
        ])
        .output()
        .unwrap();
    assert!(ren.status.success());
    assert!(String::from_utf8_lossy(&ren.stdout).contains("Renamed via CLI E2E"));

    // Prune dry-run then real prune (removes ghost 44444444)
    let prune_dry = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "prune", "--dry-run"])
        .output()
        .unwrap();
    assert!(prune_dry.status.success());
    assert!(
        String::from_utf8_lossy(&prune_dry.stdout).contains("44444444-4444-4444-4444-444444444444")
    );

    let prune_real = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "prune"])
        .output()
        .unwrap();
    assert!(prune_real.status.success());

    // Delete interactive cancel (send "1\n" to stdin)
    let mut child = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "delete", "11111111"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"1\n").unwrap();
    let cancel_out = child.wait_with_output().unwrap();
    assert!(cancel_out.status.success());
    assert!(String::from_utf8_lossy(&cancel_out.stdout).contains("cancelada"));
    assert!(app_dir
        .join("brain")
        .join("11111111-1111-1111-1111-111111111111")
        .exists());

    // Delete with --force
    let del_force = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "delete",
            "11111111",
            "--force",
        ])
        .output()
        .unwrap();
    assert!(del_force.status.success());
    assert!(!app_dir
        .join("brain")
        .join("11111111-1111-1111-1111-111111111111")
        .exists());
}

#[test]
fn test_cli_init_and_doctor() {
    let (_tmp, app_dir, ws_dir) = setup_e2e_env();

    for shell in ["bash", "zsh", "fish", "powershell"] {
        let init = Command::new(agy_brain_bin())
            .args(["init", shell])
            .output()
            .unwrap();
        assert!(init.status.success());
        assert!(String::from_utf8_lossy(&init.stdout).contains("--target-file"));
    }

    let bad_init = Command::new(agy_brain_bin())
        .args(["init", "invalid_shell"])
        .output()
        .unwrap();
    assert!(!bad_init.status.success());

    let doc = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "doctor"])
        .output()
        .unwrap();
    assert!(doc.status.success());
    let doc_out = String::from_utf8_lossy(&doc.stdout);
    assert!(doc_out.contains("AGY-BRAIN DOCTOR"));
    assert!(doc_out.contains("Sessoes principais:"));
}
