// ABOUTME: Terminal User Interface tests for cc-log-viewer
// ABOUTME: Tests TUI functionality, CLI argument parsing, and terminal interaction

use cc_log_viewer::{tui::TuiApp, AppState};
use serde_json::json;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

// Test helper to create sample project structure
fn create_test_project_structure(temp_dir: &TempDir) -> std::path::PathBuf {
    let projects_dir = temp_dir.path();
    let project_dir = projects_dir.join("test-project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create sample session files
    let session1_content = vec![
        json!({
            "type": "summary",
            "summary": "Test Session 1",
            "timestamp": "2024-01-15T10:00:00Z",
            "uuid": "summary-1"
        })
        .to_string(),
        json!({
            "type": "message",
            "userType": "human",
            "message": {"role": "user", "content": "Hello from session 1"},
            "timestamp": "2024-01-15T10:01:00Z",
            "uuid": "msg-1"
        })
        .to_string(),
        json!({
            "type": "message",
            "userType": "assistant",
            "message": {"role": "assistant", "content": "Hello back from session 1"},
            "timestamp": "2024-01-15T10:02:00Z",
            "uuid": "msg-2"
        })
        .to_string(),
    ]
    .join("\n");

    let session2_content = vec![
        json!({
            "type": "summary",
            "summary": "Test Session 2",
            "timestamp": "2024-01-15T11:00:00Z",
            "uuid": "summary-2"
        })
        .to_string(),
        json!({
            "type": "message",
            "userType": "human",
            "message": {"role": "user", "content": "Hello from session 2"},
            "timestamp": "2024-01-15T11:01:00Z",
            "uuid": "msg-3"
        })
        .to_string(),
    ]
    .join("\n");

    fs::write(project_dir.join("session1.jsonl"), session1_content).unwrap();
    fs::write(project_dir.join("session2.jsonl"), session2_content).unwrap();

    projects_dir.to_path_buf()
}

#[tokio::test]
async fn test_tui_app_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = create_test_project_structure(&temp_dir);

    let app_state = AppState::new(projects_dir).unwrap();
    let tui_app = TuiApp::new(app_state);

    // TUI should initialize in ProjectList mode
    assert_eq!(tui_app.mode, cc_log_viewer::tui::AppMode::ProjectList);
    assert!(tui_app.projects.is_empty()); // Not loaded until refresh
    assert!(tui_app.sessions.is_empty());
    assert!(tui_app.conversation.is_empty());
}

#[tokio::test]
async fn test_project_loading() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = create_test_project_structure(&temp_dir);

    let app_state = AppState::new(projects_dir).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    // Simulate loading projects (normally done in refresh_projects)
    tui_app.refresh_projects().await.unwrap();

    assert!(!tui_app.projects.is_empty());
    assert_eq!(tui_app.projects[0].name, "test-project");
    assert_eq!(tui_app.projects[0].session_count, 2);
}

#[tokio::test]
async fn test_session_loading() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = create_test_project_structure(&temp_dir);

    let app_state = AppState::new(projects_dir).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    // Load projects first
    tui_app.refresh_projects().await.unwrap();

    // Load sessions for the first project
    let project_name = &tui_app.projects[0].name.clone();
    tui_app.refresh_sessions(project_name).await.unwrap();

    assert_eq!(tui_app.sessions.len(), 2);
    assert!(tui_app
        .sessions
        .iter()
        .any(|s| s.summary == "Test Session 1"));
    assert!(tui_app
        .sessions
        .iter()
        .any(|s| s.summary == "Test Session 2"));
}

#[tokio::test]
async fn test_conversation_loading() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = create_test_project_structure(&temp_dir);

    let app_state = AppState::new(projects_dir).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    // Load projects and sessions
    tui_app.refresh_projects().await.unwrap();
    let project_name = tui_app.projects[0].name.clone();
    tui_app.refresh_sessions(&project_name).await.unwrap();

    // Load conversation for first session
    let session_id = &tui_app.sessions[0].id.clone();
    tui_app
        .refresh_conversation(&project_name, session_id)
        .await
        .unwrap();

    // The summary entry may not be included in conversation parsing
    assert!(tui_app.conversation.len() >= 2); // At least 2 messages
    assert!(tui_app.conversation.iter().any(|entry| {
        entry
            .message
            .as_ref()
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.contains("Hello from session"))
            .unwrap_or(false)
    }));
}

#[tokio::test]
async fn test_export_functionality() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = create_test_project_structure(&temp_dir);

    let app_state = AppState::new(projects_dir).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    // Load data
    tui_app.refresh_projects().await.unwrap();
    let project_name = tui_app.projects[0].name.clone();
    tui_app.refresh_sessions(&project_name).await.unwrap();
    let session_id = tui_app.sessions[0].id.clone();
    tui_app
        .refresh_conversation(&project_name, &session_id)
        .await
        .unwrap();

    // Test export
    tui_app.export_conversation().await.unwrap();

    // Check that export file was created
    assert!(std::path::Path::new("conversation_export.txt").exists());

    let exported_content = fs::read_to_string("conversation_export.txt").unwrap();
    assert!(exported_content.contains("Claude Code Conversation Export"));
    assert!(exported_content.contains("Hello from session"));

    // Cleanup
    fs::remove_file("conversation_export.txt").unwrap();
}

