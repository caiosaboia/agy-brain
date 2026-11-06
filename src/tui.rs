use crate::core::{get_brain_root, get_brain_stats, set_pin, BrainStats, SessionSummary};
use crate::ui::{open_in_editor, open_in_file_manager};
use chrono::{DateTime, Utc};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, stderr};
use std::path::{Path, PathBuf};

pub enum TuiAction {
    Navigate(PathBuf),
    Exit,
}

struct AppState {
    all_sessions: Vec<SessionSummary>,
    filtered_indices: Vec<usize>,
    search_query: String,
    list_state: ListState,
    all_mode: bool,
    workspace: PathBuf,
    app_dir: Option<PathBuf>,
}

impl AppState {
    fn new(sessions: Vec<SessionSummary>, workspace: PathBuf, app_dir: Option<PathBuf>) -> Self {
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
            workspace,
            app_dir,
        }
    }

    fn filter_sessions(&mut self) {
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

    fn next(&mut self) {
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

    fn previous(&mut self) {
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

    fn selected_session(&self) -> Option<&SessionSummary> {
        let selected_idx = self.list_state.selected()?;
        let orig_idx = *self.filtered_indices.get(selected_idx)?;
        self.all_sessions.get(orig_idx)
    }
}

pub fn run_interactive_tui(
    sessions: Vec<SessionSummary>,
    workspace: &Path,
    app_dir: Option<&Path>,
) -> io::Result<TuiAction> {
    enable_raw_mode()?;
    let mut stderr_handle = stderr();
    execute!(stderr_handle, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stderr_handle);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(
        sessions,
        workspace.to_path_buf(),
        app_dir.map(|p| p.to_path_buf()),
    );

    let res = main_loop(&mut terminal, &mut state);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn main_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &mut AppState,
) -> io::Result<TuiAction> {
    loop {
        terminal.draw(|f| ui(f, state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }

            match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Esc) => {
                    return Ok(TuiAction::Exit);
                }
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    return Ok(TuiAction::Exit);
                }
                (KeyModifiers::NONE, KeyCode::Enter) => {
                    if let Some(session) = state.selected_session() {
                        let brain_path = get_brain_root(state.app_dir.as_deref()).join(&session.id);
                        return Ok(TuiAction::Navigate(brain_path));
                    }
                }
                (KeyModifiers::CONTROL, KeyCode::Char('o')) => {
                    if let Some(session) = state.selected_session() {
                        let brain_path = get_brain_root(state.app_dir.as_deref()).join(&session.id);
                        open_in_file_manager(&brain_path);
                    }
                }
                (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                    if let Some(session) = state.selected_session() {
                        let _ = set_pin(
                            &session.id,
                            Some(&state.workspace),
                            state.app_dir.as_deref(),
                        );
                        let cur_id = session.id.clone();
                        for s in &mut state.all_sessions {
                            s.pinned = s.id == cur_id;
                        }
                    }
                }
                (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                    if let Some(session) = state.selected_session() {
                        let brain_path = get_brain_root(state.app_dir.as_deref()).join(&session.id);
                        open_in_editor(&brain_path);
                    }
                }
                (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                    state.all_mode = !state.all_mode;
                    state.all_sessions = crate::core::fetch_sessions(
                        Some(&state.workspace),
                        state.all_mode,
                        state.app_dir.as_deref(),
                        None,
                    );
                    state.filter_sessions();
                }
                (KeyModifiers::NONE, KeyCode::Up) => {
                    state.previous();
                }
                (KeyModifiers::NONE, KeyCode::Down) => {
                    state.next();
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
        }
    }
}

fn ui(f: &mut Frame, state: &mut AppState) {
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
        " [MODO: TODOS OS PROJETOS] "
    } else {
        " [PROJETO ATUAL] "
    };
    let search_title = format!(" Antigravity Brain Navigator{} ", mode_tag);
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
            let tag = if s.pinned { "[FIXADO] " } else { "         " };
            let steps = format!("{:>5} p.", s.steps);
            let title = if s.title.len() > 28 {
                format!("{}...", &s.title[..25])
            } else {
                s.title.clone()
            };

            let line = Line::from(vec![
                Span::styled(
                    tag,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
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
    let stats = selected_session.and_then(|s| get_brain_stats(&s.id, state.app_dir.as_deref()));

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
        Span::raw(" Navegar  "),
        Span::styled(
            " [Ctrl+O] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Explorer  "),
        Span::styled(
            " [Ctrl+P] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Fixar  "),
        Span::styled(
            " [Ctrl+A] ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Todos  "),
        Span::styled(
            " [Esc/q] ",
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
        Span::raw(" Sair"),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Atalhos "));
    f.render_widget(footer, chunks[2]);
}

fn render_tui_preview<'a>(
    session: Option<&'a SessionSummary>,
    stats: Option<&'a BrainStats>,
) -> Paragraph<'a> {
    let mut text_lines = Vec::new();

    if let Some(s) = session {
        let pin_badge = if s.pinned { " [FIXADO]" } else { "" };
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
            "Nenhuma sessao selecionada.",
            Style::default().fg(Color::DarkGray),
        )));
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
