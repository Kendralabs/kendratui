//! Parameter normalization for tool invocations.
//!
//! Normalizes LLM-produced tool parameters before they reach handlers:
//! - Key normalization (camelCase -> snake_case)
//! - Whitespace stripping on string params
//! - Path resolution (relative -> absolute, ~ expansion)
//! - Workspace root guard (warn for paths outside workspace)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Parameters that contain file/directory paths and should be resolved.
const PATH_PARAMS: &[&str] = &[
    "file_path",
    "notebook_path",
    "output_path",
    "plan_file_path",
    "image_path",
];

/// Known camelCase -> snake_case mappings from LLM errors.
fn camel_to_snake(key: &str) -> Option<&'static str> {
    match key {
        "filePath" => Some("file_path"),
        "fileName" => Some("file_name"),
        "maxResults" => Some("max_results"),
        "maxLines" => Some("max_lines"),
        "oldContent" => Some("old_content"),
        "newContent" => Some("new_content"),
        "matchAll" => Some("match_all"),
        "createDirs" => Some("create_dirs"),
        "extractText" => Some("extract_text"),
        "maxLength" => Some("max_length"),
        "includeToolCalls" => Some("include_tool_calls"),
        "sessionId" => Some("session_id"),
        "subagentType" => Some("subagent_type"),
        "detailLevel" => Some("detail_level"),
        "cellId" => Some("cell_id"),
        "cellNumber" => Some("cell_number"),
        "cellType" => Some("cell_type"),
        "editMode" => Some("edit_mode"),
        "newSource" => Some("new_source"),
        "notebookPath" => Some("notebook_path"),
        "deepCrawl" => Some("deep_crawl"),
        "crawlStrategy" => Some("crawl_strategy"),
        "maxDepth" => Some("max_depth"),
        "includeExternal" => Some("include_external"),
        "maxPages" => Some("max_pages"),
        "allowedDomains" => Some("allowed_domains"),
        "blockedDomains" => Some("blocked_domains"),
        "urlPatterns" => Some("url_patterns"),
        "symbolName" => Some("symbol_name"),
        "newName" => Some("new_name"),
        "newBody" => Some("new_body"),
        "preserveSignature" => Some("preserve_signature"),
        "includeDeclaration" => Some("include_declaration"),
        "planFilePath" => Some("plan_file_path"),
        "skillName" => Some("skill_name"),
        "taskId" => Some("task_id"),
        "runInBackground" => Some("run_in_background"),
        "toolCallId" => Some("tool_call_id"),
        "multiSelect" => Some("multi_select"),
        "activeForm" => Some("active_form"),
        "viewportWidth" => Some("viewport_width"),
        "viewportHeight" => Some("viewport_height"),
        "timeoutMs" => Some("timeout_ms"),
        "capturePdf" => Some("capture_pdf"),
        "outputPath" => Some("output_path"),
        "imagePath" => Some("image_path"),
        "imageUrl" => Some("image_url"),
        "maxTokens" => Some("max_tokens"),
        _ => None,
    }
}

/// Normalize tool parameters.
///
/// Applies in order:
/// 1. Key normalization (camelCase -> snake_case)
/// 2. Whitespace stripping on string values
/// 3. Path resolution for known path params
///
/// The original map is NOT mutated — a new map is returned.
pub fn normalize_params(
    _tool_name: &str,
    args: HashMap<String, serde_json::Value>,
    working_dir: Option<&str>,
) -> HashMap<String, serde_json::Value> {
    if args.is_empty() {
        return args;
    }

    let mut normalized = HashMap::with_capacity(args.len());

    for (key, mut value) in args {
        // 1. Key normalization
        let new_key = camel_to_snake(&key)
            .map(String::from)
            .unwrap_or(key);

        // 2. Whitespace stripping
        if let Some(s) = value.as_str() {
            let trimmed = s.trim();
            if trimmed.len() != s.len() {
                value = serde_json::Value::String(trimmed.to_string());
            }
        }

        // 3. Path resolution
        if PATH_PARAMS.contains(&new_key.as_str()) {
            if let Some(s) = value.as_str() {
                if !s.is_empty() {
                    let resolved = resolve_path(s, working_dir);
                    value = serde_json::Value::String(resolved);
                }
            }
        }

        normalized.insert(new_key, value);
    }

    normalized
}

