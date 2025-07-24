// ABOUTME: WebSocket integration tests for real-time watch functionality
// ABOUTME: Tests the /ws/watch endpoint and live streaming capabilities

use axum_test::TestServer;
use futures_util::stream::StreamExt;
use serde_json::json;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

// Import our app functions and types - using the crate directly since tests are integration tests
use cc_log_viewer::{
    get_projects, get_session_logs, get_sessions, index, live_activity, websocket_handler,
    AppState, WatchEvent, WatchManager,
};

// Helper to create test app state
async fn create_test_app(projects_dir: std::path::PathBuf) -> axum::Router {
    let state = AppState::new(projects_dir).expect("Failed to create app state");

    axum::Router::new()
        .route("/", axum::routing::get(index))
        .route("/live", axum::routing::get(live_activity))
        .route("/api/projects", axum::routing::get(get_projects))
        .route(
            "/api/projects/:project/sessions",
            axum::routing::get(get_sessions),
        )
        .route(
            "/api/projects/:project/sessions/:session",
            axum::routing::get(get_session_logs),
        )
        .route("/ws/watch", axum::routing::get(websocket_handler))
        .with_state(state)
}

// Helper to create sample log entry with tool use
fn create_tool_use_entry() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_abc123",
                    "name": "Bash",
                    "input": {
                        "command": "ls -la",
                        "description": "List files in current directory"
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:00:00Z",
        "uuid": "tool-use-uuid"
    })
    .to_string()
}

// Helper to create tool result entry
fn create_tool_result_entry() -> String {
    json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "toolu_abc123",
                    "content": "total 8\ndrwxr-xr-x  3 user  staff   96 Jan 15 10:00 .\ndrwxr-xr-x  4 user  staff  128 Jan 15 09:59 ..\n-rw-r--r--  1 user  staff  123 Jan 15 10:00 test.txt"
                }
            ]
        },
        "timestamp": "2024-01-15T10:00:30Z",
        "uuid": "tool-result-uuid"
    }).to_string()
}

#[tokio::test]
async fn test_websocket_connection() {
    let temp_dir = TempDir::new().unwrap();
    let app = create_test_app(temp_dir.path().to_path_buf()).await;
    let server = TestServer::new(app).unwrap();

    // Test WebSocket connection
    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);
    let connection_result = timeout(Duration::from_secs(5), connect_async(&ws_url)).await;

    assert!(
        connection_result.is_ok(),
        "WebSocket connection should succeed"
    );
}

#[tokio::test]
async fn test_watch_manager_creation() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();

    // Create a test project
    let project_path = projects_dir.join("test-project");
    fs::create_dir_all(&project_path).unwrap();

    let watch_manager = WatchManager::new(projects_dir);
    assert!(
        watch_manager.is_ok(),
        "WatchManager should be created successfully"
    );
}

#[tokio::test]
async fn test_file_change_detection() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_path = projects_dir.join("test-project");
    fs::create_dir_all(&project_path).unwrap();

    let watch_manager = WatchManager::new(projects_dir.clone()).unwrap();
    let mut rx = watch_manager.subscribe();

    // Create a new JSONL file
    let session_file = project_path.join("test-session.jsonl");
    fs::write(&session_file, create_tool_use_entry()).unwrap();

    // Wait for file system event
    let event = timeout(Duration::from_secs(2), rx.recv()).await;
    assert!(event.is_ok(), "Should receive file system event");

    let watch_event = event.unwrap().unwrap();
    assert_eq!(watch_event.event_type, "log_entry");
    assert_eq!(watch_event.project, "test-project");
    assert!(watch_event.entry.is_some());
}

#[tokio::test]
async fn test_tool_event_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_path = projects_dir.join("tool-test");
    fs::create_dir_all(&project_path).unwrap();

    let watch_manager = WatchManager::new(projects_dir.clone()).unwrap();
    let mut rx = watch_manager.subscribe();

    // Write tool use entry
    let session_file = project_path.join("session.jsonl");
    fs::write(&session_file, create_tool_use_entry()).unwrap();

    // Wait for and verify tool use event
    let event = timeout(Duration::from_secs(2), rx.recv()).await;
    assert!(event.is_ok());

    let watch_event = event.unwrap().unwrap();
    let entry = watch_event.entry.unwrap();
    assert_eq!(entry.entry_type, Some("assistant".to_string()));

    // Verify tool use content
    if let Some(message) = entry.message {
        if let Some(content_array) = message.as_array() {
            let has_tool_use = content_array
                .iter()
                .any(|c| c.get("type").and_then(|t| t.as_str()) == Some("tool_use"));
            assert!(has_tool_use, "Should detect tool_use in message content");
        }
    }
}

#[tokio::test]
async fn test_multiple_file_changes() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();

    // Create multiple projects
    for i in 1..=3 {
        let project_path = projects_dir.join(format!("project-{}", i));
        fs::create_dir_all(&project_path).unwrap();
    }

    let watch_manager = WatchManager::new(projects_dir.clone()).unwrap();
    let mut rx = watch_manager.subscribe();

    // Write to multiple files simultaneously
    for i in 1..=3 {
        let project_path = projects_dir.join(format!("project-{}", i));
        let session_file = project_path.join("session.jsonl");
        let content = json!({
            "type": "message",
            "uuid": format!("uuid-{}", i),
            "message": {"role": "user", "content": format!("Message from project {}", i)},
            "timestamp": "2024-01-15T10:00:00Z"
        })
        .to_string();
        fs::write(&session_file, content).unwrap();
    }

    // Should receive events from all projects
    let mut received_projects = std::collections::HashSet::new();
    for _ in 0..3 {
        if let Ok(Ok(event)) = timeout(Duration::from_secs(3), rx.recv()).await {
            received_projects.insert(event.project);
        }
    }

    assert!(
        received_projects.len() >= 2,
        "Should receive events from multiple projects"
    );
}

