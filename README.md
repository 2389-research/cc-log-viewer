# 🤖 Claude Code Log Viewer

A beautiful, feature-rich web interface for viewing and auditing Claude Code conversation logs. Transform your raw JSONL log files into an intuitive, searchable, and visually appealing conversation browser.

![Claude Code Log Viewer](https://img.shields.io/badge/Claude_Code-Log_Viewer-blue?style=for-the-badge)
![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Web](https://img.shields.io/badge/web-interface-green?style=for-the-badge)

## ✨ Features

### 🎨 **Rich Visual Interface**
- **Modern Design**: Clean, responsive web interface with intuitive navigation
- **Smart Layout**: Human messages on right, AI on left for natural conversation flow
- **Message Avatars**: Clear visual distinction between users and AI
- **Timestamps**: Detailed timing information for every interaction

### 🛠️ **Advanced Tool Rendering**
The log viewer includes specialized handlers for all major Claude Code tools:

- **📝 TodoWrite**: Renders todo lists with status indicators, priorities, and progress tracking
- **💻 Bash**: Shows commands with `$` prefix, descriptions, and formatted output
- **📖 Read**: Displays file paths with line ranges and syntax-highlighted content
- **✏️ Edit**: Visual diff view with red/green highlighting for changes
- **🔄 MultiEdit**: Multiple file edits in organized cards with numbered changes
- **📁 LS**: Directory listings with proper formatting
- **🔍 Grep**: Search patterns with highlighted results
- **🗂️ Glob**: File pattern matching with smart file type icons
- **🎯 Task**: Structured task display with descriptions and instructions
- **🌐 WebFetch**: Clickable URLs with analysis prompts
- **🧠 Private Journal**: Color-coded sections for feelings, insights, and context
- **🔐 Social Media**: Login status and post creation with hashtag rendering

### 🖼️ **Multimodal Support**
- **Image Rendering**: Inline image display with click-to-expand functionality
- **Mixed Content**: Seamless handling of text and images in conversations
- **Base64 Decoding**: Automatic conversion of encoded images

### 🗂️ **Project Organization**
- **Project Browser**: Overview of all your Claude Code projects
- **Session Management**: Easy navigation between conversation sessions
- **Activity Tracking**: Last activity timestamps and session counts
- **Smart URLs**: Bookmarkable links to specific projects and sessions

## 🚀 Quick Start

### Prerequisites
- [Rust](https://rustup.rs/) (latest stable version)
- Claude Code installed and configured

### Installation

1. **Clone the repository:**
   ```bash
   git clone https://github.com/your-repo/cc-log-viewer.git
   cd cc-log-viewer
   ```

2. **Build the project:**
   ```bash
   cargo build --release
   ```

3. **Run the viewer:**
   ```bash
   cargo run
   ```

   Or with custom options:
   ```bash
   # Custom port
   cargo run -- --port 3000

   # Custom projects directory
   cargo run -- /path/to/your/claude/projects

   # Both options
   cargo run -- --port 3000 /path/to/your/claude/projects
   ```

4. **Open your browser:**
   Navigate to `http://localhost:2006` (or your custom port)

## 📖 Usage

### Basic Navigation

1. **Projects View**: Start by selecting a project from the main page
2. **Sessions View**: Browse conversation sessions within a project
3. **Conversation View**: Dive into detailed conversation logs

### Command Line Options

```bash
cc-log-viewer [OPTIONS] [PROJECTS_DIR]

Arguments:
  [PROJECTS_DIR]  Path to projects directory containing log files
                  (defaults to ~/.claude/projects/)

Options:
  -p, --port <PORT>  Port to serve on [default: 2006]
  -h, --help         Print help information
```

### Default Paths

The viewer automatically looks for Claude Code logs in:
- `~/.claude/projects/` (default)
- Each project should contain `.jsonl` files representing conversation sessions

## 🎯 Tool Handler System

The log viewer features a sophisticated tool handler system that provides specialized rendering for different tool types:

### 🏗️ **Architecture**

```javascript
// Base handler provides common functionality
class ToolHandler {
    renderToolCall(toolCall)     // Renders tool input
    renderToolResult(result)     // Renders tool output
    renderInput(input)           // Custom input formatting
    renderOutput(result)         // Custom output formatting
}

// Specialized handlers extend base functionality
class BashHandler extends ToolHandler {
    // Custom bash command rendering with $ prefix
}
```

### 🎨 **Visual Design Principles**

- **Consistent Icons**: Each tool type has a unique emoji identifier
- **Color Coding**: Different tools use distinct color schemes
- **Contextual Formatting**: Input/output styled appropriately for tool type
- **Responsive Design**: All handlers work across device sizes

### 🔧 **Supported Tools**

| Tool | Icon | Description | Special Features |
|------|------|-------------|------------------|
| TodoWrite | 📝 | Task management | Status indicators, priority colors |
| Bash | 💻 | Shell commands | Command highlighting, monospace output |
| Read | 📖 | File reading | Line numbers, syntax awareness |
| Edit | ✏️ | File editing | Diff visualization, change highlighting |
| MultiEdit | 🔄 | Multiple edits | Numbered changes, scrollable diffs |
| Glob | 🗂️ | File patterns | File type icons, smart grouping |
| Task | 🎯 | Agent tasks | Structured prompts, result formatting |
| WebFetch | 🌐 | Web requests | Clickable URLs, analysis display |
| Journal | 🧠 | Private notes | Color-coded sections, organized layout |

## 🌟 Key Features in Detail

### **Smart Tool Result Attribution**
The viewer properly associates tool results with their corresponding tool calls, fixing the common issue where tool outputs appear as user messages.

### **JSON Pretty Printing**
All JSON content is automatically formatted with proper indentation and syntax highlighting.

### **Image Support**
- Base64 image decoding and display
- Click-to-expand functionality
- Support for multiple image formats
- Inline rendering within conversation flow

### **Responsive Design**
- Mobile-friendly interface
- Adaptive layouts for different screen sizes
- Touch-friendly navigation

## 🛠️ Development

### Project Structure

```
cc-log-viewer/
├── src/
│   └── main.rs          # Rust backend server
├── static/
│   └── index.html       # Frontend web interface
├── Cargo.toml           # Rust dependencies
└── README.md           # This file
```

### Building from Source

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run with hot reload during development
cargo run

# Run tests
cargo test

# Check code quality
cargo clippy
```

### Code Quality

The project maintains high code quality standards:
- **Clippy Clean**: Zero warnings from Rust's linter
- **Modern Rust**: Uses latest stable Rust features
- **Error Handling**: Comprehensive error handling throughout
- **Documentation**: Inline code documentation

### Adding New Tool Handlers

1. **Create a new handler class:**
   ```javascript
   class MyToolHandler extends ToolHandler {
       constructor() {
           super('MyTool');
       }

       renderInput(input) {
           // Custom input rendering
       }

       renderOutput(result, toolCall) {
           // Custom output rendering
       }

       getIcon() {
           return '🔥'; // Your tool's emoji
       }
   }
   ```

2. **Register the handler:**
   ```javascript
   const toolHandlers = {
       // ... existing handlers
       'MyTool': new MyToolHandler()
   };
   ```

## 🤝 Contributing

We welcome contributions! Here's how to get started:

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Make your changes**
4. **Run quality checks**:
   ```bash
   cargo clippy    # Check for warnings
   cargo test      # Run tests
   cargo build     # Ensure it builds
   ```
5. **Commit your changes**: `git commit -m 'Add amazing feature'`
6. **Push to the branch**: `git push origin feature/amazing-feature`
7. **Open a Pull Request**

### Development Guidelines

- Follow Rust conventions and idioms
- Maintain clippy-clean code (zero warnings)
- Add tests for new functionality
- Update documentation for user-facing changes
- Use semantic commit messages

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built for the [Claude Code](https://claude.ai/code) community
- Inspired by the need for better log visualization tools
- Uses modern web technologies for optimal user experience

## 🐛 Issues & Support

Found a bug or have a feature request? Please:
1. Check existing [Issues](https://github.com/your-repo/cc-log-viewer/issues)
2. Create a new issue with detailed information
3. Include your system information and steps to reproduce

## 🚀 Roadmap

Future improvements planned:
- [ ] Search and filtering capabilities
- [ ] Export functionality (PDF, HTML)
- [ ] Advanced analytics and insights
- [ ] Plugin system for custom tool handlers
- [ ] Dark mode support
- [ ] Real-time log monitoring

---

**Made with ❤️ for the Claude Code community**
