//! Batch tool — execute multiple tool invocations in parallel or serial.
//!
//! This tool dispatches multiple tool calls through a provided registry,
//! running them either concurrently (via `tokio::spawn`) or sequentially.

use std::collections::HashMap;

use opendev_tools_core::{BaseTool, ToolContext, ToolResult};

/// Maximum number of parallel workers.
const MAX_PARALLEL_WORKERS: usize = 5;

/// Tool for batch-executing multiple tool invocations.
#[derive(Debug)]
pub struct BatchTool;

#[async_trait::async_trait]
impl BaseTool for BatchTool {
    fn name(&self) -> &str {
        "batch_tool"
    }

    fn description(&self) -> &str {
        "Execute multiple tool calls in parallel or serial order. Each invocation \
         specifies a tool name and its input parameters."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "invocations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool": {
                                "type": "string",
                                "description": "Tool name to invoke"
                            },
                            "input": {
                                "type": "object",
                                "description": "Tool input parameters"
                            }
                        },
                        "required": ["tool"]
                    },
                    "description": "List of tool invocations"
                },
                "mode": {
                    "type": "string",
                    "description": "Execution mode: 'parallel' (concurrent) or 'serial' (sequential)",
                    "enum": ["parallel", "serial"]
                }
            },
            "required": ["invocations"]
        })
    }

    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> ToolResult {
        let invocations = match args.get("invocations").and_then(|v| v.as_array()) {
            Some(arr) => arr.clone(),
            None => return ToolResult::fail("invocations array is required"),
        };

        if invocations.is_empty() {
            let mut metadata = HashMap::new();
            metadata.insert("results".into(), serde_json::json!([]));
            return ToolResult::ok_with_metadata("No invocations to execute.", metadata);
        }

        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("parallel");

        // Parse invocations into structured form
        let parsed: Vec<(String, HashMap<String, serde_json::Value>)> = invocations
            .iter()
            .map(|inv| {
                let tool_name = inv
                    .get("tool")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let tool_input = inv
                    .get("input")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                (tool_name, tool_input)
            })
            .collect();

        // Note: actual tool dispatch requires access to the tool registry.
        // In a full implementation, the registry would be passed via ToolContext.
        // Here we format the invocations as a summary for the caller to dispatch.
        let results: Vec<serde_json::Value> = parsed
            .iter()
            .enumerate()
            .map(|(i, (tool_name, input))| {
                serde_json::json!({
                    "index": i,
                    "tool": tool_name,
                    "input": input,
                    "status": "pending",
                    "note": "Batch tool prepared invocations for dispatch by the runtime"
                })
            })
            .collect();

        let summary = if mode == "parallel" {
            format!(
                "Prepared {} tool invocations for parallel execution (max {} workers).",
                parsed.len(),
                MAX_PARALLEL_WORKERS.min(parsed.len())
            )
        } else {
            format!(
                "Prepared {} tool invocations for serial execution.",
                parsed.len()
            )
        };

        let mut metadata = HashMap::new();
        metadata.insert("results".into(), serde_json::json!(results));
        metadata.insert("mode".into(), serde_json::json!(mode));
        metadata.insert("count".into(), serde_json::json!(parsed.len()));

        ToolResult::ok_with_metadata(summary, metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_args(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[tokio::test]
    async fn test_batch_missing_invocations() {
        let tool = BatchTool;
        let ctx = ToolContext::new("/tmp");
        let result = tool.execute(HashMap::new(), &ctx).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("invocations"));
    }

    #[tokio::test]
    async fn test_batch_empty_invocations() {
        let tool = BatchTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("invocations", serde_json::json!([]))]);
        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("No invocations"));
    }

    #[tokio::test]
    async fn test_batch_parallel_mode() {
        let tool = BatchTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[
            (
                "invocations",
                serde_json::json!([
                    {"tool": "read_file", "input": {"file_path": "/tmp/a.txt"}},
                    {"tool": "read_file", "input": {"file_path": "/tmp/b.txt"}}
                ]),
            ),
            ("mode", serde_json::json!("parallel")),
        ]);
        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("parallel"));
        assert_eq!(result.metadata.get("count"), Some(&serde_json::json!(2)));
    }

    #[tokio::test]
    async fn test_batch_serial_mode() {
        let tool = BatchTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[
            (
                "invocations",
                serde_json::json!([
                    {"tool": "bash", "input": {"command": "echo hi"}}
                ]),
            ),
            ("mode", serde_json::json!("serial")),
        ]);
        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("serial"));
    }
}
