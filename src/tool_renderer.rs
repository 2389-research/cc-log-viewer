// ABOUTME: Tool rendering library - Handles rich formatting of Claude Code tools for multiple output formats
// ABOUTME: Provides unified interface for rendering tool inputs/outputs as markdown, HTML, or other formats

use serde_json::Value;
use std::collections::HashMap;

/// Output format for tool rendering
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Markdown,
    Html,
}

/// Tool rendering context with metadata
#[derive(Debug, Clone)]
pub struct RenderContext {
    pub tool_name: String,
    pub tool_id: Option<String>,
    pub timestamp: Option<String>,
    pub session_id: String,
    pub project_name: String,
}

/// Result of tool rendering
#[derive(Debug, Clone)]
pub struct RenderedTool {
    pub header: String,
    pub input: String,
    pub output: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Main tool renderer that dispatches to specific tool handlers
pub struct ToolRenderer {
    handlers: HashMap<String, Box<dyn ToolHandler>>,
}

/// Trait for rendering individual tool types
pub trait ToolHandler {
    /// Get the icon for this tool
    fn get_icon(&self) -> &'static str;

    /// Render tool input
    fn render_input(&self, input: &Value, format: OutputFormat, context: &RenderContext) -> String;

    /// Render tool output/result
    fn render_output(
        &self,
        output: &Value,
        input: &Value,
        format: OutputFormat,
        context: &RenderContext,
    ) -> String;

    /// Get tool-specific metadata
    fn get_metadata(&self, _input: &Value, _output: Option<&Value>) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl ToolRenderer {
    pub fn new() -> Self {
        let mut renderer = Self {
            handlers: HashMap::new(),
        };

        // Register all known tool handlers
        renderer.register_core_tools();
        renderer.register_mcp_tools();

        renderer
    }

    /// Register all core Claude Code tools
    fn register_core_tools(&mut self) {
        self.register("Bash", Box::new(BashHandler));
        self.register("Read", Box::new(ReadHandler));
        self.register("Edit", Box::new(EditHandler));
        self.register("MultiEdit", Box::new(MultiEditHandler));
        self.register("Write", Box::new(WriteHandler));
        self.register("TodoWrite", Box::new(TodoWriteHandler));
        self.register("LS", Box::new(LSHandler));
        self.register("Grep", Box::new(GrepHandler));
        self.register("Glob", Box::new(GlobHandler));
        self.register("WebFetch", Box::new(WebFetchHandler));
        self.register("WebSearch", Box::new(WebSearchHandler));
        self.register("Task", Box::new(TaskHandler));
        self.register("NotebookRead", Box::new(NotebookReadHandler));
        self.register("NotebookEdit", Box::new(NotebookEditHandler));
        self.register("ExitPlanMode", Box::new(ExitPlanModeHandler));
    }

    /// Register MCP tools
    fn register_mcp_tools(&mut self) {
        self.register(
            "mcp__private-journal__process_thoughts",
            Box::new(PrivateJournalHandler),
        );
        self.register("mcp__socialmedia__login", Box::new(SocialMediaLoginHandler));
        self.register(
            "mcp__socialmedia__create_post",
            Box::new(SocialMediaCreatePostHandler),
        );
        self.register("mcp__vocalize__speak", Box::new(VocalizeHandler));
        self.register("mcp__playwright__navigate", Box::new(PlaywrightHandler));
        // Add more MCP tools as needed

        // Register default handler for unknown tools
        self.register("_default", Box::new(DefaultHandler));
    }

    /// Register a tool handler
    pub fn register(&mut self, tool_name: &str, handler: Box<dyn ToolHandler>) {
        self.handlers.insert(tool_name.to_string(), handler);
    }

    /// Render a complete tool interaction
    pub fn render_tool(
        &self,
        tool_name: &str,
        input: &Value,
        output: Option<&Value>,
        format: OutputFormat,
        context: &RenderContext,
    ) -> RenderedTool {
        let handler = self
            .handlers
            .get(tool_name)
            .unwrap_or_else(|| self.handlers.get("_default").unwrap());

        let icon = handler.get_icon();
        let header = match format {
            OutputFormat::Markdown => format!("### {} {}\n\n", icon, tool_name),
            OutputFormat::Html => format!("<h3>{} {}</h3>", icon, tool_name),
        };

        let input_rendered = handler.render_input(input, format, context);
        let output_rendered = output.map(|o| handler.render_output(o, input, format, context));
        let metadata = handler.get_metadata(input, output);

        RenderedTool {
            header,
            input: input_rendered,
            output: output_rendered,
            metadata,
        }
    }

    /// Render a complete tool interaction (convenience method)
    pub fn render_tool_with_result(
        &self,
        tool_name: &str,
        input: &Value,
        output: Option<&Value>,
        format: OutputFormat,
        context: &RenderContext,
    ) -> Option<RenderedTool> {
        Some(self.render_tool(tool_name, input, output, format, context))
    }

    /// Check if a tool is supported
    pub fn supports_tool(&self, tool_name: &str) -> bool {
        self.handlers.contains_key(tool_name)
    }

    /// Get list of supported tools
    pub fn supported_tools(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }
}

impl Default for ToolRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// Core tool handlers
struct BashHandler;
impl ToolHandler for BashHandler {
    fn get_icon(&self) -> &'static str {
        "💻"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let command = input.get("command").and_then(|c| c.as_str()).unwrap_or("");
        let description = input.get("description").and_then(|d| d.as_str());

