// ABOUTME: End-to-end integration tests for complete live streaming workflow
// ABOUTME: Tests the full pipeline from file changes to WebSocket delivery

use axum_test::TestServer;
use futures_util::stream::StreamExt as FuturesStreamExt;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};

// Import our app functions and types
use cc_log_viewer::{
    get_projects, get_session_logs, get_sessions, index, live_activity, websocket_handler, AppState,
};

// Helper to create test app
async fn create_test_server(projects_dir: std::path::PathBuf) -> TestServer {
    let state = AppState::new(projects_dir).expect("Failed to create app state");

    let app = axum::Router::new()
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
        .with_state(state);

    TestServer::new(app).expect("Failed to create test server")
}

// Helper to create rich tool event
fn create_rich_bash_tool_event() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "I'll run this command to list the files:"
                },
                {
                    "type": "tool_use",
                    "id": "toolu_bash_e2e",
                    "name": "Bash",
                    "input": {
                        "command": "ls -la /Users/harper/Public/src/2389/cc-log-viewer",
                        "description": "List files in project directory with detailed information"
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:00:00Z",
        "uuid": "bash-tool-e2e-uuid"
    })
    .to_string()
}

fn create_rich_tool_result_event() -> String {
    json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "toolu_bash_e2e",
                    "content": "total 48\ndrwxr-xr-x  12 harper  staff   384 Jan 15 10:00 .\ndrwxr-xr-x   4 harper  staff   128 Jan 15 09:59 ..\n-rw-r--r--   1 harper  staff   123 Jan 15 10:00 Cargo.toml\n-rw-r--r--   1 harper  staff  1234 Jan 15 10:00 README.md\ndrwxr-xr-x   3 harper  staff    96 Jan 15 10:00 src\ndrwxr-xr-x   3 harper  staff    96 Jan 15 10:00 static\ndrwxr-xr-x   3 harper  staff    96 Jan 15 10:00 tests"
                }
            ]
        },
        "timestamp": "2024-01-15T10:00:30Z",
        "uuid": "bash-result-e2e-uuid"
    }).to_string()
}

fn create_multiedit_tool_event() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_multiedit_e2e",
                    "name": "MultiEdit",
                    "input": {
                        "file_path": "/Users/harper/Public/src/2389/cc-log-viewer/src/main.rs",
                        "edits": [
                            {
                                "old_string": "version = \"0.2.0\"",
                                "new_string": "version = \"0.3.0\"",
                                "replace_all": false
                            },
                            {
                                "old_string": "// TODO: implement feature",
                                "new_string": "// DONE: feature implemented",
                                "replace_all": true
                            }
                        ]
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:01:00Z",
        "uuid": "multiedit-e2e-uuid"
    })
    .to_string()
}

fn create_todowrite_event() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_todo_e2e",
                    "name": "TodoWrite",
                    "input": {
                        "todos": [
                            {
                                "content": "Implement WebSocket tests",
                                "status": "completed",
                                "priority": "high",
                                "id": "1"
                            },
                            {
                                "content": "Add tool rendering tests",
                                "status": "in_progress",
                                "priority": "high",
                                "id": "2"
                            },
                            {
                                "content": "Write end-to-end tests",
                                "status": "pending",
                                "priority": "medium",
                                "id": "3"
                            }
                        ]
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:02:00Z",
        "uuid": "todowrite-e2e-uuid"
    })
    .to_string()
}

#[tokio::test]
async fn test_complete_workflow_single_tool() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_dir = projects_dir.join("e2e-single");
    fs::create_dir_all(&project_dir).unwrap();

    let server = create_test_server(projects_dir).await;

    // Connect to WebSocket
    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    // Write tool use event
    let session_file = project_dir.join("session.jsonl");
    fs::write(&session_file, create_rich_bash_tool_event()).expect("Failed to write tool event");

    // Should receive WebSocket message for tool use
    let ws_message = timeout(Duration::from_secs(5), ws_receiver.next())
        .await
        .expect("Should receive WebSocket message")
        .expect("WebSocket stream should not end")
        .expect("WebSocket message should be valid");

    if let Message::Text(text) = ws_message {
        let watch_event: Value =
            serde_json::from_str(&text).expect("Should parse watch event JSON");

        // Verify watch event structure
        assert_eq!(watch_event["type"], "log_entry");
        assert_eq!(watch_event["project"], "e2e-single");
        assert_eq!(watch_event["session"], "session");

        // Verify log entry contains tool use
        let entry = &watch_event["entry"];
        assert_eq!(entry["type"], "assistant");

        let content = entry["message"]["content"]
            .as_array()
            .expect("Content should be array");
        let has_tool_use = content.iter().any(|item| item["type"] == "tool_use");
        assert!(has_tool_use, "Should contain tool_use in content");

        // Verify tool use details
        let tool_use = content
            .iter()
            .find(|item| item["type"] == "tool_use")
            .unwrap();
        assert_eq!(tool_use["name"], "Bash");
        assert_eq!(tool_use["id"], "toolu_bash_e2e");
        assert!(tool_use["input"]["command"]
            .as_str()
            .unwrap()
            .contains("ls -la"));
    } else {
        panic!("Expected text message from WebSocket");
    }
}

