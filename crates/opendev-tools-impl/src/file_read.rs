//! Read file tool — reads file contents with optional line ranges and binary detection.

use std::collections::HashMap;
use std::path::Path;

use opendev_tools_core::{BaseTool, ToolContext, ToolResult};

/// Tool for reading file contents.
#[derive(Debug)]
pub struct FileReadTool;

impl FileReadTool {
    /// Maximum file size we'll read (10 MB).
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

    /// Maximum number of lines to return by default.
    const DEFAULT_MAX_LINES: usize = 2000;

    /// Maximum line length before truncation.
    const MAX_LINE_LENGTH: usize = 2000;
}

#[async_trait::async_trait]
impl BaseTool for FileReadTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Supports line ranges and detects binary files."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-based)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> ToolResult {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::fail("file_path is required"),
        };

        let offset = args
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(1);

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(Self::DEFAULT_MAX_LINES);

        let path = Path::new(file_path);

        if !path.exists() {
            return ToolResult::fail(format!("File not found: {file_path}"));
        }

        if !path.is_file() {
            return ToolResult::fail(format!("Not a file: {file_path}"));
        }

        // Check file size
        match std::fs::metadata(path) {
            Ok(meta) => {
                if meta.len() > Self::MAX_FILE_SIZE {
                    return ToolResult::fail(format!(
                        "File too large: {} bytes (max {} bytes)",
                        meta.len(),
                        Self::MAX_FILE_SIZE
                    ));
                }
            }
            Err(e) => return ToolResult::fail(format!("Cannot read file metadata: {e}")),
        }

        // Check for binary content
        match std::fs::read(path) {
            Ok(bytes) => {
                if is_binary(&bytes) {
                    return ToolResult::fail(format!(
                        "Binary file detected: {file_path} ({} bytes). Use a specialized tool for binary files.",
                        bytes.len()
                    ));
                }

                let content = String::from_utf8_lossy(&bytes);
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                // Apply offset (1-based) and limit
                let start = if offset > 0 { offset - 1 } else { 0 };
                let end = (start + limit).min(total_lines);

                if start >= total_lines {
                    return ToolResult::fail(format!(
                        "Offset {offset} is beyond end of file ({total_lines} lines)"
                    ));
                }

                let mut output = String::new();
                for (i, line) in lines[start..end].iter().enumerate() {
                    let line_num = start + i + 1;
                    let truncated = if line.len() > Self::MAX_LINE_LENGTH {
                        format!("{}...", &line[..Self::MAX_LINE_LENGTH])
                    } else {
                        line.to_string()
                    };
                    output.push_str(&format!("{line_num:>6}\t{truncated}\n"));
                }

                let mut metadata = HashMap::new();
                metadata.insert(
                    "total_lines".into(),
                    serde_json::json!(total_lines),
                );
                metadata.insert(
                    "lines_shown".into(),
                    serde_json::json!(end - start),
                );

                ToolResult::ok_with_metadata(output, metadata)
            }
            Err(e) => ToolResult::fail(format!("Failed to read file: {e}")),
        }
    }
}

/// Check if content appears to be binary by looking for null bytes
/// in the first 8192 bytes.
fn is_binary(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(8192);
    bytes[..check_len].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_args(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[tokio::test]
    async fn test_read_file_basic() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "line one").unwrap();
        writeln!(tmp, "line two").unwrap();
        writeln!(tmp, "line three").unwrap();

        let tool = FileReadTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("file_path", serde_json::json!(tmp.path().to_str().unwrap()))]);
        let result = tool.execute(args, &ctx).await;

        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("line one"));
        assert!(output.contains("line two"));
        assert!(output.contains("line three"));
    }

    #[tokio::test]
    async fn test_read_file_with_offset_and_limit() {
        let mut tmp = NamedTempFile::new().unwrap();
        for i in 1..=10 {
            writeln!(tmp, "line {i}").unwrap();
        }

        let tool = FileReadTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[
            ("file_path", serde_json::json!(tmp.path().to_str().unwrap())),
            ("offset", serde_json::json!(3)),
            ("limit", serde_json::json!(2)),
        ]);
        let result = tool.execute(args, &ctx).await;

        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.contains("line 3"));
        assert!(output.contains("line 4"));
        assert!(!output.contains("line 5"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let tool = FileReadTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("file_path", serde_json::json!("/nonexistent/file.txt"))]);
        let result = tool.execute(args, &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_read_binary_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&[0u8, 1, 2, 3, 0, 5]).unwrap();

        let tool = FileReadTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("file_path", serde_json::json!(tmp.path().to_str().unwrap()))]);
        let result = tool.execute(args, &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Binary"));
    }

    #[tokio::test]
    async fn test_missing_file_path() {
        let tool = FileReadTool;
        let ctx = ToolContext::new("/tmp");
        let result = tool.execute(HashMap::new(), &ctx).await;
        assert!(!result.success);
    }

    #[test]
    fn test_is_binary() {
        assert!(is_binary(&[0u8, 1, 2]));
        assert!(!is_binary(b"hello world\n"));
    }
}