/// Resolve a path string to an absolute path.
///
/// - Expands `~` to home directory
/// - Resolves relative paths against `working_dir`
/// - Normalizes `.` and `..` components
/// - Warns for paths outside workspace and home directory
fn resolve_path(path_str: &str, working_dir: Option<&str>) -> String {
    // Expand ~ to home directory
    let expanded = if path_str.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            let rest = path_str.strip_prefix("~/").unwrap_or(&path_str[1..]);
            home.join(rest).to_string_lossy().to_string()
        } else {
            path_str.to_string()
        }
    } else {
        path_str.to_string()
    };

    let path = Path::new(&expanded);

    // If already absolute, just normalize
    let resolved = if path.is_absolute() {
        normalize_path(path)
    } else if let Some(wd) = working_dir {
        normalize_path(&Path::new(wd).join(path))
    } else if let Ok(cwd) = std::env::current_dir() {
        normalize_path(&cwd.join(path))
    } else {
        expanded
    };

    // Workspace guard: warn for paths outside workspace and home
    if let Some(wd) = working_dir {
        let resolved_path = Path::new(&resolved);
        let in_workspace = resolved_path.starts_with(wd);
        let in_home = dirs::home_dir()
            .map(|h| resolved_path.starts_with(&h))
            .unwrap_or(false);
        if !in_workspace && !in_home {
            warn!(
                path = %resolved,
                workspace = %wd,
                "Path is outside workspace and user home"
            );
        }
    }

    resolved
}

/// Normalize a path by resolving `.` and `..` components without filesystem access.
fn normalize_path(path: &Path) -> String {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    let result: PathBuf = components.iter().collect();
    result.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camel_to_snake_known() {
        assert_eq!(camel_to_snake("filePath"), Some("file_path"));
        assert_eq!(camel_to_snake("maxResults"), Some("max_results"));
        assert_eq!(camel_to_snake("sessionId"), Some("session_id"));
    }

    #[test]
    fn test_camel_to_snake_unknown() {
        assert_eq!(camel_to_snake("file_path"), None);
        assert_eq!(camel_to_snake("unknown_key"), None);
    }

    #[test]
    fn test_normalize_params_key_normalization() {
        let mut args = HashMap::new();
        args.insert("filePath".into(), serde_json::json!("/tmp/test.rs"));
        args.insert("maxResults".into(), serde_json::json!(10));

        let result = normalize_params("search", args, None);
        assert!(result.contains_key("file_path"));
        assert!(result.contains_key("max_results"));
        assert!(!result.contains_key("filePath"));
    }

    #[test]
    fn test_normalize_params_whitespace_stripping() {
        let mut args = HashMap::new();
        args.insert("query".into(), serde_json::json!("  hello world  "));

        let result = normalize_params("search", args, None);
        assert_eq!(result["query"], serde_json::json!("hello world"));
    }

    #[test]
    fn test_normalize_params_path_resolution_absolute() {
        let mut args = HashMap::new();
        args.insert("file_path".into(), serde_json::json!("/absolute/path.rs"));

        let result = normalize_params("read_file", args, Some("/workspace"));
        assert_eq!(result["file_path"], serde_json::json!("/absolute/path.rs"));
    }

    #[test]
    fn test_normalize_params_path_resolution_relative() {
        let mut args = HashMap::new();
        args.insert("file_path".into(), serde_json::json!("src/main.rs"));

        let result = normalize_params("read_file", args, Some("/workspace"));
        assert_eq!(
            result["file_path"],
            serde_json::json!("/workspace/src/main.rs")
        );
    }

    #[test]
    fn test_normalize_params_path_with_dotdot() {
        let mut args = HashMap::new();
        args.insert("file_path".into(), serde_json::json!("src/../lib.rs"));

        let result = normalize_params("read_file", args, Some("/workspace"));
        assert_eq!(result["file_path"], serde_json::json!("/workspace/lib.rs"));
    }

    #[test]
    fn test_normalize_params_tilde_expansion() {
        let mut args = HashMap::new();
        args.insert("file_path".into(), serde_json::json!("~/projects/test.rs"));

        let result = normalize_params("read_file", args, Some("/workspace"));
        let resolved = result["file_path"].as_str().unwrap();
        // Should not start with ~ anymore
        assert!(!resolved.starts_with('~'));
        assert!(resolved.ends_with("projects/test.rs"));
    }

    #[test]
    fn test_normalize_params_non_path_param_not_resolved() {
        let mut args = HashMap::new();
        args.insert("query".into(), serde_json::json!("src/main.rs"));

        let result = normalize_params("search", args, Some("/workspace"));
        // "query" is not a path param, should not be resolved
        assert_eq!(result["query"], serde_json::json!("src/main.rs"));
    }

    #[test]
    fn test_normalize_params_empty() {
        let args = HashMap::new();
        let result = normalize_params("test", args, None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_normalize_params_non_string_values_preserved() {
        let mut args = HashMap::new();
        args.insert("count".into(), serde_json::json!(42));
        args.insert("enabled".into(), serde_json::json!(true));
        args.insert("items".into(), serde_json::json!(["a", "b"]));

        let result = normalize_params("test", args, None);
        assert_eq!(result["count"], serde_json::json!(42));
        assert_eq!(result["enabled"], serde_json::json!(true));
        assert_eq!(result["items"], serde_json::json!(["a", "b"]));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path(Path::new("/a/b/../c")), "/a/c");
        assert_eq!(normalize_path(Path::new("/a/./b/c")), "/a/b/c");
        assert_eq!(normalize_path(Path::new("/a/b/c")), "/a/b/c");
    }
}