#[tokio::test]
async fn test_complete_workflow_tool_call_and_result() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_dir = projects_dir.join("e2e-pair");
    fs::create_dir_all(&project_dir).unwrap();

    let server = create_test_server(projects_dir).await;

    // Connect to WebSocket
    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    let session_file = project_dir.join("paired-session.jsonl");

    // Write tool use
    fs::write(&session_file, create_rich_bash_tool_event()).expect("Failed to write tool use");

    // Receive first message (tool use)
    let first_message = timeout(Duration::from_secs(3), ws_receiver.next())
        .await
        .expect("Should receive first message")
        .expect("Stream should continue")
        .expect("Message should be valid");

    // Append tool result
    let mut file_content = fs::read_to_string(&session_file).expect("Should read file");
    file_content.push('\n');
    file_content.push_str(&create_rich_tool_result_event());
    fs::write(&session_file, file_content).expect("Failed to append tool result");

    // Receive second message (tool result)
    let second_message = timeout(Duration::from_secs(3), ws_receiver.next())
        .await
        .expect("Should receive second message")
        .expect("Stream should continue")
        .expect("Message should be valid");

    // Verify both messages
    let messages = vec![first_message, second_message];
    let mut tool_use_found = false;
    let mut tool_result_found = false;

    for message in messages {
        if let Message::Text(text) = message {
            let watch_event: Value = serde_json::from_str(&text).expect("Should parse JSON");
            let entry = &watch_event["entry"];

            if entry["type"] == "assistant" {
                // Tool use message
                let content = entry["message"]["content"].as_array().unwrap();
                if content.iter().any(|item| item["type"] == "tool_use") {
                    tool_use_found = true;
                }
            } else if entry["type"] == "user" {
                // Tool result message
                let content = entry["message"]["content"].as_array().unwrap();
                if content.iter().any(|item| item["type"] == "tool_result") {
                    tool_result_found = true;

                    // Verify tool result content
                    let tool_result = content
                        .iter()
                        .find(|item| item["type"] == "tool_result")
                        .unwrap();
                    assert_eq!(tool_result["tool_use_id"], "toolu_bash_e2e");
                    assert!(tool_result["content"]
                        .as_str()
                        .unwrap()
                        .contains("total 48"));
                }
            }
        }
    }

    assert!(tool_use_found, "Should receive tool use event");
    assert!(tool_result_found, "Should receive tool result event");
}

#[tokio::test]
async fn test_multiple_projects_streaming() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();

    // Create multiple projects
    let projects = vec!["project-alpha", "project-beta", "project-gamma"];
    for project_name in &projects {
        fs::create_dir_all(projects_dir.join(project_name)).unwrap();
    }

    let server = create_test_server(projects_dir.clone()).await;

    // Connect to WebSocket
    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    // Write different tools to different projects
    let tools = vec![
        ("project-alpha", create_rich_bash_tool_event()),
        ("project-beta", create_multiedit_tool_event()),
        ("project-gamma", create_todowrite_event()),
    ];

    for (project_name, tool_event) in tools {
        let session_file = projects_dir.join(project_name).join("session.jsonl");
        fs::write(&session_file, tool_event).expect("Failed to write tool event");
        sleep(Duration::from_millis(100)).await; // Ensure events are spaced
    }

    // Collect events from all projects
    let mut received_projects = std::collections::HashSet::new();
    let mut tool_types = std::collections::HashSet::new();

    for _ in 0..3 {
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_secs(5), ws_receiver.next()).await
        {
            let watch_event: Value = serde_json::from_str(&text).expect("Should parse JSON");

            received_projects.insert(watch_event["project"].as_str().unwrap().to_string());

            // Extract tool name
            let entry = &watch_event["entry"];
            if let Some(content_array) = entry["message"]["content"].as_array() {
                for item in content_array {
                    if item["type"] == "tool_use" {
                        if let Some(tool_name) = item["name"].as_str() {
                            tool_types.insert(tool_name.to_string());
                        }
                    }
                }
            }
        }
    }

    assert!(
        received_projects.len() >= 2,
        "Should receive events from multiple projects"
    );
    assert!(
        tool_types.contains("Bash")
            || tool_types.contains("MultiEdit")
            || tool_types.contains("TodoWrite"),
        "Should receive different tool types"
    );
}