        let mut content = format!("$ {}", command);
        if let Some(desc) = description {
            content.push_str(&format!("\n# {}", desc));
        }

        format_utils::code_block(&content, Some("bash"), format)
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        format_utils::render_data_tool_output(output, "Output", format)
    }
}

struct ReadHandler;
impl ToolHandler for ReadHandler {
    fn get_icon(&self) -> &'static str {
        "📖"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let file_path = input
            .get("file_path")
            .and_then(|f| f.as_str())
            .unwrap_or("");
        let mut result = format!("📄 {}", format_utils::bold(file_path, format));

        if let (Some(offset), Some(limit)) = (
            input.get("offset").and_then(|o| o.as_u64()),
            input.get("limit").and_then(|l| l.as_u64()),
        ) {
            let line_info = format!("Lines: {}-{}", offset + 1, offset + limit);
            result.push_str(&match format {
                OutputFormat::Markdown => {
                    format!("\n{}\n\n", format_utils::italic(&line_info, format))
                }
                OutputFormat::Html => format!(
                    "<br><small>{}</small>",
                    format_utils::italic(&line_info, format)
                ),
            });
        } else {
            result.push_str(&match format {
                OutputFormat::Markdown => "\n\n".to_string(),
                OutputFormat::Html => "".to_string(),
            });
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        format_utils::render_data_tool_output(output, "Content", format)
    }
}

struct EditHandler;
impl ToolHandler for EditHandler {
    fn get_icon(&self) -> &'static str {
        "✏️"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let file_path = input
            .get("file_path")
            .and_then(|f| f.as_str())
            .unwrap_or("");
        let mut result = format!("✏️ {}\n\n", format_utils::bold(file_path, format));

