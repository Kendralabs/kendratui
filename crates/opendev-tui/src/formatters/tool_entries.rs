//! Static tool registry and runtime display map.
//!
//! Contains the `TOOL_REGISTRY` array and `lookup_tool()` resolution logic.

use std::collections::HashMap;
use std::sync::OnceLock;

use opendev_tools_core::ToolDisplayMeta;

use super::tool_categories::{ResultFormat, ToolCategory, ToolDisplayEntry, category_from_name};

/// The static registry — single source of truth for all tool display metadata.
pub(crate) static TOOL_REGISTRY: &[ToolDisplayEntry] = &[
    // File read tools
    ToolDisplayEntry {
        names: &["read_file", "Read"],
        category: ToolCategory::FileRead,
        verb: "Read",
        label: "file",
        primary_arg_keys: &["file_path", "path"],
        result_format: ResultFormat::File,
    },
    ToolDisplayEntry {
        names: &["read_pdf"],
        category: ToolCategory::FileRead,
        verb: "Read",
        label: "pdf",
        primary_arg_keys: &["file_path", "path"],
        result_format: ResultFormat::File,
    },
    ToolDisplayEntry {
        names: &["list_files", "Glob"],
        category: ToolCategory::FileRead,
        verb: "List",
        label: "files",
        primary_arg_keys: &["path", "directory", "pattern"],
        result_format: ResultFormat::Directory,
    },
    // File write tools
    ToolDisplayEntry {
        names: &["write_file", "Write"],
        category: ToolCategory::FileWrite,
        verb: "Write",
        label: "file",
        primary_arg_keys: &["file_path", "path"],
        result_format: ResultFormat::File,
    },
    ToolDisplayEntry {
        names: &["edit_file", "Edit"],
        category: ToolCategory::FileWrite,
        verb: "Edit",
        label: "file",
        primary_arg_keys: &["file_path", "path"],
        result_format: ResultFormat::File,
    },
    ToolDisplayEntry {
        names: &["multi_edit"],
        category: ToolCategory::FileWrite,
        verb: "Edit",
        label: "file",
        primary_arg_keys: &["file_path", "path"],
        result_format: ResultFormat::File,
    },
    ToolDisplayEntry {
        names: &["patch_file", "patch"],
        category: ToolCategory::FileWrite,
        verb: "Patch",
        label: "file",
        primary_arg_keys: &["file_path", "path"],
        result_format: ResultFormat::File,
    },
    // Bash/command tools
    ToolDisplayEntry {
        names: &["run_command", "bash_execute", "Bash"],
        category: ToolCategory::Bash,
        verb: "Bash",
        label: "command",
        primary_arg_keys: &["command"],
        result_format: ResultFormat::Bash,
    },
    // Search tools
    ToolDisplayEntry {
        names: &["grep", "search", "Grep"],
        category: ToolCategory::Search,
        verb: "Grep",
        label: "project",
        primary_arg_keys: &["pattern", "query"],
        result_format: ResultFormat::Directory,
    },
    ToolDisplayEntry {
        names: &["ast_grep", "AstGrep"],
        category: ToolCategory::Search,
        verb: "AST-Grep",
        label: "code",
        primary_arg_keys: &["pattern"],
        result_format: ResultFormat::Directory,
    },
    ToolDisplayEntry {
        names: &["web_search"],
        category: ToolCategory::Search,
        verb: "Search",
        label: "web",
        primary_arg_keys: &["query", "pattern"],
        result_format: ResultFormat::Generic,
    },
    // Web tools
    ToolDisplayEntry {
        names: &["fetch_url", "web_fetch"],
        category: ToolCategory::Web,
        verb: "Fetch",
        label: "url",
        primary_arg_keys: &["url"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["open_browser"],
        category: ToolCategory::Web,
        verb: "Open",
        label: "browser",
        primary_arg_keys: &["url"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["capture_screenshot"],
        category: ToolCategory::Web,
        verb: "Capture Screenshot",
        label: "screenshot",
        primary_arg_keys: &["url", "path"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["capture_web_screenshot", "web_screenshot"],
        category: ToolCategory::Web,
        verb: "Capture Web Screenshot",
        label: "page",
        primary_arg_keys: &["url"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["analyze_image"],
        category: ToolCategory::Web,
        verb: "Analyze Image",
        label: "image",
        primary_arg_keys: &["path", "url"],
        result_format: ResultFormat::Generic,
    },
    // Agent tools
    ToolDisplayEntry {
        names: &["spawn_subagent"],
        category: ToolCategory::Agent,
        verb: "Spawn",
        label: "subagent",
        primary_arg_keys: &["description"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["get_subagent_output"],
        category: ToolCategory::Agent,
        verb: "Get Output",
        label: "subagent",
        primary_arg_keys: &["subagent_id", "id"],
        result_format: ResultFormat::Generic,
    },
    // Symbol tools
    ToolDisplayEntry {
        names: &["find_symbol"],
        category: ToolCategory::Symbol,
        verb: "Find Symbol",
        label: "symbol",
        primary_arg_keys: &["name", "symbol"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["find_referencing_symbols"],
        category: ToolCategory::Symbol,
        verb: "Find References",
        label: "symbol",
        primary_arg_keys: &["name", "symbol"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["insert_before_symbol"],
        category: ToolCategory::Symbol,
        verb: "Insert Before",
        label: "symbol",
        primary_arg_keys: &["name", "symbol"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["insert_after_symbol"],
        category: ToolCategory::Symbol,
        verb: "Insert After",
        label: "symbol",
        primary_arg_keys: &["name", "symbol"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["replace_symbol_body"],
        category: ToolCategory::Symbol,
        verb: "Replace Symbol",
        label: "symbol",
        primary_arg_keys: &["name", "symbol"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["rename_symbol"],
        category: ToolCategory::Symbol,
        verb: "Rename Symbol",
        label: "symbol",
        primary_arg_keys: &["name", "symbol"],
        result_format: ResultFormat::Generic,
    },
    // Plan/task tools
    ToolDisplayEntry {
        names: &["present_plan"],
        category: ToolCategory::Plan,
        verb: "Present Plan",
        label: "plan",
        primary_arg_keys: &["name", "title"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["write_todos"],
        category: ToolCategory::Plan,
        verb: "Todos",
        label: "todos",
        primary_arg_keys: &["name", "title"],
        result_format: ResultFormat::Todo,
    },
    ToolDisplayEntry {
        names: &["update_todo"],
        category: ToolCategory::Plan,
        verb: "Update Todo",
        label: "todo",
        primary_arg_keys: &["id", "name"],
        result_format: ResultFormat::Todo,
    },
    ToolDisplayEntry {
        names: &["complete_todo"],
        category: ToolCategory::Plan,
        verb: "Complete Todo",
        label: "todo",
        primary_arg_keys: &["id", "name"],
        result_format: ResultFormat::Todo,
    },
    ToolDisplayEntry {
        names: &["list_todos"],
        category: ToolCategory::Plan,
        verb: "List Todos",
        label: "todos",
        primary_arg_keys: &[],
        result_format: ResultFormat::Todo,
    },
    ToolDisplayEntry {
        names: &["clear_todos"],
        category: ToolCategory::Plan,
        verb: "Clear Todos",
        label: "todos",
        primary_arg_keys: &[],
        result_format: ResultFormat::Todo,
    },
    ToolDisplayEntry {
        names: &["task_complete"],
        category: ToolCategory::Plan,
        verb: "Complete",
        label: "task",
        primary_arg_keys: &["status", "message"],
        result_format: ResultFormat::Generic,
    },
    // User interaction
    ToolDisplayEntry {
        names: &["ask_user"],
        category: ToolCategory::UserInteraction,
        verb: "Ask",
        label: "user",
        primary_arg_keys: &["question", "message"],
        result_format: ResultFormat::Generic,
    },
    // Notebook
    ToolDisplayEntry {
        names: &["notebook_edit"],
        category: ToolCategory::Notebook,
        verb: "Edit",
        label: "notebook",
        primary_arg_keys: &["path", "file_path"],
        result_format: ResultFormat::File,
    },
    // Misc
    ToolDisplayEntry {
        names: &["invoke_skill"],
        category: ToolCategory::Other,
        verb: "Skill",
        label: "skill",
        primary_arg_keys: &["name", "skill"],
        result_format: ResultFormat::Generic,
    },
    ToolDisplayEntry {
        names: &["past_sessions"],
        category: ToolCategory::Other,
        verb: "Sessions",
        label: "sessions",
        primary_arg_keys: &["action", "session_id", "query"],
        result_format: ResultFormat::Generic,
    },
    // Browser tool
    ToolDisplayEntry {
        names: &["browser"],
        category: ToolCategory::Web,
        verb: "Browse",
        label: "page",
        primary_arg_keys: &["action", "target"],
        result_format: ResultFormat::Generic,
    },
    // Memory tool
    ToolDisplayEntry {
        names: &["memory"],
        category: ToolCategory::Other,
        verb: "Memory",
        label: "memory",
        primary_arg_keys: &["action", "file", "query"],
        result_format: ResultFormat::Generic,
    },
    // Message tool
    ToolDisplayEntry {
        names: &["message"],
        category: ToolCategory::Other,
        verb: "Message",
        label: "channel",
        primary_arg_keys: &["channel", "message"],
        result_format: ResultFormat::Generic,
    },
    // Diff preview tool
    ToolDisplayEntry {
        names: &["diff_preview"],
        category: ToolCategory::FileWrite,
        verb: "Diff",
        label: "file",
        primary_arg_keys: &["file_path"],
        result_format: ResultFormat::File,
    },
    // Todo (legacy single-action) tool
    ToolDisplayEntry {
        names: &["todo"],
        category: ToolCategory::Plan,
        verb: "Todo",
        label: "task",
        primary_arg_keys: &["action", "id", "title"],
        result_format: ResultFormat::Todo,
    },
    // Vision LM tool
    ToolDisplayEntry {
        names: &["vlm"],
        category: ToolCategory::Web,
        verb: "Vision",
        label: "image",
        primary_arg_keys: &["image_path", "image_url", "prompt"],
        result_format: ResultFormat::Generic,
    },
    // LSP query tool
    ToolDisplayEntry {
        names: &["lsp_query"],
        category: ToolCategory::Symbol,
        verb: "LSP",
        label: "query",
        primary_arg_keys: &["action", "file_path"],
        result_format: ResultFormat::Generic,
    },
    // Schedule tool
    ToolDisplayEntry {
        names: &["schedule"],
        category: ToolCategory::Other,
        verb: "Schedule",
        label: "task",
        primary_arg_keys: &["action", "description", "command"],
        result_format: ResultFormat::Generic,
    },
    // Agents tool
    ToolDisplayEntry {
        names: &["agents"],
        category: ToolCategory::Agent,
        verb: "Agents",
        label: "agents",
        primary_arg_keys: &["action"],
        result_format: ResultFormat::Generic,
    },
];

/// Default entry for unknown tools.
pub(crate) static DEFAULT_ENTRY: ToolDisplayEntry = ToolDisplayEntry {
    names: &[],
    category: ToolCategory::Other,
    verb: "Call",
    label: "",
    primary_arg_keys: &[
        "command",
        "file_path",
        "path",
        "url",
        "query",
        "pattern",
        "name",
    ],
    result_format: ResultFormat::Generic,
};

/// MCP fallback entry.
static MCP_ENTRY: ToolDisplayEntry = ToolDisplayEntry {
    names: &[],
    category: ToolCategory::Mcp,
    verb: "MCP",
    label: "tool",
    primary_arg_keys: &[
        "command",
        "file_path",
        "path",
        "url",
        "query",
        "pattern",
        "name",
    ],
    result_format: ResultFormat::Generic,
};

/// Docker fallback entry.
static DOCKER_ENTRY: ToolDisplayEntry = ToolDisplayEntry {
    names: &[],
    category: ToolCategory::Docker,
    verb: "Docker",
    label: "operation",
    primary_arg_keys: &["command", "container", "image", "name"],
    result_format: ResultFormat::Generic,
};

/// Runtime display entries populated from tool `display_meta()` implementations.
/// Provides a fallback for tools not in the static registry.
static RUNTIME_DISPLAY: OnceLock<HashMap<String, ToolDisplayEntry>> = OnceLock::new();

/// Initialize the runtime display map from tool metadata.
///
/// Call this once after tool registration. Only the first call takes effect.
pub fn init_runtime_display(map: HashMap<String, ToolDisplayMeta>) {
    let entries: HashMap<String, ToolDisplayEntry> = map
        .into_iter()
        .map(|(name, meta)| {
            let entry = ToolDisplayEntry {
                names: &[],
                category: category_from_name(meta.category),
                verb: meta.verb,
                label: meta.label,
                primary_arg_keys: meta.primary_arg_keys,
                result_format: ResultFormat::Generic,
            };
            (name, entry)
        })
        .collect();
    let _ = RUNTIME_DISPLAY.set(entries);
}

/// Look up a tool's display metadata by name.
///
/// Resolution order:
/// 1. Static `TOOL_REGISTRY` exact match
/// 2. Runtime display map (from tool `display_meta()`)
/// 3. Prefix fallbacks (`mcp__*`, `docker_*`)
/// 4. `DEFAULT_ENTRY`
pub fn lookup_tool(name: &str) -> &ToolDisplayEntry {
    // 1. Exact match in static registry
    for entry in TOOL_REGISTRY {
        if entry.names.contains(&name) {
            return entry;
        }
    }

    // 2. Runtime display map (from tool display_meta() implementations)
    if let Some(rt) = RUNTIME_DISPLAY.get()
        && let Some(entry) = rt.get(name)
    {
        return entry;
    }

    // 3. Prefix fallbacks
    if name.starts_with("mcp__") {
        return &MCP_ENTRY;
    }
    if name.starts_with("docker_") {
        return &DOCKER_ENTRY;
    }

    &DEFAULT_ENTRY
}
