//! Ask user tool — pose structured questions to the user via a callback.

use std::collections::HashMap;

use opendev_tools_core::{BaseTool, ToolContext, ToolResult};

/// Tool for asking the user a question during agent execution.
///
/// In a real deployment the answer comes from a UI callback.
/// Here the tool formats the question; the runtime is responsible
/// for routing it to the user and injecting the response.
#[derive(Debug)]
pub struct AskUserTool;

#[async_trait::async_trait]
impl BaseTool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn description(&self) -> &str {
        "Ask the user a question and wait for their response. Use when clarification is needed."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask the user"
                },
                "options": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of choices for the user"
                },
                "default": {
                    "type": "string",
                    "description": "Default answer if user provides none"
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        _ctx: &ToolContext,
    ) -> ToolResult {
        let question = match args.get("question").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return ToolResult::fail("question is required"),
        };

        let options: Vec<String> = args
            .get("options")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let default = args.get("default").and_then(|v| v.as_str());

        let mut output = format!("Question: {question}");
        if !options.is_empty() {
            output.push_str("\nOptions:");
            for (i, opt) in options.iter().enumerate() {
                output.push_str(&format!("\n  {}. {opt}", i + 1));
            }
        }
        if let Some(d) = default {
            output.push_str(&format!("\nDefault: {d}"));
        }

        // The tool signals that it needs user input.
        // The runtime intercepts this result and routes it to the UI.
        let mut metadata = HashMap::new();
        metadata.insert("requires_input".into(), serde_json::json!(true));
        metadata.insert("question".into(), serde_json::json!(question));
        if !options.is_empty() {
            metadata.insert("options".into(), serde_json::json!(options));
        }
        if let Some(d) = default {
            metadata.insert("default".into(), serde_json::json!(d));
        }

        ToolResult::ok_with_metadata(output, metadata)
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
    async fn test_ask_user_basic() {
        let tool = AskUserTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[("question", serde_json::json!("What language?"))]);
        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        assert!(result.output.unwrap().contains("What language?"));
        assert_eq!(
            result.metadata.get("requires_input"),
            Some(&serde_json::json!(true))
        );
    }

    #[tokio::test]
    async fn test_ask_user_with_options() {
        let tool = AskUserTool;
        let ctx = ToolContext::new("/tmp");
        let args = make_args(&[
            ("question", serde_json::json!("Pick one")),
            ("options", serde_json::json!(["A", "B", "C"])),
        ]);
        let result = tool.execute(args, &ctx).await;
        assert!(result.success);
        let out = result.output.unwrap();
        assert!(out.contains("1. A"));
        assert!(out.contains("2. B"));
        assert!(out.contains("3. C"));
    }

    #[tokio::test]
    async fn test_ask_user_missing_question() {
        let tool = AskUserTool;
        let ctx = ToolContext::new("/tmp");
        let result = tool.execute(HashMap::new(), &ctx).await;
        assert!(!result.success);
    }
}
