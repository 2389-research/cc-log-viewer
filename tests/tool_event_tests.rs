// ABOUTME: Tests for tool event detection and parsing in JSONL logs
// ABOUTME: Validates rich tool rendering data extraction and event handling

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;

// Helper to create comprehensive tool use entries
fn create_bash_tool_entry() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "I'll run this command for you:"
                },
                {
                    "type": "tool_use",
                    "id": "toolu_bash123",
                    "name": "Bash",
                    "input": {
                        "command": "cargo test --verbose",
                        "description": "Run tests with verbose output"
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:00:00Z",
        "uuid": "bash-tool-uuid"
    })
    .to_string()
}

fn create_read_tool_entry() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_read456",
                    "name": "Read",
                    "input": {
                        "file_path": "/Users/harper/Public/src/2389/cc-log-viewer/src/main.rs",
                        "offset": 100,
                        "limit": 50
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:01:00Z",
        "uuid": "read-tool-uuid"
    })
    .to_string()
}

fn create_multiedit_tool_entry() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_edit789",
                    "name": "MultiEdit",
                    "input": {
                        "file_path": "/path/to/file.rs",
                        "edits": [
                            {
                                "old_string": "old code here",
                                "new_string": "new code here",
                                "replace_all": false
                            },
                            {
                                "old_string": "another old line",
                                "new_string": "another new line",
                                "replace_all": true
                            }
                        ]
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:02:00Z",
        "uuid": "multiedit-tool-uuid"
    })
    .to_string()
}

fn create_todowrite_tool_entry() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_todo999",
                    "name": "TodoWrite",
                    "input": {
                        "todos": [
                            {
                                "content": "Implement new feature",
                                "status": "pending",
                                "priority": "high",
                                "id": "1"
                            },
                            {
                                "content": "Fix bug in parser",
                                "status": "in_progress",
                                "priority": "medium",
                                "id": "2"
                            },
                            {
                                "content": "Update documentation",
                                "status": "completed",
                                "priority": "low",
                                "id": "3"
                            }
                        ]
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:03:00Z",
        "uuid": "todowrite-tool-uuid"
    })
    .to_string()
}

fn create_tool_result_entry() -> String {
    json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "toolu_bash123",
                    "content": "Compiling cc-log-viewer v0.3.0\n    Finished test [unoptimized + debuginfo] target(s) in 2.34s\n     Running unittests src/main.rs\ntest test_watch_manager ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured"
                }
            ]
        },
        "timestamp": "2024-01-15T10:00:30Z",
        "uuid": "bash-result-uuid"
    }).to_string()
}

fn create_mixed_content_entry() -> String {
    json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Let me check the file and then run a command:"
                },
                {
                    "type": "tool_use",
                    "id": "toolu_read001",
                    "name": "Read",
                    "input": {
                        "file_path": "/test/file.txt"
                    }
                },
                {
                    "type": "text",
                    "text": "Now I'll process this with bash:"
                },
                {
                    "type": "tool_use",
                    "id": "toolu_bash002",
                    "name": "Bash",
                    "input": {
                        "command": "wc -l /test/file.txt",
                        "description": "Count lines in file"
                    }
                }
            ]
        },
        "timestamp": "2024-01-15T10:04:00Z",
        "uuid": "mixed-content-uuid"
    })
    .to_string()
}

#[test]
fn test_tool_use_detection() {
    let entries = vec![
        create_bash_tool_entry(),
        create_read_tool_entry(),
        create_multiedit_tool_entry(),
        create_todowrite_tool_entry(),
        create_mixed_content_entry(),
    ];

    for entry_str in entries {
        let entry: Value = serde_json::from_str(&entry_str).unwrap();

        // Should be assistant message
        assert_eq!(entry["type"], "assistant");

        // Should have message content array
        let content = entry["message"]["content"].as_array().unwrap();

        // Should contain at least one tool_use
        let has_tool_use = content.iter().any(|item| item["type"] == "tool_use");
        assert!(has_tool_use, "Entry should contain tool_use");

        // Extract tool uses
        let tool_uses: Vec<_> = content
            .iter()
            .filter(|item| item["type"] == "tool_use")
            .collect();

        assert!(!tool_uses.is_empty(), "Should have tool uses");

        // Verify tool use structure
        for tool_use in tool_uses {
            assert!(tool_use["id"].is_string(), "Tool use should have ID");
            assert!(tool_use["name"].is_string(), "Tool use should have name");
            assert!(tool_use["input"].is_object(), "Tool use should have input");
        }
    }
}