        if let (Some(old_string), Some(new_string)) = (
            input.get("old_string").and_then(|o| o.as_str()),
            input.get("new_string").and_then(|n| n.as_str()),
        ) {
            result.push_str(&format_utils::diff_block(old_string, new_string, format));
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            if !content.trim().is_empty() {
                let header = match format {
                    OutputFormat::Markdown => "**Result:**\n",
                    OutputFormat::Html => "<h4>Result:</h4>",
                };
                format!(
                    "{}{}",
                    header,
                    format_utils::code_block(content, None, format)
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }
}

struct MultiEditHandler;
impl ToolHandler for MultiEditHandler {
    fn get_icon(&self) -> &'static str {
        "🔄"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let file_path = input
            .get("file_path")
            .and_then(|f| f.as_str())
            .unwrap_or("");
        let empty_vec = vec![];
        let edits = input
            .get("edits")
            .and_then(|e| e.as_array())
            .unwrap_or(&empty_vec);

        let mut result = format!(
            "🔄 Multiple Edits to {} ({} changes)\n\n",
            format_utils::bold(file_path, format),
            edits.len()
        );

        for (i, edit) in edits.iter().enumerate() {
            let edit_header = format!("Edit {}", i + 1);
            if let Some(replace_all) = edit.get("replace_all").and_then(|r| r.as_bool()) {
                if replace_all {
                    result.push_str(&format!(
                        "{} (replace all)\n",
                        format_utils::bold(&edit_header, format)
                    ));
                } else {
                    result.push_str(&format!("{}\n", format_utils::bold(&edit_header, format)));
                }
            } else {
                result.push_str(&format!("{}\n", format_utils::bold(&edit_header, format)));
            }

            if let (Some(old_string), Some(new_string)) = (
                edit.get("old_string").and_then(|o| o.as_str()),
                edit.get("new_string").and_then(|n| n.as_str()),
            ) {
                result.push_str(&format_utils::diff_block(old_string, new_string, format));
            }
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            if !content.trim().is_empty() {
                let header = match format {
                    OutputFormat::Markdown => "**Result:**\n",
                    OutputFormat::Html => "<h4>Result:</h4>",
                };
                format!(
                    "{}{}",
                    header,
                    format_utils::code_block(content, None, format)
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }
}

struct TodoWriteHandler;
impl ToolHandler for TodoWriteHandler {
    fn get_icon(&self) -> &'static str {
        "📝"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let empty_vec = vec![];
        let todos = input
            .get("todos")
            .and_then(|t| t.as_array())
            .unwrap_or(&empty_vec);
        let mut result = format!(
            "📝 Todo List ({} items)\n\n",
            format_utils::bold(&todos.len().to_string(), format)
        );

        for todo in todos {
            let status = todo
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("pending");
            let content = todo.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let priority = todo
                .get("priority")
                .and_then(|p| p.as_str())
                .unwrap_or("medium");
            let id = todo.get("id").and_then(|i| i.as_str()).unwrap_or("");

            let (status_icon, formatted_content) = match status {
                "completed" => (
                    "✅",
                    match format {
                        OutputFormat::Markdown => format!("~~{}~~", content),
                        OutputFormat::Html => {
                            format!("<del>{}</del>", format_utils::html_escape(content))
                        }
                    },
                ),
                "in_progress" => ("🔄", format_utils::bold(content, format)),
                _ => ("⭕", content.to_string()),
            };

            let priority_icon = match priority {
                "high" => "🟢",
                "medium" => "🟠",
                "low" => "🔴",
                _ => "⚪",
            };

            result.push_str(&format!("{} {}\n", status_icon, formatted_content));
            result.push_str(&format!(
                "{} {} priority • ID: {}\n\n",
                priority_icon, priority, id
            ));
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Todo updated:**\n",
                OutputFormat::Html => "<h4>Todo updated:</h4>",
            };
            format!("{}{}", header, format_utils::blockquote(content, format))
        } else {
            String::new()
        }
    }
}

struct WriteHandler;
impl ToolHandler for WriteHandler {
    fn get_icon(&self) -> &'static str {
        "📝"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let file_path = input
            .get("file_path")
            .and_then(|f| f.as_str())
            .unwrap_or("");
        let mut result = format!("📝 {}\n\n", format_utils::bold(file_path, format));

        if let Some(content) = input.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Content:**\n",
                OutputFormat::Html => "<h4>Content:</h4>",
            };
            result.push_str(&format!(
                "{}{}",
                header,
                format_utils::code_block(content, None, format)
            ));
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            if !content.trim().is_empty() {
                let header = match format {
                    OutputFormat::Markdown => "**Result:**\n",
                    OutputFormat::Html => "<h4>Result:</h4>",
                };
                format!(
                    "{}{}",
                    header,
                    format_utils::code_block(content, None, format)
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }
}

struct LSHandler;
impl ToolHandler for LSHandler {
    fn get_icon(&self) -> &'static str {
        "📂"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let path = input.get("path").and_then(|p| p.as_str()).unwrap_or("");
        format!("📂 {}\n\n", format_utils::bold(path, format))
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        format_utils::render_data_tool_output(output, "Directory listing", format)
    }
}

struct GrepHandler;
impl ToolHandler for GrepHandler {
    fn get_icon(&self) -> &'static str {
        "🔍"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::new();

        if let Some(pattern) = input.get("pattern").and_then(|p| p.as_str()) {
            result.push_str(&format!(
                "**Pattern:** {}\n",
                format_utils::inline_code(pattern, format)
            ));
        }

