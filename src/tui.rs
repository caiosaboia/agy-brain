use crate::backend::{self, ProviderKind};
use crate::core::{truncate_str, BrainStats, SessionSummary};
use crate::ui::{open_in_file_manager, open_in_neovim};
use chrono::{DateTime, Utc};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, stderr};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiAction {
    Navigate(PathBuf),
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteModalFocus {
    Cancel,
    Confirm,
}

pub struct AppState {
    pub all_sessions: Vec<SessionSummary>,
    pub filtered_indices: Vec<usize>,
    pub search_query: String,
    pub list_state: ListState,
    pub all_mode: bool,
    pub show_subagents: bool,
    pub workspace: PathBuf,
    pub app_dir: Option<PathBuf>,
    pub provider: ProviderKind,
    pub confirm_delete: bool,
    pub delete_modal_focus: DeleteModalFocus,
    pub modal_rect: Option<Rect>,
    pub renaming: bool,
    pub rename_input: String,
    pub show_help: bool,
}

impl AppState {
    pub fn new(
        provider: ProviderKind,
        sessions: Vec<SessionSummary>,
        workspace: PathBuf,
        app_dir: Option<PathBuf>,
        show_subagents: bool,
    ) -> Self {
        let indices: Vec<usize> = (0..sessions.len()).collect();
        let mut list_state = ListState::default();
        if !indices.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            all_sessions: sessions,
            filtered_indices: indices,
            search_query: String::new(),
            list_state,
            all_mode: false,
            show_subagents,
            workspace,
            app_dir,
            provider,
            confirm_delete: false,
            delete_modal_focus: DeleteModalFocus::Cancel,
            modal_rect: None,
            renaming: false,
            rename_input: String::new(),
            show_help: false,
        }
    }

    pub fn filter_sessions(&mut self) {
        let q = self.search_query.to_lowercase().trim().to_string();
        self.filtered_indices = self
            .all_sessions
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| {
                if q.is_empty()
                    || s.title.to_lowercase().contains(&q)
                    || s.id.to_lowercase().contains(&q)
                    || s.status.to_lowercase().contains(&q)
                    || s.mtime.contains(&q)
                {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        if self.filtered_indices.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub fn next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let next = if current + 1 >= self.filtered_indices.len() {
            0
        } else {
            current + 1
        };
        self.list_state.select(Some(next));
    }

    pub fn previous(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let prev = if current == 0 {
            self.filtered_indices.len().saturating_sub(1)
        } else {
            current - 1
        };
        self.list_state.select(Some(prev));
    }

    pub fn selected_session(&self) -> Option<&SessionSummary> {
        let selected_idx = self.list_state.selected()?;
        let orig_idx = *self.filtered_indices.get(selected_idx)?;
        self.all_sessions.get(orig_idx)
    }
}

pub fn handle_key_event(state: &mut AppState, key: event::KeyEvent) -> Option<TuiAction> {
    if key.kind != event::KeyEventKind::Press {
        return None;
    }

    if state.show_help {
        match (key.modifiers, key.code) {
            (
                KeyModifiers::NONE,
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q'),
            )
            | (KeyModifiers::SHIFT, KeyCode::Char('?'))
            | (KeyModifiers::NONE, KeyCode::F(1)) => {
                state.show_help = false;
            }
            _ => {
                state.show_help = false;
            }
        }
        return None;
    }

    if state.confirm_delete {
        match (key.modifiers, key.code) {
            (
                KeyModifiers::NONE,
                KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Char('h')
                | KeyCode::Char('l'),
            ) => {
                state.delete_modal_focus = match state.delete_modal_focus {
                    DeleteModalFocus::Cancel => DeleteModalFocus::Confirm,
                    DeleteModalFocus::Confirm => DeleteModalFocus::Cancel,
                };
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if state.delete_modal_focus == DeleteModalFocus::Confirm {
                    if let Some(session) = state.selected_session() {
                        let _ = backend::delete_session(
                            state.provider,
                            &session.id,
                            Some(&state.workspace),
                            state.app_dir.as_deref(),
                        );
                        let target_id = session.id.clone();
                        state.all_sessions.retain(|s| s.id != target_id);
                        state.filter_sessions();
                    }
                }
                state.confirm_delete = false;
                state.delete_modal_focus = DeleteModalFocus::Cancel;
            }
            (KeyModifiers::NONE, KeyCode::Char('y') | KeyCode::Char('s')) => {
                if let Some(session) = state.selected_session() {
                    let _ = backend::delete_session(
                        state.provider,
                        &session.id,
                        Some(&state.workspace),
                        state.app_dir.as_deref(),
                    );
                    let target_id = session.id.clone();
                    state.all_sessions.retain(|s| s.id != target_id);
                    state.filter_sessions();
                }
                state.confirm_delete = false;
                state.delete_modal_focus = DeleteModalFocus::Cancel;
            }
            (KeyModifiers::NONE, KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q')) => {
                state.confirm_delete = false;
                state.delete_modal_focus = DeleteModalFocus::Cancel;
            }
            _ => {}
        }
        return None;
    }

    if state.renaming {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                state.renaming = false;
                state.rename_input.clear();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let new_title = state.rename_input.trim().to_string();
                if !new_title.is_empty() {
                    if let Some(session) = state.selected_session() {
                        let target_id = session.id.clone();
                        if let Ok((_, saved_title)) = backend::rename_session(
                            state.provider,
                            &target_id,
                            &new_title,
                            Some(&state.workspace),
                            state.app_dir.as_deref(),
                        ) {
                            for s in &mut state.all_sessions {
                                if s.id == target_id {
                                    s.title = saved_title.clone();
                                    s.title_source = "title".to_string();
                                }
                                if s.parent_id == target_id {
                                    s.parent_title = Some(saved_title.clone());
                                }
                            }
                        }
                    }
                }
                state.renaming = false;
                state.rename_input.clear();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                state.rename_input.pop();
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                state.rename_input.push(c);
            }
            _ => {}
        }
        return None;
    }

    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            return Some(TuiAction::Exit);
        }
        (KeyModifiers::NONE, KeyCode::Enter) => {
            if let Some(session) = state.selected_session() {
                if let Some(brain_path) = backend::session_root_dir(
                    state.provider,
                    &session.id,
                    Some(&state.workspace),
                    state.app_dir.as_deref(),
                ) {
                    return Some(TuiAction::Navigate(brain_path));
                }
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
            if let Some(session) = state.selected_session() {
                if let Some(brain_path) = backend::session_root_dir(
                    state.provider,
                    &session.id,
                    Some(&state.workspace),
                    state.app_dir.as_deref(),
                ) {
                    open_in_file_manager(&brain_path);
                }
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
            if let Some(session) = state.selected_session() {
                let cur_id = session.id.clone();
                let was_pinned = session.pinned;
                if was_pinned {
                    let _ = backend::remove_pin(
                        state.provider,
                        Some(&state.workspace),
                        state.app_dir.as_deref(),
                    );
                    for s in &mut state.all_sessions {
                        if s.id == cur_id {
                            s.pinned = false;
                        }
                    }
                } else {
                    let _ = backend::set_pin(
                        state.provider,
                        &cur_id,
                        Some(&state.workspace),
                        state.app_dir.as_deref(),
                    );
                    for s in &mut state.all_sessions {
                        s.pinned = s.id == cur_id;
                    }
                }
                state
                    .all_sessions
                    .sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| b.steps.cmp(&a.steps)));
                state.filter_sessions();
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('r')) => {
            if let Some(session) = state.selected_session() {
                state.rename_input = session.title.clone();
                state.renaming = true;
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            if state.selected_session().is_some() {
                state.confirm_delete = true;
                state.delete_modal_focus = DeleteModalFocus::Cancel;
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
            state.all_mode = !state.all_mode;
            state.all_sessions = backend::fetch_sessions_ext(
                state.provider,
                Some(&state.workspace),
                state.all_mode,
                state.show_subagents,
                state.app_dir.as_deref(),
            );
            state.filter_sessions();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
            state.show_subagents = !state.show_subagents;
            state.all_sessions = backend::fetch_sessions_ext(
                state.provider,
                Some(&state.workspace),
                state.all_mode,
                state.show_subagents,
                state.app_dir.as_deref(),
            );
            state.filter_sessions();
        }
        (KeyModifiers::NONE, KeyCode::Up) => {
            state.previous();
        }
        (KeyModifiers::NONE, KeyCode::Down) => {
            state.next();
        }
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('?'))
        | (KeyModifiers::NONE, KeyCode::F(1)) => {
            state.show_help = true;
        }
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            state.search_query.pop();
            state.filter_sessions();
        }
        (KeyModifiers::NONE, KeyCode::Char(c)) => {
            state.search_query.push(c);
            state.filter_sessions();
        }
        _ => {}
    }

    None
}

