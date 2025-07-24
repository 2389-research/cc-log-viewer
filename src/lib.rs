// ABOUTME: Claude Code log viewer library - Core functionality for viewing and streaming JSONL logs
// ABOUTME: Exposes types and handlers for real-time WebSocket monitoring and rich tool rendering

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    http::{header, StatusCode},
    response::{Html, Json, Response},
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc, time::SystemTime};
use tokio::sync::broadcast;
use walkdir::WalkDir;

pub mod tui;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    #[serde(rename = "type")]
    pub entry_type: Option<String>,
    pub summary: Option<String>,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    #[serde(rename = "isSidechain")]
    pub is_sidechain: Option<bool>,
    #[serde(rename = "userType")]
    pub user_type: Option<String>,
    pub cwd: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub version: Option<String>,
    pub message: Option<Value>,
    pub uuid: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "requestId")]
    pub request_id: Option<String>,
    #[serde(rename = "leafUuid")]
    pub leaf_uuid: Option<String>,
    #[serde(rename = "toolUseResult")]
    pub tool_use_result: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub name: String,
    pub path: String,
    pub session_count: usize,
    pub latest_activity: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub summary: String,
    pub timestamp: DateTime<Utc>,
    pub message_count: usize,
    pub project_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WatchEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub project: String,
    pub session: Option<String>,
    pub entry: Option<LogEntry>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionState {
    pub project_name: String,
    pub session_file: PathBuf,
    pub last_position: u64,
    pub last_modified: SystemTime,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct WatchManager {
    _watcher: RecommendedWatcher,
    active_sessions: Arc<DashMap<String, SessionState>>,
    broadcast_tx: broadcast::Sender<WatchEvent>,
    projects_dir: PathBuf,
}

impl WatchManager {
    pub fn new(projects_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (broadcast_tx, _) = broadcast::channel(1000);
        let active_sessions = Arc::new(DashMap::new());

        let tx_clone = broadcast_tx.clone();
        let sessions_clone = active_sessions.clone();
        let projects_dir_clone = projects_dir.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if let Err(e) =
                    Self::handle_fs_event(event, &tx_clone, &sessions_clone, &projects_dir_clone)
                {
                    eprintln!("Error handling file system event: {}", e);
                }
            }
        })?;

        watcher.watch(&projects_dir, RecursiveMode::Recursive)?;

        Ok(WatchManager {
            _watcher: watcher,
            active_sessions,
            broadcast_tx,
            projects_dir,
        })
    }

    fn handle_fs_event(
        event: Event,
        broadcast_tx: &broadcast::Sender<WatchEvent>,
        active_sessions: &DashMap<String, SessionState>,
        _projects_dir: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if path.extension().is_some_and(|ext| ext == "jsonl") {
                        if let Some(project_name) = path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                        {
                            let session_id = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                                .to_string();

                            // Read new entries from the file
                            if let Ok(metadata) = fs::metadata(&path) {
                                let key = format!("{}:{}", project_name, session_id);
                                let current_pos =
                                    if let Some(session_state) = active_sessions.get(&key) {
                                        session_state.last_position
                                    } else {
                                        0
                                    };

                                if let Ok(entries_with_positions) =
                                    Self::read_new_entries(&path, current_pos)
                                {
                                    // Broadcast new entries (limit to prevent spam)
                                    let max_entries_per_event = 10;
                                    let mut last_processed_position = current_pos;

                                    for (entry, entry_position) in entries_with_positions
                                        .into_iter()
                                        .take(max_entries_per_event)
                                    {
                                        let watch_event = WatchEvent {
                                            event_type: "log_entry".to_string(),
                                            project: project_name.to_string(),
                                            session: Some(session_id.clone()),
                                            entry: Some(entry),
                                            timestamp: Utc::now(),
                                        };

                                        if broadcast_tx.send(watch_event).is_err() {
                                            // Channel is closed, stop trying to send
                                            break;
                                        }

                                        last_processed_position = entry_position;
                                    }

                                    // Update session state with the position of the last entry actually processed
                                    active_sessions.insert(
                                        key,
                                        SessionState {
                                            project_name: project_name.to_string(),
                                            session_file: path.clone(),
                                            last_position: last_processed_position,
                                            last_modified: metadata
                                                .modified()
                                                .unwrap_or(SystemTime::now()),
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn read_new_entries(
        path: &PathBuf,
        from_position: u64,
    ) -> Result<Vec<(LogEntry, u64)>, Box<dyn std::error::Error + Send + Sync>> {
        // Handle potential file access errors gracefully
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Warning: Could not read file {}: {}", path.display(), e);
                return Ok(Vec::new());
            }
        };

        let content_bytes = content.as_bytes();
        let mut entries_with_positions = Vec::new();

        // Split content into lines while tracking actual byte positions
        let mut line_start = 0usize;
        while line_start < content_bytes.len() {
            // Find the end of the current line
            let mut line_end = line_start;
            while line_end < content_bytes.len() && content_bytes[line_end] != b'\n' {
                line_end += 1;
            }

            // Calculate the byte position of this line
            let line_byte_start = line_start as u64;
            let line_byte_end = if line_end < content_bytes.len() {
                // Include newline character
                (line_end + 1) as u64
            } else {
                // Last line without newline
                line_end as u64
            };

            // Process line if it's past our starting position
            if line_byte_start >= from_position {
                // Extract the line content (excluding newline)
                let line_content =
                    std::str::from_utf8(&content_bytes[line_start..line_end]).unwrap_or("");

                // Only parse lines that look like JSON to avoid errors
                if line_content.trim().starts_with('{') && line_content.trim().ends_with('}') {
                    if let Ok(entry) = serde_json::from_str::<LogEntry>(line_content) {
                        entries_with_positions.push((entry, line_byte_end));
                    }
                }
            }

            // Move to next line
            line_start = if line_end < content_bytes.len() {
                line_end + 1 // Skip the newline
            } else {
                break;
            };
        }

        Ok(entries_with_positions)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WatchEvent> {
        self.broadcast_tx.subscribe()
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub projects_dir: PathBuf,
    pub cached_projects: Arc<tokio::sync::RwLock<Vec<ProjectSummary>>>,
    pub watch_manager: Arc<WatchManager>,
}

impl AppState {
    pub fn new(projects_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let watch_manager = Arc::new(WatchManager::new(projects_dir.clone())?);

        Ok(Self {
            projects_dir,
            cached_projects: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            watch_manager,
        })
    }

    async fn refresh_cache(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut projects = Vec::new();

        for entry in WalkDir::new(&self.projects_dir).min_depth(1).max_depth(1) {
            let entry = entry?;
            if entry.file_type().is_dir() {
                let project_name = entry.file_name().to_string_lossy().to_string();
                let project_path = entry.path().to_string_lossy().to_string();

                let mut session_count = 0;
                let mut latest_activity: Option<DateTime<Utc>> = None;

                for log_entry in WalkDir::new(entry.path()).min_depth(1).max_depth(1) {
                    let log_entry = log_entry?;
                    if log_entry.file_type().is_file()
                        && log_entry
                            .path()
                            .extension()
                            .is_some_and(|ext| ext == "jsonl")
                    {
                        session_count += 1;

                        if let Ok(content) = fs::read_to_string(log_entry.path()) {
                            for line in content.lines().take(5) {
                                if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                                    if let Some(timestamp) = entry.timestamp {
                                        match latest_activity {
                                            None => latest_activity = Some(timestamp),
                                            Some(latest) if timestamp > latest => {
                                                latest_activity = Some(timestamp)
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                projects.push(ProjectSummary {
                    name: project_name,
                    path: project_path,
                    session_count,
                    latest_activity,
                });
            }
        }

        projects.sort_by(|a, b| b.latest_activity.cmp(&a.latest_activity));

        *self.cached_projects.write().await = projects;
        Ok(())
    }
}

pub async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

pub async fn live_activity() -> Html<&'static str> {
    Html(include_str!("../static/live.html"))
}

pub async fn get_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectSummary>>, StatusCode> {
    if let Err(e) = state.refresh_cache().await {
        eprintln!("Failed to refresh project cache: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let projects = state.cached_projects.read().await;
    Ok(Json(projects.clone()))
}

pub async fn get_sessions(
    Path(project_name): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<SessionSummary>>, StatusCode> {
    let project_path = state.projects_dir.join(&project_name);

    if !project_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut sessions = Vec::new();

    for entry in WalkDir::new(&project_path).min_depth(1).max_depth(1) {
        let entry = entry.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "jsonl")
        {
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
                    project_name: project_name.clone(),
                });
            }
        }
    }

    sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(Json(sessions))
}

pub async fn get_session_logs(
    Path((project_name, session_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<Vec<LogEntry>>, StatusCode> {
    let log_path = state
        .projects_dir
        .join(&project_name)
        .join(format!("{}.jsonl", session_id));

    if !log_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = fs::read_to_string(&log_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut entries = Vec::new();
    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
            entries.push(entry);
        }
    }

    Ok(Json(entries))
}

pub async fn export_session_markdown(
    Path((project_name, session_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Response, StatusCode> {
    let log_path = state
        .projects_dir
        .join(&project_name)
        .join(format!("{}.jsonl", session_id));

    if !log_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let content = fs::read_to_string(&log_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut entries = Vec::new();
    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
            entries.push(entry);
        }
    }

    let markdown_content = generate_markdown_export(&entries, &session_id, &project_name);

    let filename = format!("{}-{}.md", project_name, session_id);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(markdown_content))
        .unwrap())
}

fn generate_markdown_export(entries: &[LogEntry], session_id: &str, project_name: &str) -> String {
    let mut markdown = String::new();

    // Header
    markdown.push_str(&format!("# Claude Code Session: {}\n\n", session_id));
    markdown.push_str(&format!("**Project:** {}\n", project_name));

    if let Some(first_entry) = entries.first() {
        if let Some(timestamp) = &first_entry.timestamp {
            markdown.push_str(&format!(
                "**Date:** {}\n",
                timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }
    }

    markdown.push_str("\n---\n\n");

    let mut current_tool_use: Option<&LogEntry> = None;

    for entry in entries {
        match entry.entry_type.as_deref() {
            Some("summary") => {
                if let Some(summary) = &entry.summary {
                    markdown.push_str(&format!("## 📋 Session Summary\n\n{}\n\n", summary));
                }
            }
            Some("user") => {
                if let Some(message) = &entry.message {
                    if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                        markdown.push_str(&format!("## 👤 User\n\n{}\n\n", content));
                    }
                }
            }
            Some("assistant") => {
                if let Some(message) = &entry.message {
                    if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                        markdown.push_str(&format!("## 🤖 Assistant\n\n{}\n\n", content));
                    }
                }
            }
            Some("toolUse") => {
                current_tool_use = Some(entry);
                if let Some(message) = &entry.message {
                    if let Some(tool_name) = message.get("name").and_then(|n| n.as_str()) {
                        let tool_icon = get_tool_icon(tool_name);
                        markdown.push_str(&format!("### {} {}\n\n", tool_icon, tool_name));

                        if let Some(input) = message.get("input") {
                            render_tool_input(&mut markdown, tool_name, input);
                        }
                    }
                }
            }
            Some("toolResult") => {
                if let Some(tool_use_entry) = current_tool_use {
                    if let Some(tool_result) = &entry.tool_use_result {
                        if let Some(message) = &tool_use_entry.message {
                            if let Some(tool_name) = message.get("name").and_then(|n| n.as_str()) {
                                if let Some(input) = message.get("input") {
                                    render_tool_result(
                                        &mut markdown,
                                        tool_name,
                                        input,
                                        tool_result,
                                    );
                                }
                            }
                        }
                    }
                }
                current_tool_use = None;
            }
            _ => {
                // Handle other entry types if needed
            }
        }

        // Add timestamp if available
        if let Some(timestamp) = &entry.timestamp {
            markdown.push_str(&format!("*Time: {}*\n\n", timestamp.format("%H:%M:%S")));
        }
    }

    markdown
}

fn get_tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        "Bash" => "💻",
        "Read" => "📖",
        "Edit" => "✏️",
        "MultiEdit" => "🔄",
        "Write" => "📝",
        "TodoWrite" => "📝",
        "LS" => "📂",
        "Grep" => "🔍",
        "Glob" => "🌐",
        "WebFetch" => "🌐",
        "Task" => "🎯",
        _ => "🔧",
    }
}

fn render_tool_input(markdown: &mut String, tool_name: &str, input: &Value) {
    match tool_name {
        "Bash" => render_bash_input(markdown, input),
        "Read" => render_read_input(markdown, input),
        "Edit" => render_edit_input(markdown, input),
        "MultiEdit" => render_multiedit_input(markdown, input),
        "Write" => render_write_input(markdown, input),
        "TodoWrite" => render_todowrite_input(markdown, input),
        "LS" => render_ls_input(markdown, input),
        "Grep" => render_grep_input(markdown, input),
        _ => render_generic_input(markdown, input),
    }
}

fn render_tool_result(markdown: &mut String, tool_name: &str, input: &Value, result: &Value) {
    if let Some(content) = result.get("content").and_then(|c| c.as_str()) {
        match tool_name {
            "Bash" => render_bash_result(markdown, content),
            "Read" => render_read_result(markdown, input, content),
            "Edit" => render_edit_result(markdown, content),
            "LS" => render_ls_result(markdown, content),
            "Grep" => render_grep_result(markdown, content),
            _ => render_generic_result(markdown, content),
        }
    }
}

fn render_bash_input(markdown: &mut String, input: &Value) {
    if let Some(command) = input.get("command").and_then(|c| c.as_str()) {
        markdown.push_str("```bash\n");
        markdown.push_str(&format!("$ {}\n", command));
        if let Some(description) = input.get("description").and_then(|d| d.as_str()) {
            markdown.push_str(&format!("# {}\n", description));
        }
        markdown.push_str("```\n\n");
    }
}

fn render_bash_result(markdown: &mut String, content: &str) {
    markdown.push_str("**Output:**\n```\n");
    markdown.push_str(content);
    markdown.push_str("\n```\n\n");
}

fn render_read_input(markdown: &mut String, input: &Value) {
    if let Some(file_path) = input.get("file_path").and_then(|f| f.as_str()) {
        markdown.push_str(&format!("**📄 {}**\n", file_path));

        if let (Some(offset), Some(limit)) = (
            input.get("offset").and_then(|o| o.as_u64()),
            input.get("limit").and_then(|l| l.as_u64()),
        ) {
            markdown.push_str(&format!("*Lines: {}-{}*\n", offset + 1, offset + limit));
        }
        markdown.push('\n');
    }
}

fn render_read_result(markdown: &mut String, _input: &Value, content: &str) {
    markdown.push_str("**Content:**\n```\n");
    markdown.push_str(content);
    markdown.push_str("\n```\n\n");
}

fn render_edit_input(markdown: &mut String, input: &Value) {
    if let Some(file_path) = input.get("file_path").and_then(|f| f.as_str()) {
        markdown.push_str(&format!("**✏️ {}**\n\n", file_path));

        if let (Some(old_string), Some(new_string)) = (
            input.get("old_string").and_then(|o| o.as_str()),
            input.get("new_string").and_then(|n| n.as_str()),
        ) {
            markdown.push_str("```diff\n");
            for line in old_string.lines() {
                markdown.push_str(&format!("- {}\n", line));
            }
            for line in new_string.lines() {
                markdown.push_str(&format!("+ {}\n", line));
            }
            markdown.push_str("```\n\n");
        }
    }
}

fn render_edit_result(markdown: &mut String, content: &str) {
    if !content.trim().is_empty() {
        markdown.push_str("**Result:**\n```\n");
        markdown.push_str(content);
        markdown.push_str("\n```\n\n");
    }
}

fn render_multiedit_input(markdown: &mut String, input: &Value) {
    if let Some(file_path) = input.get("file_path").and_then(|f| f.as_str()) {
        if let Some(edits) = input.get("edits").and_then(|e| e.as_array()) {
            markdown.push_str(&format!(
                "**🔄 Multiple Edits to {} ({} changes)**\n\n",
                file_path,
                edits.len()
            ));

            for (i, edit) in edits.iter().enumerate() {
                markdown.push_str(&format!("**Edit {}**", i + 1));
                if let Some(replace_all) = edit.get("replace_all").and_then(|r| r.as_bool()) {
                    if replace_all {
                        markdown.push_str(" (replace all)");
                    }
                }
                markdown.push('\n');

                if let (Some(old_string), Some(new_string)) = (
                    edit.get("old_string").and_then(|o| o.as_str()),
                    edit.get("new_string").and_then(|n| n.as_str()),
                ) {
                    markdown.push_str("```diff\n");
                    for line in old_string.lines() {
                        markdown.push_str(&format!("- {}\n", line));
                    }
                    for line in new_string.lines() {
                        markdown.push_str(&format!("+ {}\n", line));
                    }
                    markdown.push_str("```\n\n");
                }
            }
        }
    }
}

fn render_write_input(markdown: &mut String, input: &Value) {
    if let Some(file_path) = input.get("file_path").and_then(|f| f.as_str()) {
        markdown.push_str(&format!("**📝 {}**\n\n", file_path));

        if let Some(content) = input.get("content").and_then(|c| c.as_str()) {
            markdown.push_str("**Content:**\n```\n");
            markdown.push_str(content);
            markdown.push_str("\n```\n\n");
        }
    }
}

fn render_todowrite_input(markdown: &mut String, input: &Value) {
    if let Some(todos) = input.get("todos").and_then(|t| t.as_array()) {
        markdown.push_str(&format!("**📝 Todo List ({} items)**\n\n", todos.len()));

        for todo in todos {
            let status = todo
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("pending");
            let content = todo.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let priority = todo
                .get("priority")
                .and_then(|p| p.as_str())
                .unwrap_or("medium");
            let id = todo.get("id").and_then(|i| i.as_str()).unwrap_or("");

            let (status_icon, format_content) = match status {
                "completed" => ("✅", format!("~~{}~~", content)),
                "in_progress" => ("🔄", format!("**{}**", content)),
                _ => ("⭕", content.to_string()),
            };

            let priority_icon = match priority {
                "high" => "🟢",
                "medium" => "🟠",
                "low" => "🔴",
                _ => "⚪",
            };

            markdown.push_str(&format!("{} {}\n", status_icon, format_content));
            markdown.push_str(&format!(
                "{} {} priority • ID: {}\n\n",
                priority_icon, priority, id
            ));
        }
    }
}

fn render_ls_input(markdown: &mut String, input: &Value) {
    if let Some(path) = input.get("path").and_then(|p| p.as_str()) {
        markdown.push_str(&format!("**📂 {}**\n\n", path));
    }
}

fn render_ls_result(markdown: &mut String, content: &str) {
    markdown.push_str("**Directory listing:**\n```\n");
    markdown.push_str(content);
    markdown.push_str("\n```\n\n");
}

fn render_grep_input(markdown: &mut String, input: &Value) {
    if let Some(pattern) = input.get("pattern").and_then(|p| p.as_str()) {
        markdown.push_str(&format!("**Pattern:** `{}`\n", pattern));

        if let Some(path) = input.get("path").and_then(|p| p.as_str()) {
            markdown.push_str(&format!("**Path:** {}\n", path));
        }

        if let Some(glob) = input.get("glob").and_then(|g| g.as_str()) {
            markdown.push_str(&format!("**Glob:** {}\n", glob));
        }

        markdown.push('\n');
    }
}

fn render_grep_result(markdown: &mut String, content: &str) {
    markdown.push_str("**Matches:**\n```\n");
    markdown.push_str(content);
    markdown.push_str("\n```\n\n");
}

fn render_generic_input(markdown: &mut String, input: &Value) {
    markdown.push_str("**Input:**\n```json\n");
    markdown.push_str(&serde_json::to_string_pretty(input).unwrap_or_default());
    markdown.push_str("\n```\n\n");
}

fn render_generic_result(markdown: &mut String, content: &str) {
    markdown.push_str("**Result:**\n```\n");
    markdown.push_str(content);
    markdown.push_str("\n```\n\n");
}

pub async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut watch_rx = state.watch_manager.subscribe();

    // Handle incoming messages from client
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    println!("Received WebSocket message: {}", text);
                    // TODO: Handle client messages for subscription management
                }
                Ok(Message::Close(_)) => {
                    println!("WebSocket connection closed");
                    break;
                }
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Handle outgoing messages to client
    let send_task = tokio::spawn(async move {
        while let Ok(watch_event) = watch_rx.recv().await {
            let json_msg = match serde_json::to_string(&watch_event) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("Failed to serialize watch event: {}", e);
                    continue;
                }
            };

            if sender.send(Message::Text(json_msg)).await.is_err() {
                break;
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = recv_task => {},
        _ = send_task => {},
    }
}