        if let Some(path) = input.get("path").and_then(|p| p.as_str()) {
            result.push_str(&format!("**Path:** {}\n", path));
        }

        if let Some(glob) = input.get("glob").and_then(|g| g.as_str()) {
            result.push_str(&format!("**Glob:** {}\n", glob));
        }

        result.push('\n');
        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        format_utils::render_data_tool_output(output, "Matches", format)
    }
}

struct GlobHandler;
impl ToolHandler for GlobHandler {
    fn get_icon(&self) -> &'static str {
        "🌐"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::new();

        if let Some(pattern) = input.get("pattern").and_then(|p| p.as_str()) {
            result.push_str(&format!(
                "**Pattern:** {}\n",
                format_utils::inline_code(pattern, format)
            ));
        }

        if let Some(path) = input.get("path").and_then(|p| p.as_str()) {
            result.push_str(&format!("**Path:** {}\n", path));
        }

        result.push('\n');
        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        format_utils::render_data_tool_output(output, "Found files", format)
    }
}

struct WebFetchHandler;
impl ToolHandler for WebFetchHandler {
    fn get_icon(&self) -> &'static str {
        "🌐"
    }

    fn render_input(
        &self,
        input: &Value,
        _format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::new();

        if let Some(url) = input.get("url").and_then(|u| u.as_str()) {
            result.push_str(&format!("🌐 **URL:** {}\n", url));
        }

        if let Some(prompt) = input.get("prompt").and_then(|p| p.as_str()) {
            result.push_str(&format!("**Query:** {}\n", prompt));
        }

        result.push('\n');
        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let header = match format {
            OutputFormat::Markdown => "**Fetched content:**\n",
            OutputFormat::Html => "<h4>Fetched content:</h4>",
        };

        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            if !content.trim().is_empty() {
                format!("{}{}", header, format_utils::blockquote(content, format))
            } else {
                format!(
                    "{}{}\n\n",
                    header,
                    format_utils::italic("(empty content)", format)
                )
            }
        } else {
            // Show the raw output structure if content is missing
            format!(
                "{}{}",
                header,
                format_utils::code_block(
                    &serde_json::to_string_pretty(output).unwrap_or_default(),
                    Some("json"),
                    format
                )
            )
        }
    }
}

struct WebSearchHandler;
impl ToolHandler for WebSearchHandler {
    fn get_icon(&self) -> &'static str {
        "🔍"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::new();

        if let Some(query) = input.get("query").and_then(|q| q.as_str()) {
            result.push_str(&format!(
                "🔍 **Query:** {}\n",
                format_utils::inline_code(query, format)
            ));
        }

        if let Some(allowed_domains) = input.get("allowed_domains").and_then(|d| d.as_array()) {
            let domains: Vec<String> = allowed_domains
                .iter()
                .filter_map(|d| d.as_str())
                .map(|s| s.to_string())
                .collect();
            if !domains.is_empty() {
                result.push_str(&format!("**Allowed domains:** {}\n", domains.join(", ")));
            }
        }

        result.push('\n');
        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let header = match format {
            OutputFormat::Markdown => "**Search results:**\n",
            OutputFormat::Html => "<h4>Search results:</h4>",
        };

        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            if !content.trim().is_empty() {
                format!("{}{}", header, format_utils::blockquote(content, format))
            } else {
                format!(
                    "{}{}\n\n",
                    header,
                    format_utils::italic("(empty results)", format)
                )
            }
        } else {
            // Show the raw output structure if content is missing
            format!(
                "{}{}",
                header,
                format_utils::code_block(
                    &serde_json::to_string_pretty(output).unwrap_or_default(),
                    Some("json"),
                    format
                )
            )
        }
    }
}

struct TaskHandler;
impl ToolHandler for TaskHandler {
    fn get_icon(&self) -> &'static str {
        "🎯"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::new();

        if let Some(description) = input.get("description").and_then(|d| d.as_str()) {
            result.push_str(&format!("**Task:** {}\n", description));
        }

