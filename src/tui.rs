// ABOUTME: Terminal User Interface for Claude Code log viewer
// ABOUTME: Provides interactive terminal-based navigation, review, and export capabilities

use crate::{AppState, LogEntry, ProjectSummary, SessionSummary};
use chrono::Utc;
use walkdir::WalkDir;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame, Terminal,
};
use std::{fs, io};
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
enum AppMode {
    ProjectList,
    SessionList,
    ConversationView,
    Export,
}

#[derive(Debug)]
pub struct TuiApp {
    app_state: AppState,
    mode: AppMode,
    projects: Vec<ProjectSummary>,
    sessions: Vec<SessionSummary>,
    conversation: Vec<LogEntry>,
    selected_project: Option<usize>,
    selected_session: Option<usize>,
    selected_message: Option<usize>,
    project_list_state: ListState,
    session_list_state: ListState,
    message_list_state: ListState,
    scroll_offset: usize,
    status_message: String,
    should_quit: bool,
    last_update: Instant,
}

impl TuiApp {
    pub fn new(app_state: AppState) -> Self {
        let mut project_list_state = ListState::default();
        project_list_state.select(Some(0));

        Self {
            app_state,
            mode: AppMode::ProjectList,
            projects: Vec::new(),
            sessions: Vec::new(),
            conversation: Vec::new(),
            selected_project: Some(0),
            selected_session: None,
            selected_message: None,
            project_list_state,
            session_list_state: ListState::default(),
            message_list_state: ListState::default(),
            scroll_offset: 0,
            status_message: "Welcome to Claude Code Log Viewer TUI".to_string(),
            should_quit: false,
            last_update: Instant::now(),
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Load initial data
        self.refresh_projects().await?;

        let result = self.run_app(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if self.should_quit {
                break;
            }

            // Handle events with timeout for real-time updates
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key_event(key.code).await?;
                    }
                }
            }

