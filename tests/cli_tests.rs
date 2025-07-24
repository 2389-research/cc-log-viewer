// ABOUTME: CLI argument parsing unit tests
// ABOUTME: Tests command-line interface structure and parsing logic

use clap::{CommandFactory, Parser};
use std::path::PathBuf;

// Replicate the CLI struct from main.rs for testing
#[derive(Parser, Debug)]
#[clap(name = "cc-log-viewer")]
#[clap(about = "Claude Code log viewer - Web interface for viewing conversation logs")]
struct TestCli {
    #[clap(
        help = "Path to projects directory containing log files (defaults to ~/.claude/projects/)"
    )]
    projects_dir: Option<PathBuf>,

    #[clap(short, long, default_value = "2006", help = "Port to serve on")]
    port: u16,

    #[clap(long, help = "Use terminal UI instead of web interface")]
    tui: bool,
}

#[test]
fn test_cli_tui_flag_parsing() {
    // Test --tui flag
    let cli = TestCli::try_parse_from(["cc-log-viewer", "--tui"]).unwrap();
    assert!(cli.tui);
    assert_eq!(cli.port, 2006); // Default port
    assert!(cli.projects_dir.is_none()); // No projects dir specified

    // Test without --tui flag (default is false)
    let cli_no_tui = TestCli::try_parse_from(["cc-log-viewer"]).unwrap();
    assert!(!cli_no_tui.tui);
}

#[test]
fn test_cli_port_flag_parsing() {
    // Test default port
    let cli_default = TestCli::try_parse_from(["cc-log-viewer"]).unwrap();
    assert_eq!(cli_default.port, 2006);

    // Test custom port with short flag
    let cli_short = TestCli::try_parse_from(["cc-log-viewer", "-p", "8080"]).unwrap();
    assert_eq!(cli_short.port, 8080);

    // Test custom port with long flag
    let cli_long = TestCli::try_parse_from(["cc-log-viewer", "--port", "3000"]).unwrap();
    assert_eq!(cli_long.port, 3000);
}

#[test]
fn test_cli_projects_dir_parsing() {
    // Test without projects dir (should be None)
    let cli_default = TestCli::try_parse_from(["cc-log-viewer"]).unwrap();
    assert!(cli_default.projects_dir.is_none());

    // Test with projects dir
    let cli_with_dir = TestCli::try_parse_from(["cc-log-viewer", "/custom/path"]).unwrap();
    assert_eq!(
        cli_with_dir.projects_dir,
        Some(PathBuf::from("/custom/path"))
    );
}

#[test]
fn test_cli_combined_flags() {
    // Test all flags together
    let cli = TestCli::try_parse_from(["cc-log-viewer", "--tui", "--port", "9000", "/my/projects"])
        .unwrap();

    assert!(cli.tui);
    assert_eq!(cli.port, 9000);
    assert_eq!(cli.projects_dir, Some(PathBuf::from("/my/projects")));
}

#[test]
fn test_cli_invalid_port_handling() {
    // Test that invalid port values are rejected by clap
    let result = TestCli::try_parse_from(["cc-log-viewer", "--port", "not-a-number"]);
    assert!(result.is_err());

    // Test port out of range (0 is technically valid for u16 but may not be useful)
    let cli_zero = TestCli::try_parse_from(["cc-log-viewer", "--port", "0"]).unwrap();
    assert_eq!(cli_zero.port, 0);
}

#[test]
fn test_cli_help_generation() {
    // Test that help can be generated without hanging
    let mut app = TestCli::command();
    let help_text = app.render_help().to_string();

    // Verify key elements are in help text
    assert!(help_text.contains("--tui"));
    assert!(help_text.contains("Use terminal UI instead of web interface"));
    assert!(help_text.contains("--port"));
    assert!(help_text.contains("Port to serve on"));
    assert!(help_text.contains("[default: 2006]"));
    assert!(help_text.contains("PROJECTS_DIR"));
    assert!(help_text.contains("~/.claude/projects"));
}

#[test]
fn test_cli_flag_order_independence() {
    // Test that flag order doesn't matter
    let cli1 = TestCli::try_parse_from(["cc-log-viewer", "--tui", "--port", "8080"]).unwrap();
    let cli2 = TestCli::try_parse_from(["cc-log-viewer", "--port", "8080", "--tui"]).unwrap();

    assert_eq!(cli1.tui, cli2.tui);
    assert_eq!(cli1.port, cli2.port);
}

#[test]
fn test_cli_short_vs_long_flags() {
    // Test that -p and --port work the same
    let cli_short = TestCli::try_parse_from(["cc-log-viewer", "-p", "5000"]).unwrap();
    let cli_long = TestCli::try_parse_from(["cc-log-viewer", "--port", "5000"]).unwrap();

    assert_eq!(cli_short.port, cli_long.port);
}

#[test]
fn test_cli_unknown_flag_handling() {
    // Test that unknown flags are rejected
    let result = TestCli::try_parse_from(["cc-log-viewer", "--unknown-flag"]);
    assert!(result.is_err());

    let error_msg = format!("{}", result.unwrap_err());
    assert!(
        error_msg.contains("unexpected")
            || error_msg.contains("unrecognized")
            || error_msg.contains("error")
            || error_msg.contains("unknown")
    );
}

#[test]
fn test_cli_version_info_structure() {
    // Test that version information can be accessed
    let mut app = TestCli::command();

    // Should have name and about text
    assert_eq!(app.get_name(), "cc-log-viewer");

    let about = app.get_about().unwrap().to_string();
    assert!(about.contains("Claude Code log viewer"));
    assert!(about.contains("Web interface for viewing conversation logs"));
}
