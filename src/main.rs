// ABOUTME: Claude Code log viewer - Web interface for viewing Claude Code project logs
// ABOUTME: Main executable that sets up CLI parsing and starts the web server

use axum::{
    routing::{get, get_service},
    Router,
};
use clap::Parser;
use std::path::PathBuf;
use tower_http::services::ServeDir;

use cc_log_viewer::{
    get_projects, get_session_logs, get_sessions, index, live_activity, websocket_handler, AppState,
    tui::TuiApp,
};

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

    #[clap(long, help = "Use terminal UI instead of web interface")]
    tui: bool,
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

    let state = AppState::new(projects_dir)
        .map_err(|e| format!("Failed to initialize watch manager: {}", e))?;

    if cli.tui {
        // Terminal UI mode
        println!("🖥️  Starting Claude Code Log Viewer in Terminal UI mode");
        println!("Press 'q' to quit, '↑/↓' to navigate, 'Enter' to select");
        
        let mut tui_app = TuiApp::new(state);
        tui_app.run().await?;
    } else {
        // Web UI mode (default)
        let app = Router::new()
            .route("/", get(index))
            .route("/live", get(live_activity))
            .route("/api/projects", get(get_projects))
            .route("/api/projects/:project/sessions", get(get_sessions))
            .route(
                "/api/projects/:project/sessions/:session",
                get(get_session_logs),
            )
            .route("/ws/watch", get(websocket_handler))
            .nest_service("/static", get_service(ServeDir::new("static")))
            .fallback(index) // Serve index.html for all other routes (SPA routing)
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", cli.port)).await?;
        println!(
            "🚀 Claude Code Log Viewer running on http://localhost:{}",
            cli.port
        );

        axum::serve(listener, app).await?;
    }

    Ok(())
}