        if let Some(prompt) = input.get("prompt").and_then(|p| p.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Instructions:**\n",
                OutputFormat::Html => "<h4>Instructions:</h4>",
            };
            result.push_str(&format!(
                "{}{}",
                header,
                format_utils::blockquote(prompt, format)
            ));
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let header = match format {
            OutputFormat::Markdown => "**Task completion:**\n",
            OutputFormat::Html => "<h4>Task completion:</h4>",
        };

        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            if !content.trim().is_empty() {
                format!("{}{}", header, format_utils::blockquote(content, format))
            } else {
                format!(
                    "{}{}\n\n",
                    header,
                    format_utils::italic("(task completed with no output)", format)
                )
            }
        } else {
            // Show the raw output structure if content is missing
            format!(
                "{}{}",
                header,
                format_utils::code_block(
                    &serde_json::to_string_pretty(output).unwrap_or_default(),
                    Some("json"),
                    format
                )
            )
        }
    }
}

struct NotebookReadHandler;
impl ToolHandler for NotebookReadHandler {
    fn get_icon(&self) -> &'static str {
        "📓"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let notebook_path = input
            .get("notebook_path")
            .and_then(|f| f.as_str())
            .unwrap_or("");
        let mut result = format!("📓 {}\n", format_utils::bold(notebook_path, format));

        if let Some(cell_id) = input.get("cell_id").and_then(|c| c.as_str()) {
            result.push_str(&format!("**Cell ID:** {}\n", cell_id));
        }

        result.push('\n');
        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let header = match format {
            OutputFormat::Markdown => "**Notebook content:**\n",
            OutputFormat::Html => "<h4>Notebook content:</h4>",
        };

        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            if !content.trim().is_empty() {
                format!(
                    "{}{}",
                    header,
                    format_utils::code_block(content, Some("json"), format)
                )
            } else {
                format!(
                    "{}{}\n\n",
                    header,
                    format_utils::italic("(empty notebook content)", format)
                )
            }
        } else {
            // Show the raw output structure if content is missing
            format!(
                "{}{}",
                header,
                format_utils::code_block(
                    &serde_json::to_string_pretty(output).unwrap_or_default(),
                    Some("json"),
                    format
                )
            )
        }
    }
}

struct NotebookEditHandler;
impl ToolHandler for NotebookEditHandler {
    fn get_icon(&self) -> &'static str {
        "📝"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let notebook_path = input
            .get("notebook_path")
            .and_then(|f| f.as_str())
            .unwrap_or("");
        let mut result = format!("📝 {}\n", format_utils::bold(notebook_path, format));

        if let Some(cell_type) = input.get("cell_type").and_then(|c| c.as_str()) {
            result.push_str(&format!("**Cell type:** {}\n", cell_type));
        }

        if let Some(new_source) = input.get("new_source").and_then(|s| s.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**New content:**\n",
                OutputFormat::Html => "<h4>New content:</h4>",
            };
            result.push_str(&format!(
                "{}{}",
                header,
                format_utils::code_block(new_source, None, format)
            ));
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Result:**\n",
                OutputFormat::Html => "<h4>Result:</h4>",
            };
            format!(
                "{}{}",
                header,
                format_utils::code_block(content, None, format)
            )
        } else {
            String::new()
        }
    }
}

struct ExitPlanModeHandler;
impl ToolHandler for ExitPlanModeHandler {
    fn get_icon(&self) -> &'static str {
        "🎯"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(plan) = input.get("plan").and_then(|p| p.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Plan:**\n",
                OutputFormat::Html => "<h4>Plan:</h4>",
            };
            format!("{}{}", header, format_utils::blockquote(plan, format))
        } else {
            "**Exiting plan mode**\n\n".to_string()
        }
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Result:**\n",
                OutputFormat::Html => "<h4>Result:</h4>",
            };
            format!("{}{}", header, format_utils::blockquote(content, format))
        } else {
            String::new()
        }
    }
}

// MCP Tool Handlers
struct PrivateJournalHandler;
impl ToolHandler for PrivateJournalHandler {
    fn get_icon(&self) -> &'static str {
        "📔"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::from("📔 **Private Journal Entry**\n\n");

        if let Some(obj) = input.as_object() {
            for (key, value) in obj {
                if let Some(content) = value.as_str() {
                    if !content.trim().is_empty() {
                        let section_name = key.replace('_', " ").to_uppercase();
                        result.push_str(&format!("**{}:**\n", section_name));
                        result.push_str(&format_utils::blockquote(content, format));
                    }
                }
            }
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Journal saved:**\n",
                OutputFormat::Html => "<h4>Journal saved:</h4>",
            };
            format!("{}{}", header, format_utils::blockquote(content, format))
        } else {
            String::new()
        }
    }
}

