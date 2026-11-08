use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn agy_brain_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_agy-brain"))
}

fn encode_project_dir(path: &Path) -> String {
    path.to_string_lossy().replace(['\\', '/', ':'], "-")
}

fn ws_str(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn user_entry(cwd: &str, text: &str, ts: &str) -> String {
    format!(
        r#"{{"type":"user","message":{{"role":"user","content":"{text}"}},"cwd":"{cwd}","timestamp":"{ts}","isSidechain":false}}"#
    )
}

fn assistant_entry(cwd: &str, ts: &str) -> String {
    format!(
        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"ok"}}]}},"cwd":"{cwd}","timestamp":"{ts}","isSidechain":false}}"#
    )
}

fn summary_entry(title: &str) -> String {
    format!(r#"{{"type":"summary","summary":"{title}","leafUuid":"x"}}"#)
}

fn write_session_file(projects_root: &Path, ws: &Path, id: &str, lines: &[String]) {
    let project_dir = projects_root.join(encode_project_dir(ws));
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(
        project_dir.join(format!("{id}.jsonl")),
        lines.join("\n") + "\n",
    )
    .unwrap();
}

fn setup_e2e_env() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("claude_home");
    let ws_dir = tmp.path().join("workspace_main");
    let other_ws = tmp.path().join("workspace_other");
    fs::create_dir_all(app_dir.join("projects")).unwrap();
    fs::create_dir_all(&ws_dir).unwrap();
    fs::create_dir_all(&other_ws).unwrap();

    let projects_root = app_dir.join("projects");
    let main_ws_str = ws_str(&ws_dir);
    let other_ws_str = ws_str(&other_ws);

    write_session_file(
        &projects_root,
        &ws_dir,
        "11111111-1111-1111-1111-111111111111",
        &[
            summary_entry("Main Workspace Session"),
            user_entry(
                &main_ws_str,
                "please refactor the auth module",
                "2026-11-06T10:00:00.000Z",
            ),
            assistant_entry(&main_ws_str, "2026-11-06T10:05:00.000Z"),
        ],
    );
    write_session_file(
        &projects_root,
        &ws_dir,
        "44444444-4444-4444-4444-444444444444",
        &[],
    );
    write_session_file(
        &projects_root,
        &other_ws,
        "33333333-3333-3333-3333-333333333333",
        &[
            summary_entry("Other Repo Session"),
            user_entry(&other_ws_str, "unrelated work", "2026-11-06T11:00:00.000Z"),
        ],
    );

    (tmp, app_dir, ws_dir, other_ws)
}

#[test]
fn test_cli_claude_code_list_local_global_and_subagents_noop() {
    let (_tmp, app_dir, ws_dir, _other_ws) = setup_e2e_env();

    let out = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "list",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Main Workspace Session"));
    assert!(!stdout.contains("Other Repo Session"));

    // --subagents is accepted but a no-op for this provider.
    let out_sub = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "--subagents",
            "list",
        ])
        .output()
        .unwrap();
    assert!(out_sub.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out_sub.stdout),
        String::from_utf8_lossy(&out.stdout)
    );

    let out_all = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "list",
            "--all",
        ])
        .output()
        .unwrap();
    assert!(out_all.status.success());
    let stdout_all = String::from_utf8_lossy(&out_all.stdout);
    assert!(stdout_all.contains("Main Workspace Session"));
    assert!(stdout_all.contains("Other Repo Session"));
}

#[test]
fn test_cli_claude_code_resolve_go_info_and_artifacts() {
    let (_tmp, app_dir, ws_dir, _other_ws) = setup_e2e_env();

    let res = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "resolve",
        ])
        .output()
        .unwrap();
    assert!(res.status.success());
    let resolved_path = String::from_utf8_lossy(&res.stdout).trim().to_string();
    assert_eq!(PathBuf::from(&resolved_path), ws_dir);

    let go = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "go",
            "0",
        ])
        .output()
        .unwrap();
    assert!(go.status.success());
    assert_eq!(
        PathBuf::from(String::from_utf8_lossy(&go.stdout).trim()),
        ws_dir
    );

    let info = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "info",
            "11111111",
        ])
        .output()
        .unwrap();
    assert!(info.status.success());
    let info_out = String::from_utf8_lossy(&info.stdout);
    assert!(info_out.contains("Main Workspace Session"));

    let art = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "artifacts",
            "11111111",
        ])
        .output()
        .unwrap();
    assert!(art.status.success());
    let art_out = String::from_utf8_lossy(&art.stdout);
    assert!(art_out.contains("11111111-1111-1111-1111-111111111111.jsonl"));
}