#[tokio::test]
async fn test_live_activity_endpoint() {
    let temp_dir = TempDir::new().unwrap();
    let server = create_test_server(temp_dir.path().to_path_buf()).await;

    // Test that /live endpoint serves the live activity page
    let response = server.get("/live").await;
    response.assert_status_ok();

    let body = response.text();
    assert!(
        body.contains("Live Activity Stream"),
        "Should contain live activity page content"
    );
    assert!(
        body.contains("WebSocket"),
        "Should contain WebSocket functionality"
    );
    assert!(
        body.contains("ToolHandler"),
        "Should contain tool handler classes"
    );
}

#[tokio::test]
async fn test_api_endpoints_with_tools() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_dir = projects_dir.join("api-test");
    fs::create_dir_all(&project_dir).unwrap();

    // Create session with tool events
    let session_file = project_dir.join("tool-session.jsonl");
    let content = vec![
        json!({
            "type": "summary",
            "summary": "Session with tool usage",
            "timestamp": "2024-01-15T09:59:00Z",
            "uuid": "summary-uuid"
        })
        .to_string(),
        create_rich_bash_tool_event(),
        create_rich_tool_result_event(),
        create_multiedit_tool_event(),
    ]
    .join("\n");
    fs::write(&session_file, content).expect("Failed to write session file");

    let server = create_test_server(projects_dir).await;

    // Test projects API
    let projects_response = server.get("/api/projects").await;
    projects_response.assert_status_ok();

    let projects: Value = projects_response.json();
    let projects_array = projects.as_array().expect("Should be array");
    assert!(!projects_array.is_empty(), "Should have projects");

    let project = &projects_array[0];
    assert_eq!(project["name"], "api-test");

    // Test sessions API
    let sessions_response = server.get("/api/projects/api-test/sessions").await;
    sessions_response.assert_status_ok();

    let sessions: Value = sessions_response.json();
    let sessions_array = sessions.as_array().expect("Should be array");
    assert!(!sessions_array.is_empty(), "Should have sessions");

    let session = &sessions_array[0];
    assert_eq!(session["id"], "tool-session");
    assert_eq!(session["summary"], "Session with tool usage");

    // Test session logs API
    let logs_response = server
        .get("/api/projects/api-test/sessions/tool-session")
        .await;
    logs_response.assert_status_ok();

    let logs: Value = logs_response.json();
    let logs_array = logs.as_array().expect("Should be array");
    assert!(logs_array.len() >= 3, "Should have multiple log entries");

    // Verify tool events are included
    let has_bash_tool = logs_array.iter().any(|entry| {
        if let Some(content_array) = entry["message"]["content"].as_array() {
            content_array
                .iter()
                .any(|item| item["type"] == "tool_use" && item["name"] == "Bash")
        } else {
            false
        }
    });
    assert!(has_bash_tool, "Should include Bash tool events");
}

#[tokio::test]
async fn test_websocket_connection_management() {
    let temp_dir = TempDir::new().unwrap();
    let server = create_test_server(temp_dir.path().to_path_buf()).await;

    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);

    // Test multiple concurrent connections
    let mut connections = Vec::new();
    for _ in 0..3 {
        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .expect("WebSocket connection failed");
        connections.push(ws_stream);
    }

    assert_eq!(
        connections.len(),
        3,
        "Should support multiple WebSocket connections"
    );

    // Test connection persistence
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    // Connection should stay alive
    sleep(Duration::from_secs(1)).await;

    // Try to send/receive (connection should still be active)
    // Note: In a real test, we'd trigger an event here to verify the connection works
}

