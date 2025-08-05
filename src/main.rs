// ABOUTME: Claude Code log viewer - Web interface for viewing Claude Code project logs
// ABOUTME: Main executable that sets up CLI parsing and starts the web server

use axum::{
    routing::{get, get_service},
    Router,
};
use clap::Parser;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tower_http::services::ServeDir;

use cc_log_viewer::{
    export_session_markdown, get_projects, get_session_logs, get_sessions, index, live_activity,
    tui::TuiApp, websocket_handler, AppState,
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

    #[clap(long, help = "Export projects to markdown format")]
    export: bool,

    #[clap(
        long,
        help = "Export all projects to markdown (requires --export)",
        requires = "export"
    )]
    export_all: bool,

    #[clap(
        long,
        help = "Specific project names to export (comma-separated, requires --export)",
        requires = "export"
    )]
    export_projects: Option<String>,

    #[clap(
        long,
        help = "Destination directory for exported markdown files (defaults to ./exports)",
        requires = "export"
    )]
    export_dir: Option<PathBuf>,

    #[clap(
        long,
        help = "Incrementally update exports in claude-code-exports directory (only export changed files)"
    )]
    update_export: bool,
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

    let state = AppState::new(projects_dir.clone())
        .map_err(|e| format!("Failed to initialize watch manager: {}", e))?;

    // Handle update export mode
    if cli.update_export {
        let export_dir = PathBuf::from("./claude-code-exports");
        update_export_mode(&projects_dir, &export_dir).await?;
        return Ok(());
    }

    // Handle export mode
    if cli.export {
        let export_dir = cli.export_dir.unwrap_or_else(|| PathBuf::from("./exports"));

        // Create export directory if it doesn't exist
        if !export_dir.exists() {
            fs::create_dir_all(&export_dir)?;
            println!("📁 Created export directory: {}", export_dir.display());
        }

        if cli.export_all {
            // Export all projects
            export_all_projects(&projects_dir, &export_dir).await?;
        } else if let Some(project_names) = cli.export_projects {
            // Export specific projects
            let projects: Vec<&str> = project_names.split(',').map(|s| s.trim()).collect();
            export_specific_projects(&projects_dir, &export_dir, &projects).await?;
        } else {
            eprintln!("❌ Error: --export requires either --export-all or --export-projects");
            std::process::exit(1);
        }

        println!("✅ Export completed successfully!");
        return Ok(());
    }

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
            .route(
                "/api/projects/:project/sessions/:session/export/markdown",
                get(export_session_markdown),
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

fn clean_project_name_for_export(project_name: &str) -> String {
    let mut cleaned_name = project_name.to_string();

    // Remove leading dash to avoid command-line flag conflicts
    if cleaned_name.starts_with('-') {
        cleaned_name = cleaned_name.trim_start_matches('-').to_string();
    }

    // Ensure we still have a valid name
    if cleaned_name.is_empty() {
        cleaned_name = "root".to_string();
    }

    cleaned_name
}

async fn update_export_mode(
    projects_dir: &Path,
    export_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Updating exports in claude-code-exports directory for current project...");

    // Get current working directory to determine the project name
    let current_dir = std::env::current_dir()?;
    let claude_project_name = current_dir.display().to_string().replace('/', "-");

    // Create a clean export directory name (remove leading dash)
    let export_project_name = clean_project_name_for_export(&claude_project_name);

    println!("📂 Current project: {}", export_project_name);

    // Look for the matching project in the Claude Code projects directory (using original name)
    let project_dir = projects_dir.join(&claude_project_name);

    if !project_dir.exists() {
        println!("⚠️  No Claude Code sessions found for current project");
        println!("   Expected: {}", project_dir.display());
        println!("   Tip: Make sure you've used Claude Code in this directory");
        return Ok(());
    }

    // Create export directory if it doesn't exist
    if !export_dir.exists() {
        fs::create_dir_all(export_dir)?;
        println!("📁 Created export directory: {}", export_dir.display());
    }

    let project_export_dir = export_dir.join(&export_project_name);

    // Create project-specific export directory if needed
    if !project_export_dir.exists() {
        fs::create_dir_all(&project_export_dir)?;
    }

    let sessions = discover_sessions(&project_dir)?;
    let mut updated_count = 0;
    let mut skipped_count = 0;

    println!("📄 Found {} session(s) for this project", sessions.len());

    for session_id in sessions {
        let session_file = project_dir.join(format!("{}.jsonl", session_id));
        let export_file = project_export_dir.join(format!("{}.md", session_id));

        // Check if we need to update this export
        let should_export = if export_file.exists() {
            // Compare modification times
            let session_metadata = fs::metadata(&session_file)?;
            let export_metadata = fs::metadata(&export_file)?;

            let session_modified = session_metadata.modified()?;
            let export_modified = export_metadata.modified()?;

            session_modified > export_modified
        } else {
            // Export doesn't exist, so we need to create it
            true
        };

        if should_export {
            // Read and parse the session file
            let content = fs::read_to_string(&session_file)?;
            let entries = parse_log_entries(&content);

            // Generate markdown using the same function as the web export
            let markdown = cc_log_viewer::generate_markdown_export(
                &entries,
                &session_id,
                &export_project_name,
            );

            // Write the markdown file
            fs::write(&export_file, markdown)?;

            println!("    ✅ Updated {}.md", session_id);
            updated_count += 1;
        } else {
            println!("    ⏭️  Skipped {}.md (no changes)", session_id);
            skipped_count += 1;
        }
    }

    println!("\n📊 Export Summary:");
    println!("   Updated: {} files", updated_count);
    println!("   Skipped: {} files", skipped_count);
    println!("✅ Update export completed successfully!");

    Ok(())
}

async fn export_all_projects(
    projects_dir: &Path,
    export_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📚 Exporting all projects...");

    let projects = discover_projects(projects_dir)?;

    for project_name in projects {
        println!("📖 Exporting project: {}", project_name);
        export_project(projects_dir, export_dir, &project_name).await?;
    }

    Ok(())
}

async fn export_specific_projects(
    projects_dir: &Path,
    export_dir: &Path,
    project_names: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📚 Exporting {} project(s)...", project_names.len());

    for project_name in project_names {
        if project_exists(projects_dir, project_name) {
            println!("📖 Exporting project: {}", project_name);
            export_project(projects_dir, export_dir, project_name).await?;
        } else {
            eprintln!(
                "⚠️  Warning: Project '{}' not found, skipping",
                project_name
            );
        }
    }

    Ok(())
}

async fn export_project(
    projects_dir: &Path,
    export_dir: &Path,
    project_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = projects_dir.join(project_name);
    let clean_project_name = clean_project_name_for_export(project_name);
    let project_export_dir = export_dir.join(&clean_project_name);

    // Create project-specific export directory
    if !project_export_dir.exists() {
        fs::create_dir_all(&project_export_dir)?;
    }

    // Discover all session files in the project
    let sessions = discover_sessions(&project_dir)?;

    println!("  📄 Found {} session(s)", sessions.len());

    for session_id in sessions {
        let session_file = project_dir.join(format!("{}.jsonl", session_id));
        let export_file = project_export_dir.join(format!("{}.md", session_id));

        // Read and parse the session file
        let content = fs::read_to_string(&session_file)?;
        let entries = parse_log_entries(&content);

        // Generate markdown using the same function as the web export
        let markdown =
            cc_log_viewer::generate_markdown_export(&entries, &session_id, &clean_project_name);

        // Write the markdown file
        fs::write(&export_file, markdown)?;

        println!("    ✅ {}.md", session_id);
    }

    Ok(())
}

fn discover_projects(projects_dir: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut projects = Vec::new();

    for entry in fs::read_dir(projects_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                projects.push(name.to_string());
            }
        }
    }

    projects.sort();
    Ok(projects)
}

fn discover_sessions(project_dir: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut sessions = Vec::new();

    for entry in fs::read_dir(project_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".jsonl") {
                    // Remove .jsonl extension to get session ID
                    let session_id = name.trim_end_matches(".jsonl");
                    sessions.push(session_id.to_string());
                }
            }
        }
    }

    sessions.sort();
    Ok(sessions)
}

fn project_exists(projects_dir: &Path, project_name: &str) -> bool {
    projects_dir.join(project_name).exists()
}

fn parse_log_entries(content: &str) -> Vec<cc_log_viewer::LogEntry> {
    let mut entries = Vec::new();

    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<cc_log_viewer::LogEntry>(line) {
            entries.push(entry);
        }
    }

    entries
}
