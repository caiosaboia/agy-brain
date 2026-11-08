#![allow(dead_code)]

#[path = "../src/shells.rs"]
mod shells;

use shells::{get_shell_integration, install_shell_integration_in_home};
use tempfile::tempdir;

#[test]
fn test_get_shell_integration() {
    let bash = get_shell_integration("bash").unwrap();
    assert!(bash.contains("agy-brain() {"));
    assert!(bash.contains("cd-brain"));
    assert!(bash.contains("--target-file"));

    let zsh = get_shell_integration("zsh").unwrap();
    assert!(zsh.contains("agy-brain() {"));
    assert!(zsh.contains("--target-file"));

    let fish = get_shell_integration("fish").unwrap();
    assert!(fish.contains("function agy-brain"));
    assert!(fish.contains("--target-file"));

    let pwsh = get_shell_integration("powershell").unwrap();
    assert!(pwsh.contains("function agy-brain {"));
    assert!(pwsh.contains("--target-file"));

    assert!(get_shell_integration("unknown_shell").is_none());
}

#[test]
fn test_install_shell_integration_idempotence() {
    let tmp = tempdir().unwrap();
    let mock_home = tmp.path().join("home");
    std::fs::create_dir_all(&mock_home).unwrap();

    for shell in ["bash", "zsh", "fish", "powershell"] {
        // First install in isolated mock_home
        let (ok, msg) = install_shell_integration_in_home(Some(shell), Some(&mock_home));
        assert!(ok, "Failed installing {shell}: {msg}");
        assert!(
            msg.contains("instalada com sucesso"),
            "Expected fresh install message for {shell}, got: {msg}"
        );

        // Second install in isolated mock_home - must be idempotent
        let (ok2, msg2) = install_shell_integration_in_home(Some(shell), Some(&mock_home));
        assert!(ok2);
        assert!(
            msg2.contains("ja esta instalada"),
            "Expected idempotent message for {shell}, got: {msg2}"
        );
    }
}
