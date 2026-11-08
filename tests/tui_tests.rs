#![allow(dead_code)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use rusqlite::Connection;
use std::fs;
use tempfile::tempdir;

#[path = "../src/core.rs"]
mod core;

#[path = "../src/ui.rs"]
mod ui;

#[path = "../src/tui.rs"]
mod tui;

use core::fetch_sessions_ext;
use tui::{handle_key_event, render_ui, AppState, DeleteModalFocus, TuiAction};

fn buffer_to_string(terminal: &Terminal<TestBackend>) -> String {
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn setup_test_db() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let tmp = tempdir().unwrap();
    let app_dir = tmp.path().join("agy_home");
    let ws_dir = tmp.path().join("project_a");
    let other_ws = tmp.path().join("project_b");
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

    let ws_uri = format!("[\"{}\"]", ws_dir.to_string_lossy().replace('\\', "/"));
    let other_uri = format!("[\"{}\"]", other_ws.to_string_lossy().replace('\\', "/"));

    conn.execute(
        "INSERT INTO conversation_summaries VALUES
        ('alpha-1111-1111-1111-111111111111', 'Refactor Auth Module', 'Preview A', 120, '2026-11-06 10:00:00', ?1, 'IDLE', '', '', 0, NULL),
        ('beta-2222-2222-2222-222222222222', 'Optimize SQLite Queries', 'Preview B', 80, '2026-11-06 11:00:00', ?1, 'IDLE', '', '', 0, NULL),
        ('sub-3333-3333-3333-333333333333', 'Subagent Worker', '', 30, '2026-11-06 11:30:00', ?1, 'IDLE', 'self', 'alpha-1111-1111-1111-111111111111', 1, NULL),
        ('global-4444-4444-4444-444444444444', 'Other Workspace Session', 'Preview C', 200, '2026-11-06 12:00:00', ?2, 'IDLE', '', '', 0, NULL)",
        rusqlite::params![ws_uri, other_uri],
    )
    .unwrap();
    drop(conn);

    let alpha_brain = app_dir
        .join("brain")
        .join("alpha-1111-1111-1111-111111111111");
    fs::create_dir_all(&alpha_brain).unwrap();
    fs::write(alpha_brain.join("walkthrough.md"), "# Walkthrough").unwrap();

    (tmp, app_dir, ws_dir, db_path)
}

#[test]
fn test_tui_initial_render_and_split_layout() {
    let (_tmp, app_dir, ws_dir, db_path) = setup_test_db();
    let sessions = fetch_sessions_ext(Some(&ws_dir), false, false, Some(&app_dir), Some(&db_path));
    assert_eq!(sessions.len(), 2);

    let mut state = AppState::new(sessions, ws_dir, Some(app_dir), false);
    let backend = TestBackend::new(120, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| render_ui(f, &mut state)).unwrap();
    let screen = buffer_to_string(&terminal);

    assert!(screen.contains("Antigravity Brain Navigator [PROJETO ATUAL]"));
    assert!(screen.contains("Sessoes (2/2)"));
    assert!(screen.contains("Refactor Auth Module"));
    assert!(screen.contains("[+1sub]"));
    assert!(screen.contains("Optimize SQLite Queries"));
    assert!(screen.contains("Detalhes do Cerebro"));
    assert!(screen.contains("walkthrough.md"));
}

#[test]
fn test_tui_search_filtering_and_navigation() {
    let (_tmp, app_dir, ws_dir, db_path) = setup_test_db();
    let sessions = fetch_sessions_ext(Some(&ws_dir), false, false, Some(&app_dir), Some(&db_path));
    let mut state = AppState::new(sessions, ws_dir, Some(app_dir.clone()), false);

    // Initially first session is selected
    assert_eq!(
        state.selected_session().unwrap().id,
        "alpha-1111-1111-1111-111111111111"
    );

    // Press Down -> selects second session
    handle_key_event(&mut state, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(
        state.selected_session().unwrap().id,
        "beta-2222-2222-2222-222222222222"
    );

    // Press Up -> back to first session
    handle_key_event(&mut state, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(
        state.selected_session().unwrap().id,
        "alpha-1111-1111-1111-111111111111"
    );

    // Type "sqlite" in search box -> filters down to 1 session
    for ch in "sqlite".chars() {
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
        );
    }
    assert_eq!(state.filtered_indices.len(), 1);
    assert_eq!(
        state.selected_session().unwrap().id,
        "beta-2222-2222-2222-222222222222"
    );

    // Press Enter -> returns TuiAction::Navigate
    let action = handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );
    assert_eq!(
        action,
        Some(TuiAction::Navigate(
            app_dir
                .join("brain")
                .join("beta-2222-2222-2222-222222222222")
        ))
    );
}