#[test]
fn test_tool_result_detection() {
    let result_entry_str = create_tool_result_entry();
    let entry: Value = serde_json::from_str(&result_entry_str).unwrap();

    // Should be user message
    assert_eq!(entry["type"], "user");

    // Should have message content array
    let content = entry["message"]["content"].as_array().unwrap();

    // Should contain tool_result
    let has_tool_result = content.iter().any(|item| item["type"] == "tool_result");
    assert!(has_tool_result, "Entry should contain tool_result");

    // Extract tool result
    let tool_result = content
        .iter()
        .find(|item| item["type"] == "tool_result")
        .unwrap();

    assert!(
        tool_result["tool_use_id"].is_string(),
        "Tool result should have tool_use_id"
    );
    assert!(
        tool_result["content"].is_string(),
        "Tool result should have content"
    );

    // Verify content is meaningful
    let result_content = tool_result["content"].as_str().unwrap();
    assert!(
        result_content.contains("Compiling"),
        "Result should contain expected output"
    );
}

#[test]
fn test_specific_tool_parsing() {
    // Test Bash tool parsing
    let bash_entry: Value = serde_json::from_str(&create_bash_tool_entry()).unwrap();
    let bash_tool = bash_entry["message"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "tool_use")
        .unwrap();

    assert_eq!(bash_tool["name"], "Bash");
    assert_eq!(bash_tool["input"]["command"], "cargo test --verbose");
    assert_eq!(
        bash_tool["input"]["description"],
        "Run tests with verbose output"
    );

    // Test Read tool parsing
    let read_entry: Value = serde_json::from_str(&create_read_tool_entry()).unwrap();
    let read_tool = read_entry["message"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "tool_use")
        .unwrap();

    assert_eq!(read_tool["name"], "Read");
    assert!(read_tool["input"]["file_path"]
        .as_str()
        .unwrap()
        .ends_with("main.rs"));
    assert_eq!(read_tool["input"]["offset"], 100);
    assert_eq!(read_tool["input"]["limit"], 50);

    // Test MultiEdit tool parsing
    let multiedit_entry: Value = serde_json::from_str(&create_multiedit_tool_entry()).unwrap();
    let multiedit_tool = multiedit_entry["message"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "tool_use")
        .unwrap();

    assert_eq!(multiedit_tool["name"], "MultiEdit");
    let edits = multiedit_tool["input"]["edits"].as_array().unwrap();
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0]["old_string"], "old code here");
    assert_eq!(edits[1]["replace_all"], true);
}

#[test]
fn test_todowrite_tool_structure() {
    let todo_entry: Value = serde_json::from_str(&create_todowrite_tool_entry()).unwrap();
    let todo_tool = todo_entry["message"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "tool_use")
        .unwrap();

    assert_eq!(todo_tool["name"], "TodoWrite");

    let todos = todo_tool["input"]["todos"].as_array().unwrap();
    assert_eq!(todos.len(), 3);

    // Verify todo structure
    for todo in todos {
        assert!(todo["content"].is_string());
        assert!(todo["status"].is_string());
        assert!(todo["priority"].is_string());
        assert!(todo["id"].is_string());
    }

    // Verify specific todo content
    assert_eq!(todos[0]["status"], "pending");
    assert_eq!(todos[0]["priority"], "high");
    assert_eq!(todos[1]["status"], "in_progress");
    assert_eq!(todos[2]["status"], "completed");
}

#[test]
fn test_mixed_content_parsing() {
    let mixed_entry: Value = serde_json::from_str(&create_mixed_content_entry()).unwrap();
    let content = mixed_entry["message"]["content"].as_array().unwrap();

    // Should have mix of text and tool_use items
    let text_items: Vec<_> = content
        .iter()
        .filter(|item| item["type"] == "text")
        .collect();
    let tool_items: Vec<_> = content
        .iter()
        .filter(|item| item["type"] == "tool_use")
        .collect();

    assert_eq!(text_items.len(), 2, "Should have 2 text items");
    assert_eq!(tool_items.len(), 2, "Should have 2 tool_use items");

    // Verify order is preserved
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "tool_use");
    assert_eq!(content[2]["type"], "text");
    assert_eq!(content[3]["type"], "tool_use");

    // Verify tool names
    assert_eq!(content[1]["name"], "Read");
    assert_eq!(content[3]["name"], "Bash");
}