#[test]
fn test_cli_claude_code_pin_unpin_rename_prune_and_delete() {
    let (_tmp, app_dir, ws_dir, _other_ws) = setup_e2e_env();

    let pin = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "pin",
            "33333333-3333-3333-3333-333333333333",
        ])
        .output()
        .unwrap();
    assert!(pin.status.success());
    assert!(ws_dir.join(".claude-brain").is_file());
    assert!(!ws_dir.join(".brain").exists());

    let unpin = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "unpin",
        ])
        .output()
        .unwrap();
    assert!(unpin.status.success());
    assert!(!ws_dir.join(".claude-brain").exists());

    let ren = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "rename",
            "11111111",
            "Renamed via CLI E2E",
        ])
        .output()
        .unwrap();
    assert!(ren.status.success());
    assert!(String::from_utf8_lossy(&ren.stdout).contains("Renamed via CLI E2E"));
    assert!(app_dir.join("agy_titles.json").is_file());

    let prune_dry = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "prune",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(prune_dry.status.success());
    assert!(
        String::from_utf8_lossy(&prune_dry.stdout).contains("44444444-4444-4444-4444-444444444444")
    );

    let prune_real = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "prune",
        ])
        .output()
        .unwrap();
    assert!(prune_real.status.success());

    let mut child = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "delete",
            "11111111",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"1\n").unwrap();
    let cancel_out = child.wait_with_output().unwrap();
    assert!(cancel_out.status.success());
    assert!(String::from_utf8_lossy(&cancel_out.stdout).contains("cancelada"));
    assert!(app_dir
        .join("projects")
        .join(encode_project_dir(&ws_dir))
        .join("11111111-1111-1111-1111-111111111111.jsonl")
        .exists());

    let del_force = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "delete",
            "11111111",
            "--force",
        ])
        .output()
        .unwrap();
    assert!(del_force.status.success());
    assert!(String::from_utf8_lossy(&del_force.stdout).contains("Transcript removido"));
    assert!(!app_dir
        .join("projects")
        .join(encode_project_dir(&ws_dir))
        .join("11111111-1111-1111-1111-111111111111.jsonl")
        .exists());
}

#[test]
fn test_cli_claude_code_doctor() {
    let (_tmp, app_dir, ws_dir, _other_ws) = setup_e2e_env();

    let doc = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args([
            "--app-dir",
            app_dir.to_str().unwrap(),
            "--provider",
            "claude-code",
            "doctor",
        ])
        .output()
        .unwrap();
    assert!(doc.status.success());
    let doc_out = String::from_utf8_lossy(&doc.stdout);
    assert!(doc_out.contains("AGY-BRAIN DOCTOR"));
    assert!(doc_out.contains("Provider selecionado: claude-code"));
    assert!(doc_out.contains("Pasta de projetos:"));
}

#[test]
fn test_cli_provider_auto_detection_without_explicit_flag() {
    let (_tmp, app_dir, ws_dir, _other_ws) = setup_e2e_env();

    // No --provider flag: the shape of --app-dir (a `projects/` subfolder,
    // no conversation_summaries.db) must auto-select Claude Code.
    let out = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "list"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Main Workspace Session"));

    let doc = Command::new(agy_brain_bin())
        .current_dir(&ws_dir)
        .args(["--app-dir", app_dir.to_str().unwrap(), "doctor"])
        .output()
        .unwrap();
    assert!(doc.status.success());
    assert!(String::from_utf8_lossy(&doc.stdout).contains("Provider selecionado: claude-code"));
}