#[test]
fn test_tui_pin_and_unpin_toggle() {
    let (_tmp, app_dir, ws_dir, db_path) = setup_test_db();
    let sessions = fetch_sessions_ext(Some(&ws_dir), false, false, Some(&app_dir), Some(&db_path));
    let mut state = AppState::new(sessions, ws_dir.clone(), Some(app_dir), false);

    assert!(!state.selected_session().unwrap().pinned);
    assert!(!ws_dir.join(".brain").exists());

    // First Ctrl+P -> pins session
    handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
    );
    assert!(state.selected_session().unwrap().pinned);
    assert!(ws_dir.join(".brain").is_file());

    // Second Ctrl+P on same pinned session -> unpins immediately
    handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
    );
    assert!(!state.selected_session().unwrap().pinned);
    assert!(!ws_dir.join(".brain").exists());
}

#[test]
fn test_tui_subagents_and_all_mode_toggles() {
    let (_tmp, app_dir, ws_dir, db_path) = setup_test_db();
    let sessions = fetch_sessions_ext(Some(&ws_dir), false, false, Some(&app_dir), Some(&db_path));
    let mut state = AppState::new(sessions, ws_dir, Some(app_dir), false);

    assert_eq!(state.all_sessions.len(), 2);

    // Ctrl+S -> shows subagents (now 3 sessions in current workspace)
    handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );
    assert!(state.show_subagents);
    assert_eq!(state.all_sessions.len(), 3);

    // Ctrl+A -> shows all workspaces + subagents (now 4 sessions total)
    handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
    );
    assert!(state.all_mode);
    assert_eq!(state.all_sessions.len(), 4);
}

#[test]
fn test_tui_rename_and_delete_modals() {
    let (_tmp, app_dir, ws_dir, db_path) = setup_test_db();
    let sessions = fetch_sessions_ext(Some(&ws_dir), false, false, Some(&app_dir), Some(&db_path));
    let mut state = AppState::new(sessions, ws_dir, Some(app_dir), false);
    let backend = TestBackend::new(120, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    // Open rename modal with Ctrl+R
    handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
    );
    assert!(state.renaming);

    terminal.draw(|f| render_ui(f, &mut state)).unwrap();
    assert!(buffer_to_string(&terminal).contains("RENOMEAR SESSAO"));

    // Clear existing input and type new title
    state.rename_input.clear();
    for ch in "Novo Titulo TUI".chars() {
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
        );
    }
    handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );
    assert!(!state.renaming);
    assert_eq!(state.selected_session().unwrap().title, "Novo Titulo TUI");

    // Open delete modal with Ctrl+D
    handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    );
    assert!(state.confirm_delete);
    assert_eq!(state.delete_modal_focus, DeleteModalFocus::Cancel);

    terminal.draw(|f| render_ui(f, &mut state)).unwrap();
    assert!(buffer_to_string(&terminal).contains("CONFIRMAR EXCLUSAO DEFINITIVA"));

    // Switch focus to Confirm using Tab and press Enter
    handle_key_event(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(state.delete_modal_focus, DeleteModalFocus::Confirm);
    handle_key_event(
        &mut state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );

    assert!(!state.confirm_delete);
    assert_eq!(state.all_sessions.len(), 1);
    assert_eq!(
        state.selected_session().unwrap().id,
        "beta-2222-2222-2222-222222222222"
    );
}