            // Periodic refresh for real-time monitoring
            if self.last_update.elapsed() > Duration::from_secs(2) {
                match self.mode {
                    AppMode::ProjectList => {
                        self.refresh_projects().await?;
                    }
                    AppMode::SessionList => {
                        if let Some(project_idx) = self.selected_project {
                            if let Some(project) = self.projects.get(project_idx) {
                                self.refresh_sessions(&project.name).await?;
                            }
                        }
                    }
                    AppMode::ConversationView => {
                        if let Some(project_idx) = self.selected_project {
                            if let Some(session_idx) = self.selected_session {
                                if let Some(project) = self.projects.get(project_idx) {
                                    if let Some(session) = self.sessions.get(session_idx) {
                                        self.refresh_conversation(&project.name, &session.id).await?;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
                self.last_update = Instant::now();
            }
        }

        Ok(())
    }

    async fn handle_key_event(&mut self, key: KeyCode) -> Result<(), Box<dyn std::error::Error>> {
        match key {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                match self.mode {
                    AppMode::SessionList => {
                        self.mode = AppMode::ProjectList;
                        self.selected_session = None;
                        self.sessions.clear();
                    }
                    AppMode::ConversationView => {
                        self.mode = AppMode::SessionList;
                        self.selected_message = None;
                        self.conversation.clear();
                        self.scroll_offset = 0;
                    }
                    AppMode::Export => {
                        self.mode = AppMode::ConversationView;
                    }
                    _ => {}
                }
            }
            KeyCode::Enter => {
                match self.mode {
                    AppMode::ProjectList => {
                        if let Some(selected) = self.selected_project {
                            if let Some(project) = self.projects.get(selected) {
                                self.refresh_sessions(&project.name).await?;
                                self.mode = AppMode::SessionList;
                                self.selected_session = Some(0);
                                self.session_list_state.select(Some(0));
                            }
                        }
                    }
                    AppMode::SessionList => {
                        if let Some(session_idx) = self.selected_session {
                            if let Some(project_idx) = self.selected_project {
                                if let Some(project) = self.projects.get(project_idx) {
                                    if let Some(session) = self.sessions.get(session_idx) {
                                        self.refresh_conversation(&project.name, &session.id).await?;
                                        self.mode = AppMode::ConversationView;
                                        self.selected_message = Some(0);
                                        self.message_list_state.select(Some(0));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Up => {
                match self.mode {
                    AppMode::ProjectList => {
                        if let Some(selected) = self.selected_project {
                            if selected > 0 {
                                self.selected_project = Some(selected - 1);
                                self.project_list_state.select(Some(selected - 1));
                            }
                        }
                    }
                    AppMode::SessionList => {
                        if let Some(selected) = self.selected_session {
                            if selected > 0 {
                                self.selected_session = Some(selected - 1);
                                self.session_list_state.select(Some(selected - 1));
                            }
                        }
                    }
                    AppMode::ConversationView => {
                        if self.scroll_offset > 0 {
                            self.scroll_offset -= 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Down => {
                match self.mode {
                    AppMode::ProjectList => {
                        if let Some(selected) = self.selected_project {
                            if selected < self.projects.len().saturating_sub(1) {
                                self.selected_project = Some(selected + 1);
                                self.project_list_state.select(Some(selected + 1));
                            }
                        }
                    }
                    AppMode::SessionList => {
                        if let Some(selected) = self.selected_session {
                            if selected < self.sessions.len().saturating_sub(1) {
                                self.selected_session = Some(selected + 1);
                                self.session_list_state.select(Some(selected + 1));
                            }
                        }
                    }
                    AppMode::ConversationView => {
                        if self.scroll_offset < self.conversation.len().saturating_sub(1) {
                            self.scroll_offset += 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Char('e') => {
                if self.mode == AppMode::ConversationView {
                    self.mode = AppMode::Export;
                }
            }
            KeyCode::Char('r') => {
                // Manual refresh
                match self.mode {
                    AppMode::ProjectList => {
                        self.refresh_projects().await?;
                        self.status_message = "Projects refreshed".to_string();
                    }
                    AppMode::SessionList => {
                        if let Some(project_idx) = self.selected_project {
                            if let Some(project) = self.projects.get(project_idx) {
                                self.refresh_sessions(&project.name).await?;
                                self.status_message = "Sessions refreshed".to_string();
                            }
                        }
                    }
                    AppMode::ConversationView => {
                        if let Some(project_idx) = self.selected_project {
                            if let Some(session_idx) = self.selected_session {
                                if let Some(project) = self.projects.get(project_idx) {
                                    if let Some(session) = self.sessions.get(session_idx) {
                                        self.refresh_conversation(&project.name, &session.id).await?;
                                        self.status_message = "Conversation refreshed".to_string();
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Char('s') => {
                if self.mode == AppMode::Export {
                    self.export_conversation().await?;
                    self.mode = AppMode::ConversationView;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(f.area());

        match self.mode {
            AppMode::ProjectList => {
                self.render_project_list(f, chunks[0]);
            }
            AppMode::SessionList => {
                self.render_session_list(f, chunks[0]);
            }
            AppMode::ConversationView => {
                self.render_conversation(f, chunks[0]);
            }
            AppMode::Export => {
                self.render_export_dialog(f, chunks[0]);
            }
        }

        self.render_status_bar(f, chunks[1]);
    }

    fn render_project_list(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .projects
            .iter()
            .enumerate()
            .map(|(i, project)| {
                let style = if Some(i) == self.selected_project {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let activity = project
                    .latest_activity
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "No activity".to_string());

                ListItem::new(vec![Line::from(vec![
                    Span::styled(format!("📁 {}", project.name), style),
                    Span::raw(format!(" ({} sessions, last: {})", project.session_count, activity)),
                ])])
            })
            .collect();

        let projects_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Projects (↑/↓ to navigate, Enter to select, q to quit)"),
            )
            .highlight_style(Style::default().fg(Color::Black).bg(Color::Yellow));

        f.render_stateful_widget(projects_list, area, &mut self.project_list_state);
    }

    fn render_session_list(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, session)| {
                let style = if Some(i) == self.selected_session {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(vec![Line::from(vec![
                    Span::styled(format!("💬 {}", session.summary), style),
                    Span::raw(format!(
                        " ({} messages, {})",
                        session.message_count,
                        session.timestamp.format("%Y-%m-%d %H:%M")
                    )),
                ])])
            })
            .collect();

        let title = if let Some(project_idx) = self.selected_project {
            if let Some(project) = self.projects.get(project_idx) {
                format!("Sessions in {} (↑/↓ to navigate, Enter to select, Esc to go back)", project.name)
            } else {
                "Sessions".to_string()
            }
        } else {
            "Sessions".to_string()
        };

        let sessions_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().fg(Color::Black).bg(Color::Yellow));

        f.render_stateful_widget(sessions_list, area, &mut self.session_list_state);
    }

    fn render_conversation(&mut self, f: &mut Frame, area: Rect) {
        let title = if let Some(project_idx) = self.selected_project {
            if let Some(session_idx) = self.selected_session {
                if let Some(project) = self.projects.get(project_idx) {
                    if let Some(session) = self.sessions.get(session_idx) {
                        format!("Conversation: {} / {} (↑/↓ to scroll, e to export, Esc to go back)", project.name, session.summary)
                    } else {
                        "Conversation".to_string()
                    }
                } else {
                    "Conversation".to_string()
                }
            } else {
                "Conversation".to_string()
            }
        } else {
            "Conversation".to_string()
        };

        let visible_messages = self.conversation
            .iter()
            .skip(self.scroll_offset)
            .take(area.height.saturating_sub(2) as usize)
            .enumerate()
            .map(|(i, entry)| {
                let role = entry.message
                    .as_ref()
                    .and_then(|m| m.get("role"))
                    .and_then(|r| r.as_str())
                    .unwrap_or("system");

                let content = entry.message
                    .as_ref()
                    .and_then(|m| m.get("content"))
                    .and_then(|c| {
                        if c.is_string() {
                            c.as_str().map(|s| s.to_string())
                        } else if c.is_array() {
                            Some(format!("{}", c))
                        } else {
                            Some(format!("{}", c))
                        }
                    })
                    .unwrap_or_else(|| "No content".to_string());

                let icon = match role {
                    "user" => "👤",
                    "assistant" => "🤖",
                    _ => "ℹ️",
                };

                let style = match role {
                    "user" => Style::default().fg(Color::Cyan),
                    "assistant" => Style::default().fg(Color::Green),
                    _ => Style::default().fg(Color::Gray),
                };

                let timestamp = entry.timestamp
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                Line::from(vec![
                    Span::styled(format!("{} [{}] ", icon, timestamp), style),
                    Span::styled(content.chars().take(120).collect::<String>(), style),
                    if content.len() > 120 { Span::raw("...") } else { Span::raw("") },
                ])
            })
            .collect::<Vec<_>>();

        let conversation_text = Text::from(visible_messages);
        let paragraph = Paragraph::new(conversation_text)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }

    fn render_export_dialog(&mut self, f: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 20, area);
        
        f.render_widget(Clear, popup_area);
        
        let block = Block::default()
            .title("Export Conversation")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::DarkGray));

        let text = Text::from(vec![
            Line::from("Press 's' to save conversation to file"),
            Line::from("Press Esc to cancel"),
            Line::from(""),
            Line::from("File will be saved as: conversation_export.txt"),
        ]);

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, popup_area);
    }

    fn render_status_bar(&mut self, f: &mut Frame, area: Rect) {
        let status_text = match self.mode {
            AppMode::ProjectList => format!("{} | q: Quit, r: Refresh", self.status_message),
            AppMode::SessionList => format!("{} | Esc: Back, r: Refresh", self.status_message),
            AppMode::ConversationView => format!("{} | Esc: Back, e: Export, r: Refresh", self.status_message),
            AppMode::Export => format!("{} | s: Save, Esc: Cancel", self.status_message),
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::White).bg(Color::Blue))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(status, area);
    }

    async fn refresh_projects(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Err(e) = self.app_state.refresh_cache().await {
            self.status_message = format!("Failed to refresh projects: {}", e);
            return Ok(());
        }

        let projects = self.app_state.cached_projects.read().await;
        self.projects = projects.clone();

        if self.projects.is_empty() {
            self.status_message = "No projects found".to_string();
            self.selected_project = None;
            self.project_list_state.select(None);
        } else if self.selected_project.is_none() {
            self.selected_project = Some(0);
            self.project_list_state.select(Some(0));
        }

        Ok(())
    }

    async fn refresh_sessions(&mut self, project_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let project_path = self.app_state.projects_dir.join(project_name);

        if !project_path.exists() {
            self.status_message = "Project directory not found".to_string();
            return Ok(());
        }

        let mut sessions = Vec::new();

        for entry in WalkDir::new(&project_path).min_depth(1).max_depth(1) {
            let entry = entry?;
            if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "jsonl") {
                let session_id = entry
                    .path()
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let mut summary = "Untitled Session".to_string();
                    let mut timestamp = Utc::now();
                    let message_count = content.lines().count();

                    for line in content.lines().take(10) {
                        if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                            if entry.entry_type.as_deref() == Some("summary") {
                                if let Some(s) = entry.summary {
                                    summary = s;
                                }
                            }
                            if let Some(ts) = entry.timestamp {
                                timestamp = ts;
                                break;
                            }
                        }
                    }

                    sessions.push(SessionSummary {
                        id: session_id,
                        summary,
                        timestamp,
                        message_count,
                        project_name: project_name.to_string(),
                    });
                }
            }
        }

        sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        self.sessions = sessions;

        if self.sessions.is_empty() {
            self.status_message = "No sessions found in project".to_string();
            self.selected_session = None;
            self.session_list_state.select(None);
        } else if self.selected_session.is_none() {
            self.selected_session = Some(0);
            self.session_list_state.select(Some(0));
        }

        Ok(())
    }

    async fn refresh_conversation(&mut self, project_name: &str, session_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let log_path = self.app_state
            .projects_dir
            .join(project_name)
            .join(format!("{}.jsonl", session_id));

        if !log_path.exists() {
            self.status_message = "Session file not found".to_string();
            return Ok(());
        }

        let content = fs::read_to_string(&log_path)?;
        let mut entries = Vec::new();

        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                entries.push(entry);
            }
        }

        self.conversation = entries;
        self.scroll_offset = 0;

        Ok(())
    }

    async fn export_conversation(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.conversation.is_empty() {
            self.status_message = "No conversation to export".to_string();
            return Ok(());
        }

        let mut export_content = String::new();
        export_content.push_str("Claude Code Conversation Export\n");
        export_content.push_str("================================\n\n");

        for entry in &self.conversation {
            if let Some(message) = &entry.message {
                let role = message.get("role")
                    .and_then(|r| r.as_str())
                    .unwrap_or("system");

                let content = message.get("content")
                    .and_then(|c| {
                        if c.is_string() {
                            c.as_str().map(|s| s.to_string())
                        } else {
                            Some(format!("{}", c))
                        }
                    })
                    .unwrap_or_else(|| "No content".to_string());

                let timestamp = entry.timestamp
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                export_content.push_str(&format!("[{}] {}: {}\n\n", timestamp, role.to_uppercase(), content));
            }
        }

        let filename = "conversation_export.txt";
        fs::write(filename, export_content)?;
        self.status_message = format!("Conversation exported to {}", filename);

        Ok(())
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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