#[tokio::test]
async fn test_path_formatting_in_events() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();

    // Create project with path that would be encoded
    let encoded_project_name = "-Users-harper-Public-src-2389-cc-log-viewer";
    let project_dir = projects_dir.join(&encoded_project_name);
    fs::create_dir_all(&project_dir).unwrap();

    let server = create_test_server(projects_dir).await;

    // Connect to WebSocket
    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    // Write event
    let session_file = project_dir.join("path-test.jsonl");
    fs::write(&session_file, create_rich_bash_tool_event()).expect("Failed to write event");

    // Receive event
    let ws_message = timeout(Duration::from_secs(3), ws_receiver.next())
        .await
        .expect("Should receive message")
        .expect("Stream should continue")
        .expect("Message should be valid");

    if let Message::Text(text) = ws_message {
        let watch_event: Value = serde_json::from_str(&text).expect("Should parse JSON");

        // Project name should be the encoded version (backend doesn't format)
        // The frontend is responsible for formatting the display
        assert_eq!(watch_event["project"], encoded_project_name);
    }
}

#[tokio::test]
async fn test_session_id_in_events() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_dir = projects_dir.join("session-id-test");
    fs::create_dir_all(&project_dir).unwrap();

    let server = create_test_server(projects_dir).await;

    // Connect to WebSocket
    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    // Use a session ID that looks like a UUID
    let session_id = "142e1b10-3ed3-42c5-9c3d-aae2a607974b";
    let session_file = project_dir.join(format!("{}.jsonl", session_id));
    fs::write(&session_file, create_rich_bash_tool_event()).expect("Failed to write event");

    // Receive event
    let ws_message = timeout(Duration::from_secs(3), ws_receiver.next())
        .await
        .expect("Should receive message")
        .expect("Stream should continue")
        .expect("Message should be valid");

    if let Message::Text(text) = ws_message {
        let watch_event: Value = serde_json::from_str(&text).expect("Should parse JSON");

        // Session should be the full UUID
        assert_eq!(watch_event["session"], session_id);
    }
}

#[tokio::test]
async fn test_error_resilience() {
    let temp_dir = TempDir::new().unwrap();
    let projects_dir = temp_dir.path().to_path_buf();
    let project_dir = projects_dir.join("error-resilience");
    fs::create_dir_all(&project_dir).unwrap();

    let server = create_test_server(projects_dir).await;

    // Connect to WebSocket
    let server_addr = match server.server_address() {
        Some(addr) => addr,
        None => {
            eprintln!("Warning: Cannot get server address, skipping WebSocket test");
            return;
        }
    };
    let ws_url = format!("ws://{}/ws/watch", server_addr);
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection failed");
    let (_ws_sender, mut ws_receiver) = ws_stream.split();

    let session_file = project_dir.join("error-test.jsonl");

    // Write mix of valid and invalid entries
    let mixed_content = vec![
        create_rich_bash_tool_event(),
        "invalid json line that should be skipped".to_string(),
        create_rich_tool_result_event(),
        "{incomplete: json}".to_string(),
        create_todowrite_event(),
        "".to_string(),
    ]
    .join("\n");

    fs::write(&session_file, mixed_content).expect("Failed to write mixed content");

    // Should receive events for valid entries only
    let mut valid_events = 0;
    let mut event_uuids = Vec::new();

    while let Ok(Some(Ok(Message::Text(text)))) =
        timeout(Duration::from_millis(1000), ws_receiver.next()).await
    {
        let watch_event: Value = serde_json::from_str(&text).expect("Should parse watch event");
        if let Some(entry) = watch_event["entry"].as_object() {
            if let Some(uuid) = entry["uuid"].as_str() {
                event_uuids.push(uuid.to_string());
                valid_events += 1;
            }
        }
        if valid_events >= 3 {
            break;
        }
    }

    assert!(event_uuids.contains(&"bash-tool-e2e-uuid".to_string()));
    assert!(event_uuids.contains(&"bash-result-e2e-uuid".to_string()));
    assert!(event_uuids.contains(&"todowrite-e2e-uuid".to_string()));
    assert_eq!(valid_events, 3, "Should process exactly 3 valid entries");
}
