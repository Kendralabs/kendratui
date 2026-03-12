//! Shared API response models used by Web routes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::message::ToolCall;

/// Serialization view of a ToolCall for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub id: String,
    pub name: String,
    pub parameters: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested_tool_calls: Option<Vec<ToolCallResponse>>,
}

/// Response model for a chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_trace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

/// Session information model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    pub id: String,
    pub working_dir: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub has_session_model: bool,
}

/// Recursively convert a ToolCall to ToolCallResponse.
///
/// Handles nested calls and coerces non-string results to JSON strings.
pub fn tool_call_to_response(tc: &ToolCall) -> ToolCallResponse {
    let nested = if tc.nested_tool_calls.is_empty() {
        None
    } else {
        Some(
            tc.nested_tool_calls
                .iter()
                .map(tool_call_to_response)
                .collect(),
        )
    };

    let result = tc.result.as_ref().map(|r| {
        if let serde_json::Value::String(s) = r {
            s.clone()
        } else {
            serde_json::to_string(r).unwrap_or_else(|_| r.to_string())
        }
    });

    ToolCallResponse {
        id: tc.id.clone(),
        name: tc.name.clone(),
        parameters: tc.parameters.clone(),
        result,
        error: tc.error.clone(),
        result_summary: tc.result_summary.clone(),
        approved: Some(tc.approved),
        nested_tool_calls: nested,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_tool_call_to_response() {
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "read_file".to_string(),
            parameters: HashMap::new(),
            result: Some(serde_json::Value::String("file contents".to_string())),
            result_summary: Some("Read 10 lines".to_string()),
            timestamp: Utc::now(),
            approved: true,
            error: None,
            nested_tool_calls: vec![],
        };

        let response = tool_call_to_response(&tc);
        assert_eq!(response.id, "tc-1");
        assert_eq!(response.name, "read_file");
        assert_eq!(response.result.as_deref(), Some("file contents"));
        assert_eq!(response.result_summary.as_deref(), Some("Read 10 lines"));
        assert!(response.nested_tool_calls.is_none());
    }

    #[test]
    fn test_tool_call_to_response_with_nested() {
        let nested = ToolCall {
            id: "nested-1".to_string(),
            name: "bash".to_string(),
            parameters: HashMap::new(),
            result: Some(serde_json::json!({"exit_code": 0})),
            result_summary: None,
            timestamp: Utc::now(),
            approved: true,
            error: None,
            nested_tool_calls: vec![],
        };

        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "agent".to_string(),
            parameters: HashMap::new(),
            result: Some(serde_json::Value::String("done".to_string())),
            result_summary: None,
            timestamp: Utc::now(),
            approved: true,
            error: None,
            nested_tool_calls: vec![nested],
        };

        let response = tool_call_to_response(&tc);
        assert!(response.nested_tool_calls.is_some());
        let nested_responses = response.nested_tool_calls.unwrap();
        assert_eq!(nested_responses.len(), 1);
        assert_eq!(nested_responses[0].name, "bash");
        // Non-string result should be serialized to JSON string
        assert!(nested_responses[0]
            .result
            .as_ref()
            .unwrap()
            .contains("exit_code"));
    }

    #[test]
    fn test_session_response_roundtrip() {
        let resp = SessionResponse {
            id: "abc123".to_string(),
            working_dir: "/home/user/project".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T01:00:00Z".to_string(),
            message_count: 10,
            total_tokens: 5000,
            title: Some("Test session".to_string()),
            has_session_model: false,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: SessionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "abc123");
        assert_eq!(deserialized.message_count, 10);
    }
}
