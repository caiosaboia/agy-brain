#![allow(dead_code)]
use std::path::PathBuf;

#[path = "../src/core.rs"]
mod core;

#[path = "../src/ui.rs"]
mod ui;

#[test]
fn test_open_in_neovim_non_existent_path() {
    let dummy_path = PathBuf::from("non/existent/path/for/session/neovim");
    assert!(!ui::open_in_neovim(&dummy_path));
}

#[test]
fn test_open_in_editor_non_existent_path() {
    let dummy_path = PathBuf::from("non/existent/path/for/session/editor");
    assert!(!ui::open_in_editor(&dummy_path));
}
