# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Testing
- **Run all tests**: `./run-tests.sh` - Comprehensive test runner with formatting, linting, build, and all test types
- **Quick tests**: `./run-tests.sh --quick` - Skip benchmarks and coverage for faster feedback
- **Unit tests only**: `./run-tests.sh --unit-only` or `cargo test --lib`
- **Integration tests**: `./run-tests.sh --integration-only` or `cargo test --test <test_name>`
- **Verbose output**: `./run-tests.sh --verbose` - Shows detailed test output
- **With coverage**: `./run-tests.sh --coverage` - Generates HTML coverage report
- **Single test**: `cargo test <test_name>` - Run specific test by name
- **Documentation tests**: `cargo test --doc` - Test code examples in docs

### Build and Development
- **Development build**: `cargo build`
- **Release build**: `cargo build --release`
- **Run locally**: `cargo run` - Serves on port 2006 with default Claude Code projects dir
- **Custom port**: `cargo run -- --port 3000`
- **Custom projects dir**: `cargo run -- /path/to/projects`
- **Format code**: `cargo fmt`
- **Lint code**: `cargo clippy -- -D warnings`
- **Pedantic linting**: `CLIPPY_PEDANTIC=true ./run-tests.sh`
- **Generate docs**: `cargo doc --no-deps --all-features`
- **Security audit**: `cargo audit` (install with `cargo install cargo-audit`)

### Installation
- **Install from crates.io**: `cargo install cc-log-viewer`
- **Install from source**: `cargo install --path .`

## Architecture Overview

This is a Rust-based web application that provides a rich interface for viewing Claude Code conversation logs.

### Core Components

**Backend (Rust)**:
- `main.rs` - CLI argument parsing and web server setup
- `lib.rs` - Core business logic, WebSocket handling, and API endpoints
- Uses Axum web framework with async/await throughout
- Real-time file watching with `notify` crate for live log updates
- WebSocket support for streaming log updates to frontend

**Frontend (Static HTML/JS)**:
- `static/index.html` - Main conversation viewer interface
- `static/live.html` - Real-time log monitoring interface
- Rich tool handler system with specialized rendering for each Claude Code tool type
- Responsive design with message avatars and timestamps

### Key Features

**Log Processing**:
- Parses JSONL files from `~/.claude/projects/` directory structure
- Handles tool use results and associates them with corresponding tool calls
- Real-time streaming of new log entries via WebSocket

**Tool Rendering**:
- Specialized handlers for all Claude Code tools (TodoWrite, Bash, Read, Edit, MultiEdit, etc.)
- Visual diff rendering for file edits
- Syntax highlighting and pretty-printing of JSON/code content
- Image support with base64 decoding and inline display

**Navigation**:
- Three-level hierarchy: Projects → Sessions → Conversations
- Bookmarkable URLs for specific projects and sessions
- Live activity monitoring across all projects

### Project Structure
- `src/` - Rust source code (main.rs, lib.rs)
- `static/` - Frontend HTML/CSS/JS files
- `tests/` - Integration and unit tests
- `run-tests.sh` - Comprehensive test runner script
- `Cargo.toml` - Rust dependencies and metadata

### Dependencies
- **Web**: `axum`, `tower-http`, `tokio` for async web server
- **Serialization**: `serde`, `serde_json` for JSON handling
- **CLI**: `clap` for command-line argument parsing
- **File watching**: `notify` for real-time log monitoring
- **WebSocket**: `tokio-tungstenite` for real-time communication
- **Utilities**: `chrono`, `walkdir`, `uuid`, `dashmap`

### Testing Strategy
- Unit tests in `src/` files using `#[cfg(test)]` modules
- Integration tests in `tests/` directory
- End-to-end WebSocket and API testing
- Property-based testing for log parsing
- Coverage reporting via `cargo-llvm-cov`
- Security auditing via `cargo-audit`

### Development Workflow
1. Use `./run-tests.sh` frequently during development
2. Code must pass `cargo clippy -- -D warnings` (zero warnings policy)
3. All code must be formatted with `cargo fmt`
4. Integration tests verify WebSocket functionality and log parsing
5. Documentation tests ensure code examples remain valid

### Default Paths and Configuration
- **Default projects directory**: `~/.claude/projects/`
- **Default port**: `2006`
- **Live monitoring endpoint**: `/live.html`
- **WebSocket endpoint**: `/ws` for real-time updates
- **API endpoints**: `/api/projects`, `/api/sessions/{project}`, `/api/logs/{project}/{session}`