struct SocialMediaLoginHandler;
impl ToolHandler for SocialMediaLoginHandler {
    fn get_icon(&self) -> &'static str {
        "🔐"
    }

    fn render_input(
        &self,
        input: &Value,
        _format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::from("🔐 **Social Media Login**\n");

        if let Some(platform) = input.get("platform").and_then(|p| p.as_str()) {
            result.push_str(&format!("**Platform:** {}\n", platform));
        }

        result.push('\n');
        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Login result:**\n",
                OutputFormat::Html => "<h4>Login result:</h4>",
            };
            format!(
                "{}{}",
                header,
                format_utils::code_block(content, None, format)
            )
        } else {
            String::new()
        }
    }
}

struct SocialMediaCreatePostHandler;
impl ToolHandler for SocialMediaCreatePostHandler {
    fn get_icon(&self) -> &'static str {
        "📱"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::from("📱 **Creating Social Media Post**\n");

        if let Some(content) = input.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Content:**\n",
                OutputFormat::Html => "<h4>Content:</h4>",
            };
            result.push_str(&format!(
                "{}{}",
                header,
                format_utils::blockquote(content, format)
            ));
        }

        if let Some(platform) = input.get("platform").and_then(|p| p.as_str()) {
            result.push_str(&format!("**Platform:** {}\n\n", platform));
        }

        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Post result:**\n",
                OutputFormat::Html => "<h4>Post result:</h4>",
            };
            format!(
                "{}{}",
                header,
                format_utils::code_block(content, None, format)
            )
        } else {
            String::new()
        }
    }
}

struct VocalizeHandler;
impl ToolHandler for VocalizeHandler {
    fn get_icon(&self) -> &'static str {
        "🔊"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::from("🔊 **Text-to-Speech**\n");

        if let Some(text) = input.get("text").and_then(|t| t.as_str()) {
            result.push_str(&format!(
                "**Text:** {}\n",
                format_utils::inline_code(text, format)
            ));
        }

        if let Some(voice) = input.get("voice").and_then(|v| v.as_str()) {
            result.push_str(&format!("**Voice:** {}\n", voice));
        }

        if let Some(emotion) = input.get("emotion").and_then(|e| e.as_str()) {
            result.push_str(&format!("**Emotion:** {}\n", emotion));
        }

        result.push('\n');
        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Speech result:**\n",
                OutputFormat::Html => "<h4>Speech result:</h4>",
            };
            format!("{}{}", header, format_utils::blockquote(content, format))
        } else {
            String::new()
        }
    }
}

struct PlaywrightHandler;
impl ToolHandler for PlaywrightHandler {
    fn get_icon(&self) -> &'static str {
        "🎭"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        let mut result = String::from("🎭 **Playwright Automation**\n");

        if let Some(url) = input.get("url").and_then(|u| u.as_str()) {
            result.push_str(&format!("**URL:** {}\n", url));
        }

        if let Some(selector) = input.get("selector").and_then(|s| s.as_str()) {
            result.push_str(&format!(
                "**Selector:** {}\n",
                format_utils::inline_code(selector, format)
            ));
        }

        if let Some(action) = input.get("action").and_then(|a| a.as_str()) {
            result.push_str(&format!("**Action:** {}\n", action));
        }

        result.push('\n');
        result
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            let header = match format {
                OutputFormat::Markdown => "**Playwright result:**\n",
                OutputFormat::Html => "<h4>Playwright result:</h4>",
            };
            format!(
                "{}{}",
                header,
                format_utils::code_block(content, None, format)
            )
        } else {
            String::new()
        }
    }
}

// Utility functions for common formatting patterns
pub mod format_utils {
    use super::OutputFormat;
    use serde_json::Value;

