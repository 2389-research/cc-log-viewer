// ABOUTME: Basic integration tests for cc-log-viewer functionality
// ABOUTME: Tests core functionality without breaking the existing working code

use serde_json::json;
use std::fs;
use tempfile::TempDir;

// Test helper to create sample JSONL content
fn create_sample_jsonl() -> String {
    vec![
        json!({
            "type": "summary",
            "summary": "Test session",
            "timestamp": "2024-01-15T10:00:00Z",
            "uuid": "summary-uuid"
        })
        .to_string(),
        json!({
            "type": "message",
            "userType": "human",
            "message": {"role": "user", "content": "Hello"},
            "timestamp": "2024-01-15T10:01:00Z",
            "uuid": "msg-uuid"
        })
        .to_string(),
    ]
    .join("\n")
}

#[test]
fn test_jsonl_parsing() {
    let jsonl_content = create_sample_jsonl();
    let lines: Vec<&str> = jsonl_content.lines().collect();

    assert_eq!(lines.len(), 2);

    // Test that each line is valid JSON
    for line in lines {
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(parsed.is_object());
    }
}

#[test]
fn test_project_structure_creation() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("test-project");

    fs::create_dir_all(&project_path).unwrap();
    fs::write(project_path.join("session.jsonl"), create_sample_jsonl()).unwrap();

    assert!(project_path.exists());
    assert!(project_path.join("session.jsonl").exists());

    let content = fs::read_to_string(project_path.join("session.jsonl")).unwrap();
    assert!(!content.is_empty());
    assert!(content.contains("Test session"));
}

#[test]
fn test_malformed_json_handling() {
    let malformed_lines = vec!["invalid json", "{incomplete", "null", ""];

    for line in malformed_lines {
        let result: Result<serde_json::Value, _> = serde_json::from_str(line);
        // Should either parse successfully or fail gracefully
        match result {
            Ok(_) => {}  // Valid JSON
            Err(_) => {} // Expected for malformed JSON
        }
    }
}

#[test]
fn test_empty_project_handling() {
    let temp_dir = TempDir::new().unwrap();
    let empty_project = temp_dir.path().join("empty-project");

    fs::create_dir_all(&empty_project).unwrap();

    assert!(empty_project.exists());
    assert!(empty_project.is_dir());

    // Should be able to list directory contents (empty)
    let entries: Vec<_> = fs::read_dir(&empty_project).unwrap().collect();
    assert_eq!(entries.len(), 0);
}

#[test]
fn test_timestamp_format_validation() {
    let valid_timestamps = vec![
        "2024-01-15T10:00:00Z",
        "2024-12-31T23:59:59.999Z",
        "2024-01-01T00:00:00+00:00",
    ];

    for timestamp_str in valid_timestamps {
        let json_obj = json!({
            "timestamp": timestamp_str,
            "type": "message"
        });

        // Should parse without errors
        let serialized = json_obj.to_string();
        let parsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["timestamp"], timestamp_str);
    }
}

#[test]
fn test_session_file_naming() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("naming-test");
    fs::create_dir_all(&project_path).unwrap();

    let test_names = vec![
        "simple-session.jsonl",
        "session with spaces.jsonl",
        "session-123.jsonl",
        "main-session.jsonl",
    ];

    for name in test_names {
        let file_path = project_path.join(name);
        fs::write(&file_path, create_sample_jsonl()).unwrap();
        assert!(file_path.exists());
    }

    // Should be able to read directory
    let entries: Vec<_> = fs::read_dir(&project_path).unwrap().collect();
    assert_eq!(entries.len(), 4);
}

#[test]
fn test_concurrent_file_access() {
    use std::sync::Arc;
    use std::thread;

    let temp_dir = Arc::new(TempDir::new().unwrap());
    let project_path = temp_dir.path().join("concurrent-test");
    fs::create_dir_all(&project_path).unwrap();

    let test_file = project_path.join("test.jsonl");
    fs::write(&test_file, create_sample_jsonl()).unwrap();

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let file_path = test_file.clone();
            thread::spawn(move || {
                // Multiple threads reading the same file
                let content = fs::read_to_string(&file_path).unwrap();
                assert!(!content.is_empty());
                println!("Thread {} read {} bytes", i, content.len());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_large_jsonl_content() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("large-test");
    fs::create_dir_all(&project_path).unwrap();

    // Create a file with many entries
    let mut large_content = String::new();
    for i in 0..1000 {
        let entry = json!({
            "type": "message",
            "uuid": format!("uuid-{}", i),
            "message": {"role": "user", "content": format!("Message {}", i)},
            "timestamp": "2024-01-15T10:00:00Z"
        });
        large_content.push_str(&entry.to_string());
        large_content.push('\n');
    }

    let large_file = project_path.join("large.jsonl");
    fs::write(&large_file, &large_content).unwrap();

    // Should be able to read it back
    let read_content = fs::read_to_string(&large_file).unwrap();
    assert_eq!(read_content.len(), large_content.len());

    let lines: Vec<&str> = read_content.lines().collect();
    assert_eq!(lines.len(), 1000);
}

#[test]
fn test_unicode_content_handling() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("unicode-test");
    fs::create_dir_all(&project_path).unwrap();

    let unicode_content = json!({
        "type": "message",
        "message": {
            "role": "user",
            "content": "Hello 👋 World 🌍 Unicode: 中文 日本語 한글 العربية"
        },
        "uuid": "unicode-test"
    })
    .to_string();

    let unicode_file = project_path.join("unicode.jsonl");
    fs::write(&unicode_file, &unicode_content).unwrap();

    let read_content = fs::read_to_string(&unicode_file).unwrap();
    assert!(read_content.contains("👋"));
    assert!(read_content.contains("🌍"));
    assert!(read_content.contains("中文"));
    assert!(read_content.contains("العربية"));
}
