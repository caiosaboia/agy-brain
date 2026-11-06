#[path = "../src/shells.rs"]
mod shells;

use shells::{get_shell_integration, install_shell_integration};
use tempfile::tempdir;

#[test]
fn test_get_shell_integration() {
    let bash = get_shell_integration("bash").unwrap();
    assert!(bash.contains("agy-brain() {"));
    assert!(bash.contains("cd-brain"));

    let zsh = get_shell_integration("zsh").unwrap();
    assert!(zsh.contains("agy-brain() {"));

    let fish = get_shell_integration("fish").unwrap();
    assert!(fish.contains("function agy-brain"));

    let pwsh = get_shell_integration("powershell").unwrap();
    assert!(pwsh.contains("function agy-brain {"));

    assert!(get_shell_integration("unknown_shell").is_none());
}

#[test]
fn test_install_shell_integration_idempotence() {
    let tmp = tempdir().unwrap();
    let mock_home = tmp.path().join("home");
    std::fs::create_dir_all(&mock_home).unwrap();

    // Test install bash
    let (ok, msg) = install_shell_integration(Some("bash"));
    assert!(ok);
    assert!(msg.contains("sucesso") || msg.contains("instalada"));

    // Run again - should be idempotent
    let (ok2, msg2) = install_shell_integration(Some("bash"));
    assert!(ok2);
    assert!(msg2.contains("ja esta instalada"));
}