#[tokio::test]
async fn test_empty_project_handling() {
    let temp_dir = TempDir::new().unwrap();
    let empty_projects_dir = temp_dir.path().to_path_buf();

    let app_state = AppState::new(empty_projects_dir).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    tui_app.refresh_projects().await.unwrap();

    assert!(tui_app.projects.is_empty());
    assert!(tui_app.status_message.contains("No projects found"));
}

#[tokio::test]
async fn test_malformed_session_handling() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path();
    let project_dir = projects_dir.join("malformed-project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create session with malformed JSON
    let malformed_content = "invalid json line\n{incomplete\n}";
    fs::write(project_dir.join("malformed.jsonl"), malformed_content).unwrap();

    let app_state = AppState::new(projects_dir.to_path_buf()).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    // Should handle malformed content gracefully
    tui_app.refresh_projects().await.unwrap();
    tui_app.refresh_sessions("malformed-project").await.unwrap();

    // Should create session entry even with malformed content
    assert_eq!(tui_app.sessions.len(), 1);
}

#[tokio::test]
async fn test_nonexistent_project_handling() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();

    let app_state = AppState::new(projects_dir).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    // Try to refresh sessions for non-existent project
    let result = tui_app.refresh_sessions("nonexistent-project").await;
    assert!(result.is_ok()); // Should handle gracefully
    assert!(tui_app.status_message.contains("not found"));
}

#[tokio::test]
async fn test_nonexistent_session_handling() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = create_test_project_structure(&temp_dir);

    let app_state = AppState::new(projects_dir).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    tui_app.refresh_projects().await.unwrap();
    let project_name = tui_app.projects[0].name.clone();

    // Try to load non-existent session
    let result = tui_app
        .refresh_conversation(&project_name, "nonexistent-session")
        .await;
    assert!(result.is_ok()); // Should handle gracefully
    assert!(tui_app.status_message.contains("not found"));
}

#[tokio::test]
async fn test_unicode_content_handling() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path();
    let project_dir = projects_dir.join("unicode-project");
    fs::create_dir_all(&project_dir).unwrap();

    let unicode_content = json!({
        "type": "message",
        "userType": "human",
        "message": {
            "role": "user",
            "content": "Unicode test: 👋 🌍 中文 日本語 한글 العربية"
        },
        "timestamp": "2024-01-15T10:00:00Z",
        "uuid": "unicode-test"
    })
    .to_string();

    fs::write(project_dir.join("unicode.jsonl"), unicode_content).unwrap();

    let app_state = AppState::new(projects_dir.to_path_buf()).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    tui_app.refresh_projects().await.unwrap();
    tui_app.refresh_sessions("unicode-project").await.unwrap();
    tui_app
        .refresh_conversation("unicode-project", "unicode")
        .await
        .unwrap();

    // Should handle unicode content properly
    assert_eq!(tui_app.conversation.len(), 1);
    let content = tui_app.conversation[0]
        .message
        .as_ref()
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap();

    assert!(content.contains("👋"));
    assert!(content.contains("🌍"));
    assert!(content.contains("中文"));
    assert!(content.contains("العربية"));
}

#[tokio::test]
async fn test_large_conversation_handling() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path();
    let project_dir = projects_dir.join("large-project");
    fs::create_dir_all(&project_dir).unwrap();

    // Create large conversation with many messages
    let mut large_content = String::new();
    for i in 0..1000 {
        let entry = json!({
            "type": "message",
            "userType": "human",
            "message": {
                "role": "user",
                "content": format!("Message number {}", i)
            },
            "timestamp": "2024-01-15T10:00:00Z",
            "uuid": format!("msg-{}", i)
        });
        large_content.push_str(&entry.to_string());
        large_content.push('\n');
    }

    fs::write(project_dir.join("large.jsonl"), large_content).unwrap();

    let app_state = AppState::new(projects_dir.to_path_buf()).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    tui_app.refresh_projects().await.unwrap();
    tui_app.refresh_sessions("large-project").await.unwrap();
    tui_app
        .refresh_conversation("large-project", "large")
        .await
        .unwrap();

    // Should handle large conversations
    assert_eq!(tui_app.conversation.len(), 1000);
}

#[test]
fn test_cli_argument_parsing() {
    use std::process::Command;

    // Test --tui flag is recognized
    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    let help_text = String::from_utf8(output.stdout).unwrap();
    assert!(help_text.contains("--tui"));
    assert!(help_text.contains("Use terminal UI instead of web interface"));
}

#[test]
fn test_cli_default_behavior() {
    use std::process::Command;

    // Test that help shows both modes
    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    let help_text = String::from_utf8(output.stdout).unwrap();
    assert!(help_text.contains("--port"));
    assert!(help_text.contains("--tui"));
    assert!(help_text.contains("[default: 2006]")); // Default port shown
}

#[tokio::test]
async fn test_tui_timeout_handling() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = create_test_project_structure(&temp_dir);

    let app_state = AppState::new(projects_dir).unwrap();
    let mut tui_app = TuiApp::new(app_state);

    // Test that TUI operations complete within reasonable time
    timeout(Duration::from_secs(5), async {
        tui_app.refresh_projects().await.unwrap();
        let project_name = tui_app.projects[0].name.clone();
        tui_app.refresh_sessions(&project_name).await.unwrap();
        let session_id = tui_app.sessions[0].id.clone();
        tui_app
            .refresh_conversation(&project_name, &session_id)
            .await
            .unwrap();
    })
    .await
    .expect("TUI operations should complete within timeout");
}
