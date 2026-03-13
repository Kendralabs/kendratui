//! List files tool — glob-based file listing.

use std::collections::HashMap;
use std::path::Path;

use opendev_tools_core::{BaseTool, ToolContext, ToolResult};

/// Tool for listing files using glob patterns.
#[derive(Debug)]
pub struct FileListTool;

impl FileListTool {
    /// Maximum number of files to return.
    const MAX_RESULTS: usize = 500;
}

#[async_trait::async_trait]
impl BaseTool for FileListTool {
    fn name(&self) -> &str {
        "list_files"
    }

    fn description(&self) -> &str {
        "List files matching a glob pattern. Returns file paths sorted by modification time."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g., \"**/*.rs\", \"src/**/*.ts\")"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (defaults to working directory)"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum directory depth to recurse into (0 = base dir only)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        ctx: &ToolContext,
    ) -> ToolResult {
        let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::fail("pattern is required"),
        };

        let base_dir = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| {
                let path = Path::new(p);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    ctx.working_dir.join(path)
                }
            })
            .unwrap_or_else(|| ctx.working_dir.clone());

        let max_depth = args
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        if !base_dir.exists() {
            return ToolResult::fail(format!("Directory not found: {}", base_dir.display()));
        }

        // Build full glob pattern
        let full_pattern = base_dir.join(pattern);
        let full_pattern_str = full_pattern.to_string_lossy();

        let glob_opts = glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        let entries = match glob::glob_with(&full_pattern_str, glob_opts) {
            Ok(paths) => paths,
            Err(e) => return ToolResult::fail(format!("Invalid glob pattern: {e}")),
        };

        let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();

        for entry in entries {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        // Apply max_depth filter: count components relative to base_dir
                        if let Some(depth) = max_depth
                            && let Ok(rel) = path.strip_prefix(&base_dir)
                        {
                            // Depth is number of parent directories (components - 1 for the file itself)
                            let rel_depth = rel.components().count().saturating_sub(1);
                            if rel_depth > depth {
                                continue;
                            }
                        }
                        let mtime = path
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        files.push((path, mtime));
                    }
                }
                Err(e) => {
                    tracing::debug!("Glob entry error: {}", e);
                }
            }
        }

        // Sort by modification time (most recent first)
        files.sort_by(|a, b| b.1.cmp(&a.1));

        let total = files.len();
        let truncated = total > Self::MAX_RESULTS;
        let files = &files[..total.min(Self::MAX_RESULTS)];

        if files.is_empty() {
            return ToolResult::ok(format!(
                "No files found matching '{pattern}' in {}",
                base_dir.display()
            ));
        }

        let mut output = String::new();
        for (path, _) in files {
            // Try to make path relative to base_dir
            let display = path.strip_prefix(&base_dir).unwrap_or(path).display();
            output.push_str(&format!("{display}\n"));
        }

        if truncated {
            output.push_str(&format!(
                "\n... and {} more files (showing first {})\n",
                total - Self::MAX_RESULTS,
                Self::MAX_RESULTS
            ));
        }

        let mut metadata = HashMap::new();
        metadata.insert("total_files".into(), serde_json::json!(total));
        metadata.insert("truncated".into(), serde_json::json!(truncated));

        ToolResult::ok_with_metadata(output, metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_args(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[tokio::test]
    async fn test_list_files_basic() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.rs"), "").unwrap();
        fs::write(tmp.path().join("b.rs"), "").unwrap();
        fs::write(tmp.path().join("c.txt"), "").unwrap();

        let tool = FileListTool;
        let ctx = ToolContext::new(tmp.path());
        let args = make_args(&[("pattern", serde_json::json!("*.rs"))]);

        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("a.rs"));
        assert!(output.contains("b.rs"));
        assert!(!output.contains("c.txt"));
    }

    #[tokio::test]
    async fn test_list_files_recursive() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("src/sub")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "").unwrap();
        fs::write(tmp.path().join("src/sub/lib.rs"), "").unwrap();

        let tool = FileListTool;
        let ctx = ToolContext::new(tmp.path());
        let args = make_args(&[("pattern", serde_json::json!("**/*.rs"))]);

        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("main.rs"));
        assert!(output.contains("lib.rs"));
    }

    #[tokio::test]
    async fn test_list_files_max_depth() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("a/b/c")).unwrap();
        fs::write(tmp.path().join("top.rs"), "").unwrap();
        fs::write(tmp.path().join("a/mid.rs"), "").unwrap();
        fs::write(tmp.path().join("a/b/deep.rs"), "").unwrap();
        fs::write(tmp.path().join("a/b/c/deeper.rs"), "").unwrap();

        let tool = FileListTool;
        let ctx = ToolContext::new(tmp.path());

        // max_depth 0 = only files in base dir
        let args = make_args(&[
            ("pattern", serde_json::json!("**/*.rs")),
            ("max_depth", serde_json::json!(0)),
        ]);
        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("top.rs"));
        assert!(!output.contains("mid.rs"));
        assert!(!output.contains("deep.rs"));

        // max_depth 1 = base dir + one level
        let args = make_args(&[
            ("pattern", serde_json::json!("**/*.rs")),
            ("max_depth", serde_json::json!(1)),
        ]);
        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("top.rs"));
        assert!(output.contains("mid.rs"));
        assert!(!output.contains("deep.rs"));

        // No max_depth = all files
        let args = make_args(&[("pattern", serde_json::json!("**/*.rs"))]);
        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("top.rs"));
        assert!(output.contains("mid.rs"));
        assert!(output.contains("deep.rs"));
        assert!(output.contains("deeper.rs"));
    }

    #[tokio::test]
    async fn test_list_files_no_matches() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "").unwrap();

        let tool = FileListTool;
        let ctx = ToolContext::new(tmp.path());
        let args = make_args(&[("pattern", serde_json::json!("*.rs"))]);

        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("No files found"));
    }
}