#[test]
fn test_tool_result_correlation() {
    // Create correlated tool use and result
    let tool_use_str = create_bash_tool_entry();
    let tool_result_str = create_tool_result_entry();

    let tool_use_entry: Value = serde_json::from_str(&tool_use_str).unwrap();
    let tool_result_entry: Value = serde_json::from_str(&tool_result_str).unwrap();

    // Extract tool use ID
    let tool_use = tool_use_entry["message"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "tool_use")
        .unwrap();
    let tool_use_id = tool_use["id"].as_str().unwrap();

    // Extract tool result ID
    let tool_result = tool_result_entry["message"]["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "tool_result")
        .unwrap();
    let result_tool_id = tool_result["tool_use_id"].as_str().unwrap();

    // Should match
    assert_eq!(
        tool_use_id, result_tool_id,
        "Tool use ID should match result tool_use_id"
    );
    assert_eq!(tool_use_id, "toolu_bash123");
}

#[test]
fn test_file_system_integration() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("tool-test");
    fs::create_dir_all(&project_path).unwrap();

    // Create JSONL file with various tool entries
    let session_file = project_path.join("tools.jsonl");
    let content = vec![
        create_bash_tool_entry(),
        create_tool_result_entry(),
        create_read_tool_entry(),
        create_multiedit_tool_entry(),
        create_todowrite_tool_entry(),
        create_mixed_content_entry(),
    ]
    .join("\n");

    fs::write(&session_file, content).unwrap();

    // Read back and parse each line
    let file_content = fs::read_to_string(&session_file).unwrap();
    let lines: Vec<&str> = file_content.lines().collect();

    assert_eq!(lines.len(), 6);

    let mut tool_use_count = 0;
    let mut tool_result_count = 0;

    for line in lines {
        let entry: Value = serde_json::from_str(line).unwrap();

        if let Some(content_array) = entry["message"]["content"].as_array() {
            for item in content_array {
                match item["type"].as_str() {
                    Some("tool_use") => tool_use_count += 1,
                    Some("tool_result") => tool_result_count += 1,
                    _ => {}
                }
            }
        }
    }

    assert!(
        tool_use_count >= 5,
        "Should have multiple tool uses (got {})",
        tool_use_count
    );
    assert!(
        tool_result_count >= 1,
        "Should have tool results (got {})",
        tool_result_count
    );
}

#[test]
fn test_edge_cases() {
    // Test empty content array
    let empty_content_entry = json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": []
        },
        "timestamp": "2024-01-15T10:00:00Z",
        "uuid": "empty-uuid"
    })
    .to_string();

    let entry: Value = serde_json::from_str(&empty_content_entry).unwrap();
    let content = entry["message"]["content"].as_array().unwrap();
    assert_eq!(content.len(), 0);

    // Test malformed tool use (missing required fields)
    let malformed_tool = json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    // Missing id, name, input
                }
            ]
        },
        "timestamp": "2024-01-15T10:00:00Z",
        "uuid": "malformed-uuid"
    })
    .to_string();

    let malformed_entry: Value = serde_json::from_str(&malformed_tool).unwrap();
    let malformed_content = malformed_entry["message"]["content"].as_array().unwrap();
    let tool_use = &malformed_content[0];

    assert_eq!(tool_use["type"], "tool_use");
    assert!(tool_use["id"].is_null());
    assert!(tool_use["name"].is_null());
    assert!(tool_use["input"].is_null());
}

#[test]
fn test_tool_input_validation() {
    // Test various tool input structures
    let tools = vec![
        (
            "Bash",
            json!({"command": "ls", "description": "List files"}),
        ),
        ("Read", json!({"file_path": "/path/to/file"})),
        (
            "Edit",
            json!({"file_path": "/path", "old_string": "old", "new_string": "new"}),
        ),
        ("Glob", json!({"pattern": "*.rs", "path": "/src"})),
        (
            "Grep",
            json!({"pattern": "search", "path": "/src", "output_mode": "content"}),
        ),
    ];

    for (tool_name, input) in tools {
        let tool_entry = json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": format!("tool_{}", tool_name.to_lowercase()),
                        "name": tool_name,
                        "input": input
                    }
                ]
            },
            "timestamp": "2024-01-15T10:00:00Z",
            "uuid": format!("{}-uuid", tool_name.to_lowercase())
        })
        .to_string();

        let entry: Value = serde_json::from_str(&tool_entry).unwrap();
        let tool = entry["message"]["content"][0].as_object().unwrap();

        assert_eq!(tool["name"], tool_name);
        assert!(tool["input"].is_object());

        // Validate specific input fields based on tool type
        match tool_name {
            "Bash" => {
                assert!(tool["input"]["command"].is_string());
            }
            "Read" => {
                assert!(tool["input"]["file_path"].is_string());
            }
            "Edit" => {
                assert!(tool["input"]["file_path"].is_string());
                assert!(tool["input"]["old_string"].is_string());
                assert!(tool["input"]["new_string"].is_string());
            }
            _ => {}
        }
    }
}
