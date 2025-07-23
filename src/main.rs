// ABOUTME: Claude Code log viewer - Web interface for viewing Claude Code project logs
// ABOUTME: Parses JSONL conversation logs and serves them via web interface for auditing

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, get_service},
    Router,
};
use chrono::{DateTime, Utc};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc};
use tower_http::services::ServeDir;
use walkdir::WalkDir;

#[derive(Parser)]
#[clap(name = "cc-log-viewer")]
#[clap(about = "Claude Code log viewer - Web interface for viewing conversation logs")]
struct Cli {
    #[clap(
        help = "Path to projects directory containing log files (defaults to ~/.claude/projects/)"
    )]
    projects_dir: Option<PathBuf>,

    #[clap(short, long, default_value = "2006", help = "Port to serve on")]
    port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LogEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    summary: Option<String>,
    #[serde(rename = "parentUuid")]
    parent_uuid: Option<String>,
    #[serde(rename = "isSidechain")]
    is_sidechain: Option<bool>,
    #[serde(rename = "userType")]
    user_type: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    version: Option<String>,
    message: Option<Value>,
    uuid: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    #[serde(rename = "leafUuid")]
    leaf_uuid: Option<String>,
    #[serde(rename = "toolUseResult")]
    tool_use_result: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectSummary {
    name: String,
    path: String,
    session_count: usize,
    latest_activity: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
struct SessionSummary {
    id: String,
    summary: String,
    timestamp: DateTime<Utc>,
    message_count: usize,
    project_name: String,
}

#[derive(Debug, Clone)]
struct AppState {
    projects_dir: PathBuf,
    cached_projects: Arc<tokio::sync::RwLock<Vec<ProjectSummary>>>,
}

impl AppState {
    fn new(projects_dir: PathBuf) -> Self {
        Self {
            projects_dir,
            cached_projects: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
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
                                        if latest_activity.is_none_or(|latest| timestamp > latest) {
                                            latest_activity = Some(timestamp);
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

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn get_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectSummary>>, StatusCode> {
    if (state.refresh_cache().await).is_err() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let projects = state.cached_projects.read().await;
    Ok(Json(projects.clone()))
}

async fn get_sessions(
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

async fn get_session_logs(
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Default to ~/.claude/projects/ if not specified
    let projects_dir = if let Some(dir) = cli.projects_dir {
        dir
    } else {
        let home = std::env::var("HOME").map_err(|_| "Could not determine home directory")?;
        PathBuf::from(home).join(".claude").join("projects")
    };

    if !projects_dir.exists() {
        eprintln!(
            "Projects directory does not exist: {}",
            projects_dir.display()
        );
        eprintln!("Tip: Claude Code logs are typically stored in ~/.claude/projects/");
        std::process::exit(1);
    }

    let state = AppState::new(projects_dir);

    let app = Router::new()
        .route("/", get(index))
        .route("/api/projects", get(get_projects))
        .route("/api/projects/:project/sessions", get(get_sessions))
        .route(
            "/api/projects/:project/sessions/:session",
            get(get_session_logs),
        )
        .nest_service("/static", get_service(ServeDir::new("static")))
        .fallback(index) // Serve index.html for all other routes (SPA routing)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", cli.port)).await?;
    println!(
        "🚀 Claude Code Log Viewer running on http://localhost:{}",
        cli.port
    );

    axum::serve(listener, app).await?;

    Ok(())
}