pub fn run_interactive_tui(
    provider: ProviderKind,
    sessions: Vec<SessionSummary>,
    workspace: &Path,
    app_dir: Option<&Path>,
    show_subagents: bool,
) -> io::Result<TuiAction> {
    enable_raw_mode()?;
    let mut stderr_handle = stderr();
    execute!(stderr_handle, EnterAlternateScreen, EnableMouseCapture)?;
    let term_backend = CrosstermBackend::new(stderr_handle);
    let mut terminal = Terminal::new(term_backend)?;

    let mut state = AppState::new(
        provider,
        sessions,
        workspace.to_path_buf(),
        app_dir.map(|p| p.to_path_buf()),
        show_subagents,
    );

    let res = main_loop(&mut terminal, &mut state);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn main_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &mut AppState,
) -> io::Result<TuiAction> {
    loop {
        terminal.draw(|f| render_ui(f, state))?;

        let event = event::read()?;

        if let Event::Mouse(mouse) = &event {
            if state.show_help {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    state.show_help = false;
                }
                continue;
            }
            if state.confirm_delete {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    if let Some(rect) = state.modal_rect {
                        if mouse.column >= rect.x
                            && mouse.column < rect.x + rect.width
                            && mouse.row >= rect.y
                            && mouse.row < rect.y + rect.height
                        {
                            let button_row_start = rect.y + rect.height.saturating_sub(5);
                            if mouse.row >= button_row_start {
                                let mid_x = rect.x + rect.width / 2;
                                if mouse.column < mid_x {
                                    state.confirm_delete = false;
                                    state.delete_modal_focus = DeleteModalFocus::Cancel;
                                } else {
                                    if let Some(session) = state.selected_session() {
                                        let _ = backend::delete_session(
                                            state.provider,
                                            &session.id,
                                            Some(&state.workspace),
                                            state.app_dir.as_deref(),
                                        );
                                        let target_id = session.id.clone();
                                        state.all_sessions.retain(|s| s.id != target_id);
                                        state.filter_sessions();
                                    }
                                    state.confirm_delete = false;
                                    state.delete_modal_focus = DeleteModalFocus::Cancel;
                                }
                            }
                        } else {
                            state.confirm_delete = false;
                            state.delete_modal_focus = DeleteModalFocus::Cancel;
                        }
                    }
                }
                continue;
            }
        }

        if let Event::Key(key) = event {
            if key.kind == event::KeyEventKind::Press
                && !state.confirm_delete
                && !state.renaming
                && matches!(
                    (key.modifiers, key.code),
                    (
                        KeyModifiers::CONTROL,
                        KeyCode::Char('e') | KeyCode::Char('v') | KeyCode::Char('n')
                    ) | (
                        KeyModifiers::ALT,
                        KeyCode::Char('v') | KeyCode::Char('n') | KeyCode::Char('e')
                    ) | (KeyModifiers::NONE, KeyCode::F(2))
                )
            {
                if let Some(session) = state.selected_session() {
                    if let Some(brain_path) = backend::session_root_dir(
                        state.provider,
                        &session.id,
                        Some(&state.workspace),
                        state.app_dir.as_deref(),
                    ) {
                        let _ = disable_raw_mode();
                        let _ =
                            execute!(std::io::stderr(), LeaveAlternateScreen, DisableMouseCapture);
                        let _ = terminal.show_cursor();

                        let _ = open_in_neovim(&brain_path);

                        let _ = enable_raw_mode();
                        let _ =
                            execute!(std::io::stderr(), EnterAlternateScreen, EnableMouseCapture);
                        let _ = terminal.hide_cursor();
                        let _ = terminal.clear();
                    }
                }
                continue;
            }

            if let Some(action) = handle_key_event(state, key) {
                return Ok(action);
            }
        }
    }
}