    /// Standard output rendering for data tools that should always show content
    pub fn render_data_tool_output(
        output: &Value,
        header_text: &str,
        format: OutputFormat,
    ) -> String {
        let header = match format {
            OutputFormat::Markdown => format!("**{}:**\n", header_text),
            OutputFormat::Html => format!("<h4>{}:</h4>", header_text),
        };

        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
            if !content.trim().is_empty() {
                format!("{}{}", header, code_block(content, None, format))
            } else {
                format!("{}{}\n\n", header, italic("(empty output)", format))
            }
        } else {
            // Show the raw output structure if content is missing
            format!(
                "{}{}",
                header,
                code_block(
                    &serde_json::to_string_pretty(output).unwrap_or_default(),
                    Some("json"),
                    format
                )
            )
        }
    }

    pub fn code_block(content: &str, language: Option<&str>, format: OutputFormat) -> String {
        match format {
            OutputFormat::Markdown => {
                let lang = language.unwrap_or("");
                format!("```{}\n{}\n```\n\n", lang, content)
            }
            OutputFormat::Html => {
                let class = language
                    .map(|l| format!(" class=\"language-{}\"", l))
                    .unwrap_or_default();
                format!("<pre><code{}>{}</code></pre>", class, html_escape(content))
            }
        }
    }

    pub fn diff_block(old_content: &str, new_content: &str, format: OutputFormat) -> String {
        match format {
            OutputFormat::Markdown => {
                let mut diff = String::from("```diff\n");
                for line in old_content.lines() {
                    diff.push_str(&format!("- {}\n", line));
                }
                for line in new_content.lines() {
                    diff.push_str(&format!("+ {}\n", line));
                }
                diff.push_str("```\n\n");
                diff
            }
            OutputFormat::Html => {
                let mut html = String::from("<div class=\"diff\">");
                for line in old_content.lines() {
                    html.push_str(&format!(
                        "<div class=\"diff-removed\">- {}</div>",
                        html_escape(line)
                    ));
                }
                for line in new_content.lines() {
                    html.push_str(&format!(
                        "<div class=\"diff-added\">+ {}</div>",
                        html_escape(line)
                    ));
                }
                html.push_str("</div>");
                html
            }
        }
    }

    pub fn blockquote(content: &str, format: OutputFormat) -> String {
        match format {
            OutputFormat::Markdown => {
                format!("> {}\n\n", content.replace('\n', "\n> "))
            }
            OutputFormat::Html => {
                format!("<blockquote>{}</blockquote>", html_escape(content))
            }
        }
    }

    pub fn bold(text: &str, format: OutputFormat) -> String {
        match format {
            OutputFormat::Markdown => format!("**{}**", text),
            OutputFormat::Html => format!("<strong>{}</strong>", html_escape(text)),
        }
    }

    pub fn italic(text: &str, format: OutputFormat) -> String {
        match format {
            OutputFormat::Markdown => format!("*{}*", text),
            OutputFormat::Html => format!("<em>{}</em>", html_escape(text)),
        }
    }

    pub fn inline_code(text: &str, format: OutputFormat) -> String {
        match format {
            OutputFormat::Markdown => format!("`{}`", text),
            OutputFormat::Html => format!("<code>{}</code>", html_escape(text)),
        }
    }

    pub fn html_escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }
}

// Default handler for unknown tools
struct DefaultHandler;
impl ToolHandler for DefaultHandler {
    fn get_icon(&self) -> &'static str {
        "🔧"
    }

    fn render_input(
        &self,
        input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        format_utils::code_block(
            &serde_json::to_string_pretty(input).unwrap_or_default(),
            Some("json"),
            format,
        )
    }

    fn render_output(
        &self,
        output: &Value,
        _input: &Value,
        format: OutputFormat,
        _context: &RenderContext,
    ) -> String {
        format_utils::render_data_tool_output(output, "Output", format)
    }

    fn get_metadata(&self, _input: &Value, _output: Option<&Value>) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "unknown".to_string());
        metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_renderer_creation() {
        let renderer = ToolRenderer::new();
        assert!(renderer.supports_tool("Bash"));
        assert!(renderer.supports_tool("Read"));
        assert!(!renderer.supports_tool("UnknownTool"));
    }

    #[test]
    fn test_format_utils() {
        use format_utils::*;

        let code = code_block("echo hello", Some("bash"), OutputFormat::Markdown);
        assert!(code.contains("```bash"));

        let diff = diff_block("old", "new", OutputFormat::Markdown);
        assert!(diff.contains("- old"));
        assert!(diff.contains("+ new"));
    }
}