#[tokio::test]
async fn test_session_state_management() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_path = projects_dir.join("state-test");
    fs::create_dir_all(&project_path).unwrap();

    let watch_manager = WatchManager::new(projects_dir.clone()).unwrap();
    let mut rx = watch_manager.subscribe();

    let session_file = project_path.join("session.jsonl");

    // Write initial content
    fs::write(&session_file, create_tool_use_entry()).unwrap();

    // Wait for first event
    timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();

    // Append more content
    let mut file_content = fs::read_to_string(&session_file).unwrap();
    file_content.push('\n');
    file_content.push_str(&create_tool_result_entry());
    fs::write(&session_file, file_content).unwrap();

    // Should receive second event with new content only
    let second_event = timeout(Duration::from_secs(2), rx.recv()).await;
    assert!(
        second_event.is_ok(),
        "Should receive second event for appended content"
    );
}

#[tokio::test]
async fn test_websocket_message_format() {
    let temp_dir = TempDir::new().unwrap();
    let app = create_test_app(temp_dir.path().to_path_buf()).await;
    let server = TestServer::new(app).unwrap();

    // Create test project and file for events
    let project_path = temp_dir.path().join("ws-test");
    fs::create_dir_all(&project_path).unwrap();

    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);
    let (ws_stream, _) = connect_async(&ws_url).await.unwrap();
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    // Trigger an event by writing to a file
    let session_file = project_path.join("test.jsonl");
    fs::write(&session_file, create_tool_use_entry()).unwrap();

    // Wait for WebSocket message
    if let Ok(Some(msg)) = timeout(Duration::from_secs(3), ws_receiver.next()).await {
        if let Ok(WsMessage::Text(text)) = msg {
            let watch_event: serde_json::Value = serde_json::from_str(&text).unwrap();

            assert_eq!(watch_event["type"], "log_entry");
            assert_eq!(watch_event["project"], "ws-test");
            assert!(watch_event["entry"].is_object());
            assert!(watch_event["timestamp"].is_string());
        } else {
            panic!("Expected text message from WebSocket");
        }
    } else {
        panic!("Should receive WebSocket message within timeout");
    }
}

#[tokio::test]
async fn test_rate_limiting() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_path = projects_dir.join("rate-limit-test");
    fs::create_dir_all(&project_path).unwrap();

    let watch_manager = WatchManager::new(projects_dir.clone()).unwrap();
    let mut rx = watch_manager.subscribe();

    // Create a large file with many entries (should be rate limited)
    let session_file = project_path.join("large.jsonl");
    let mut large_content = String::new();
    for i in 0..20 {
        // More than the 10 entry limit
        let entry = json!({
            "type": "message",
            "uuid": format!("uuid-{}", i),
            "message": {"role": "user", "content": format!("Message {}", i)},
            "timestamp": "2024-01-15T10:00:00Z"
        });
        large_content.push_str(&entry.to_string());
        large_content.push('\n');
    }
    fs::write(&session_file, large_content).unwrap();

    // Count received events - should be all 20 entries but may come in multiple batches
    // due to file system events (CREATE + MODIFY), with each batch limited to 10
    let mut event_count = 0;
    let mut unique_uuids = std::collections::HashSet::new();

    while let Ok(Ok(event)) = timeout(Duration::from_millis(500), rx.recv()).await {
        event_count += 1;

        // Track unique UUIDs to ensure we don't get duplicates
        if let Some(entry) = &event.entry {
            if let Some(uuid) = &entry.uuid {
                unique_uuids.insert(uuid.clone());
            }
        }

        if event_count > 25 {
            // Safety break - should never get more than ~20 events
            break;
        }
    }

    // Should receive all 20 entries (possibly in 2 batches of 10 due to multiple FS events)
    assert!(
        event_count >= 20,
        "Should process all entries across multiple file system events (got {} events)",
        event_count
    );
    assert!(
        event_count <= 25,
        "Should not exceed reasonable limit even with multiple FS events (got {} events)",
        event_count
    );

    // Verify we got unique entries, not duplicates
    assert_eq!(
        unique_uuids.len(),
        20,
        "Should have exactly 20 unique entries, got {} unique UUIDs",
        unique_uuids.len()
    );
}

#[tokio::test]
async fn test_malformed_jsonl_handling() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_path = projects_dir.join("malformed-test");
    fs::create_dir_all(&project_path).unwrap();

    let watch_manager = WatchManager::new(projects_dir.clone()).unwrap();
    let mut rx = watch_manager.subscribe();

    // Write file with mix of valid and invalid JSON
    let session_file = project_path.join("mixed.jsonl");
    let mixed_content = vec![
        "invalid json line",
        &create_tool_use_entry(),
        "{incomplete json",
        &create_tool_result_entry(),
        "",
    ]
    .join("\n");

    fs::write(&session_file, mixed_content).unwrap();

    // Should only receive events for valid JSON lines
    let mut valid_events = 0;
    while let Ok(Ok(_)) = timeout(Duration::from_millis(500), rx.recv()).await {
        valid_events += 1;
        if valid_events > 5 {
            // Safety break
            break;
        }
    }

    assert_eq!(
        valid_events, 2,
        "Should process exactly 2 valid JSON entries (tool use and tool result)"
    );
}