pub fn render_ui(f: &mut Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    // 1. Search Header
    let mode_tag = if state.all_mode {
        " [MODO: TODOS OS PROJETOS]"
    } else {
        " [PROJETO ATUAL]"
    };
    let sub_tag = if state.show_subagents {
        " [SUBAGENTES: VISIVEIS] "
    } else {
        " "
    };
    let search_title = format!(" Antigravity Brain Navigator{}{} ", mode_tag, sub_tag);
    let search_bar = Paragraph::new(Line::from(vec![
        Span::styled(
            " Buscar: ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&state.search_query, Style::default().fg(Color::White)),
        Span::styled(
            "_",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(search_title)
            .style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(search_bar, chunks[0]);

    // 2. Middle area: Split List (45%) & Preview (55%)
    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    // Left List
    let items: Vec<ListItem> = state
        .filtered_indices
        .iter()
        .map(|&idx| {
            let s = &state.all_sessions[idx];
            let (tag, tag_color) = if s.pinned {
                ("[FIXADO] ".to_string(), Color::Green)
            } else if s.is_subagent {
                ("[SUB]    ".to_string(), Color::Magenta)
            } else if s.subagent_count > 0 {
                (
                    format!("{:<9}", format!("[+{}sub]", s.subagent_count)),
                    Color::Cyan,
                )
            } else {
                ("         ".to_string(), Color::Green)
            };
            let steps = format!("{:>5} p.", s.steps);
            let title = truncate_str(&s.title, 28);

            let line = Line::from(vec![
                Span::styled(
                    tag,
                    Style::default().fg(tag_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{:<28} ", title), Style::default().fg(Color::White)),
                Span::styled(steps, Style::default().fg(Color::Yellow)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list_title = format!(
        " Sessoes ({}/{}) ",
        state.filtered_indices.len(),
        state.all_sessions.len()
    );
    let list_widget = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(list_title)
                .style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(30, 45, 65))
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list_widget, middle_chunks[0], &mut state.list_state);

    // Right Preview Card
    let selected_session = state.selected_session();
    let stats = selected_session
        .and_then(|s| backend::get_brain_stats(state.provider, &s.id, state.app_dir.as_deref()));

    let preview_widget = render_tui_preview(selected_session, stats.as_ref());
    f.render_widget(preview_widget, middle_chunks[1]);

    // 3. Bottom Shortcuts
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " [Enter] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Ir "),
        Span::styled(
            " [?] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Ajuda "),
        Span::styled(
            " [Ctrl+E] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(80, 220, 140))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Nvim "),
        Span::styled(
            " [Ctrl+R] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(120, 180, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Renomear "),
        Span::styled(
            " [Ctrl+S] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(200, 140, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Subs "),
        Span::styled(
            " [Ctrl+P] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Fixar "),
        Span::styled(
            " [Ctrl+D] ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Apagar "),
        Span::styled(
            " [Ctrl+A] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Todos "),
        Span::styled(
            " [Esc] ",
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
        Span::raw(" Sair"),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Atalhos "));
    f.render_widget(footer, chunks[2]);

    if state.renaming {
        let area = centered_rect(65, 25, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                " RENOMEAR SESSAO ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));

        let selected_id = state
            .selected_session()
            .map(|s| s.id.as_str())
            .unwrap_or("");

        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
                Span::styled(selected_id, Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Novo Titulo: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    &state.rename_input,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "_",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Enter] ", Style::default().fg(Color::Green)),
                Span::raw("Salvar no SQLite | "),
                Span::styled("[Esc] ", Style::default().fg(Color::Red)),
                Span::raw("Cancelar"),
            ]),
        ];

        let p = Paragraph::new(text)
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
    } else if state.confirm_delete {
        let area = centered_rect(65, 30, f.area());
        state.modal_rect = Some(area);
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(Span::styled(
                " CONFIRMAR EXCLUSAO DEFINITIVA ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));

        let selected_title = state
            .selected_session()
            .map(|s| s.title.as_str())
            .unwrap_or("(sem titulo)");
        let selected_id = state
            .selected_session()
            .map(|s| s.id.as_str())
            .unwrap_or("");

        let (cancel_btn, confirm_btn) = match state.delete_modal_focus {
            DeleteModalFocus::Cancel => (
                Span::styled(
                    "  >>> [ CANCELAR ] <<<  ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "      [ APAGAR SESSAO ]      ",
                    Style::default()
                        .fg(Color::DarkGray)
                        .bg(Color::Rgb(40, 20, 20)),
                ),
            ),
            DeleteModalFocus::Confirm => (
                Span::styled(
                    "      [ CANCELAR ]      ",
                    Style::default()
                        .fg(Color::DarkGray)
                        .bg(Color::Rgb(20, 40, 20)),
                ),
                Span::styled(
                    "  >>> [ APAGAR SESSAO ] <<<  ",
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
            ),
        };

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Esta acao ira apagar a pasta do cerebro e o registro permanentemente!",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Sessao: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    selected_title,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("ID:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(selected_id, Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
            Line::from(vec![cancel_btn, Span::raw("   "), confirm_btn]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Setas [<- / ->] ou [Tab] ",
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw("Mudar opcao | "),
                Span::styled("[Enter] ", Style::default().fg(Color::Cyan)),
                Span::raw("Executar | "),
                Span::styled("[Esc] ", Style::default().fg(Color::DarkGray)),
                Span::raw("Fechar | Clique com o mouse no botao"),
            ]),
            Line::from(""),
        ];

        let p = Paragraph::new(text)
            .block(block)
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
    } else if state.show_help {
        render_help_modal(f, state);
    } else {
        state.modal_rect = None;
    }
}

fn render_help_modal(f: &mut Frame, state: &mut AppState) {
    let area = centered_rect(72, 76, f.area());
    state.modal_rect = Some(area);
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Span::styled(
            " AJUDA - COMANDOS E ATALHOS DO BRAIN ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    let rows = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " [Setas Cima / Baixo] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Navegar pela lista de sessoes / historico"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Enter]              ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Ir para a pasta do cerebro selecionado (Set-Location / cd)"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Ctrl+E] ou [F2]     ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(80, 220, 140))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Abrir sessao e artefatos diretamente no Neovim"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Ctrl+O]             ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(80, 220, 140))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Abrir diretorio no explorador de arquivos (Explorer / Finder)"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Ctrl+R]             ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(120, 180, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Renomear o titulo customizado da sessao selecionada"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Ctrl+P]             ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Fixar / desafixar sessao para o workspace atual (Pin/Unpin)"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Ctrl+S]             ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(200, 140, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Alternar exibicao de subagentes (conversas filhas)"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Ctrl+A]             ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Alternar entre workspace atual e todos os projetos"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Ctrl+D]             ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Excluir sessao selecionada (solicita confirmacao)"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Texto digitado]     ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Filtro instantaneo por titulo, id, data ou status"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Backspace]          ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Apagar caractere do filtro de busca"),
        ]),
        Line::from(vec![
            Span::styled(
                " [?] ou [F1]          ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Abrir / fechar esta janela flutuante de ajuda"),
        ]),
        Line::from(vec![
            Span::styled(
                " [Esc]                ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  Fechar janela / sair do navegador interativo"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Pressione ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[?] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(", ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[Esc] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("ou ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[Enter] ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "para fechar esta ajuda",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let p = Paragraph::new(rows)
        .block(block)
        .alignment(ratatui::layout::Alignment::Left)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_tui_preview<'a>(
    session: Option<&'a SessionSummary>,
    stats: Option<&'a BrainStats>,
) -> Paragraph<'a> {
    let mut text_lines = Vec::new();

    if let Some(s) = session {
        let pin_badge = if s.pinned { " [FIXADO]" } else { "" };
        let sub_badge = if s.is_subagent { " [SUBAGENTE]" } else { "" };
        text_lines.push(Line::from(vec![
            Span::styled(
                "SESSAO: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &s.title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                pin_badge,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                sub_badge,
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        text_lines.push(Line::from(vec![
            Span::styled("  ID:         ", Style::default().fg(Color::DarkGray)),
            Span::styled(&s.id, Style::default().fg(Color::Cyan)),
        ]));
        text_lines.push(Line::from(vec![
            Span::styled("  Passos:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} interacoes", s.steps),
                Style::default().fg(Color::Yellow),
            ),
        ]));
        if s.is_subagent {
            if !s.agent_name.is_empty() {
                text_lines.push(Line::from(vec![
                    Span::styled("  Agente:     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&s.agent_name, Style::default().fg(Color::Magenta)),
                ]));
            }
            if !s.parent_id.is_empty() {
                let p_title = s.parent_title.as_deref().unwrap_or("(sessao pai)");
                text_lines.push(Line::from(vec![
                    Span::styled("  Sessao Pai: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("{} ({})", truncate_str(&s.parent_id, 11), p_title),
                        Style::default().fg(Color::Cyan),
                    ),
                ]));
            }
        } else if s.subagent_count > 0 {
            text_lines.push(Line::from(vec![
                Span::styled("  Subagentes: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} sessoes filhas", s.subagent_count),
                    Style::default().fg(Color::Cyan),
                ),
            ]));
        }
        text_lines.push(Line::from(vec![
            Span::styled("  Status:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(&s.status),
        ]));
        text_lines.push(Line::from(vec![
            Span::styled("  Modificado: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&s.mtime),
        ]));
        if !s.primary_workspace.is_empty() {
            text_lines.push(Line::from(vec![
                Span::styled("  Workspace:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(&s.primary_workspace),
            ]));
        }
    } else {
        text_lines.push(Line::from(Span::styled(
            "Nenhuma sessao registrada para este diretorio.",
            Style::default().fg(Color::Yellow),
        )));
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(vec![
            Span::styled("Dica: Pressione ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[Ctrl+A]",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " se desejar visualizar sessoes de todos os projetos.",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    text_lines.push(Line::from(""));

    if let Some(st) = stats {
        if st.exists {
            text_lines.push(Line::from(Span::styled(
                "ESTATISTICAS DO CEREBRO",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            text_lines.push(Line::from(vec![
                Span::styled("  Artefatos MD:    ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} arquivos", st.md_count),
                    Style::default().fg(Color::Green),
                ),
            ]));
            text_lines.push(Line::from(vec![
                Span::styled("  Imagens PNG:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} capturas", st.png_count),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
            text_lines.push(Line::from(vec![
                Span::styled("  Scripts Scratch: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{}", st.scratch_count)),
            ]));
            text_lines.push(Line::from(vec![
                Span::styled("  Caminho:         ", Style::default().fg(Color::DarkGray)),
                Span::raw(st.path.to_string_lossy()),
            ]));

            text_lines.push(Line::from(""));
            if !st.recent_artifacts.is_empty() {
                text_lines.push(Line::from(Span::styled(
                    "ULTIMOS ARTEFATOS:",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                for art in &st.recent_artifacts {
                    let kb = format!("{:.1} KB", art.size as f64 / 1024.0);
                    let dt = if let Some(utc) = DateTime::<Utc>::from_timestamp(art.mtime as i64, 0)
                    {
                        utc.format("%d/%m %H:%M").to_string()
                    } else {
                        "-".to_string()
                    };
                    text_lines.push(Line::from(vec![
                        Span::styled("  * ", Style::default().fg(Color::Green)),
                        Span::styled(
                            format!("{:<32}", art.name),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            format!(" ({} - {})", kb, dt),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }
            }
        }
    }

    Paragraph::new(text_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Detalhes do Cerebro ")
                .style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false })
}
