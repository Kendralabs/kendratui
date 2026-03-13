//! Delta batch operations for playbook mutations.
//!
//! Mirrors `opendev/core/context_engineering/memory/delta.py`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Type of delta operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DeltaOperationType {
    Add,
    Update,
    Tag,
    Remove,
}

impl fmt::Display for DeltaOperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "ADD"),
            Self::Update => write!(f, "UPDATE"),
            Self::Tag => write!(f, "TAG"),
            Self::Remove => write!(f, "REMOVE"),
        }
    }
}

/// Single mutation to apply to the playbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaOperation {
    #[serde(rename = "type")]
    pub op_type: DeltaOperationType,
    pub section: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bullet_id: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, i64>,
}

impl DeltaOperation {
    /// Create a DeltaOperation from a JSON value.
    pub fn from_json(payload: &serde_json::Value) -> Option<Self> {
        let op_type_str = payload["type"].as_str()?.to_uppercase();
        let op_type = match op_type_str.as_str() {
            "ADD" => DeltaOperationType::Add,
            "UPDATE" => DeltaOperationType::Update,
            "TAG" => DeltaOperationType::Tag,
            "REMOVE" => DeltaOperationType::Remove,
            _ => return None,
        };

        let section = payload
            .get("section")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let content = payload
            .get("content")
            .and_then(|v| v.as_str())
            .map(String::from);

        let bullet_id = payload
            .get("bullet_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let mut metadata = HashMap::new();
        if let Some(meta_obj) = payload.get("metadata").and_then(|v| v.as_object()) {
            let valid_tags: &[&str] = if op_type == DeltaOperationType::Tag {
                &["helpful", "harmful", "neutral"]
            } else {
                // For non-TAG operations, accept all keys
                &[]
            };

            for (k, v) in meta_obj {
                if op_type == DeltaOperationType::Tag && !valid_tags.contains(&k.as_str()) {
                    continue;
                }
                if let Some(n) = v.as_i64() {
                    metadata.insert(k.clone(), n);
                }
            }
        }

        Some(Self {
            op_type,
            section,
            content,
            bullet_id,
            metadata,
        })
    }

    /// Convert to JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        let mut data = serde_json::json!({
            "type": self.op_type,
            "section": self.section,
        });
        if let Some(ref content) = self.content {
            data["content"] = serde_json::Value::String(content.clone());
        }
        if let Some(ref bullet_id) = self.bullet_id {
            data["bullet_id"] = serde_json::Value::String(bullet_id.clone());
        }
        if !self.metadata.is_empty() {
            data["metadata"] = serde_json::to_value(&self.metadata).unwrap_or_default();
        }
        data
    }
}

/// Bundle of curator reasoning and operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaBatch {
    pub reasoning: String,
    #[serde(default)]
    pub operations: Vec<DeltaOperation>,
}

impl DeltaBatch {
    /// Create a DeltaBatch from a JSON value.
    pub fn from_json(payload: &serde_json::Value) -> Self {
        let reasoning = payload
            .get("reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut operations = Vec::new();
        if let Some(ops_array) = payload.get("operations").and_then(|v| v.as_array()) {
            for item in ops_array {
                if let Some(op) = DeltaOperation::from_json(item) {
                    operations.push(op);
                }
            }
        }

        Self {
            reasoning,
            operations,
        }
    }

    /// Convert to JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "reasoning": self.reasoning,
            "operations": self.operations.iter().map(|op| op.to_json()).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_operation_from_json() {
        let json = serde_json::json!({
            "type": "ADD",
            "section": "file_operations",
            "content": "Always read before write",
            "bullet_id": "fo-001",
            "metadata": {"helpful": 3}
        });
        let op = DeltaOperation::from_json(&json).unwrap();
        assert_eq!(op.op_type, DeltaOperationType::Add);
        assert_eq!(op.section, "file_operations");
        assert_eq!(op.content.as_deref(), Some("Always read before write"));
        assert_eq!(op.bullet_id.as_deref(), Some("fo-001"));
    }

    #[test]
    fn test_delta_operation_tag_filters_metadata() {
        let json = serde_json::json!({
            "type": "TAG",
            "section": "testing",
            "bullet_id": "t-001",
            "metadata": {"helpful": 1, "invalid_key": 5, "harmful": 0}
        });
        let op = DeltaOperation::from_json(&json).unwrap();
        assert_eq!(op.metadata.len(), 2);
        assert_eq!(op.metadata.get("helpful"), Some(&1));
        assert_eq!(op.metadata.get("harmful"), Some(&0));
        assert!(!op.metadata.contains_key("invalid_key"));
    }

    #[test]
    fn test_delta_operation_roundtrip() {
        let op = DeltaOperation {
            op_type: DeltaOperationType::Update,
            section: "code_nav".to_string(),
            content: Some("Search then read".to_string()),
            bullet_id: Some("cn-001".to_string()),
            metadata: HashMap::new(),
        };
        let json = op.to_json();
        let restored = DeltaOperation::from_json(&json).unwrap();
        assert_eq!(restored.op_type, DeltaOperationType::Update);
        assert_eq!(restored.content.as_deref(), Some("Search then read"));
    }

    #[test]
    fn test_delta_operation_invalid_type() {
        let json = serde_json::json!({
            "type": "INVALID",
            "section": "x"
        });
        assert!(DeltaOperation::from_json(&json).is_none());
    }

    #[test]
    fn test_delta_batch_from_json() {
        let json = serde_json::json!({
            "reasoning": "Updating playbook based on feedback",
            "operations": [
                {"type": "ADD", "section": "testing", "content": "Run tests after changes"},
                {"type": "TAG", "section": "nav", "bullet_id": "n-001", "metadata": {"helpful": 1}},
                {"type": "REMOVE", "section": "old", "bullet_id": "old-001"}
            ]
        });
        let batch = DeltaBatch::from_json(&json);
        assert_eq!(batch.reasoning, "Updating playbook based on feedback");
        assert_eq!(batch.operations.len(), 3);
        assert_eq!(batch.operations[0].op_type, DeltaOperationType::Add);
        assert_eq!(batch.operations[1].op_type, DeltaOperationType::Tag);
        assert_eq!(batch.operations[2].op_type, DeltaOperationType::Remove);
    }

    #[test]
    fn test_delta_batch_roundtrip() {
        let batch = DeltaBatch {
            reasoning: "test reasoning".to_string(),
            operations: vec![DeltaOperation {
                op_type: DeltaOperationType::Add,
                section: "testing".to_string(),
                content: Some("Test content".to_string()),
                bullet_id: None,
                metadata: HashMap::new(),
            }],
        };
        let json = batch.to_json();
        let restored = DeltaBatch::from_json(&json);
        assert_eq!(restored.reasoning, "test reasoning");
        assert_eq!(restored.operations.len(), 1);
    }

    #[test]
    fn test_delta_batch_empty_operations() {
        let json = serde_json::json!({"reasoning": "no ops"});
        let batch = DeltaBatch::from_json(&json);
        assert_eq!(batch.reasoning, "no ops");
        assert!(batch.operations.is_empty());
    }

    #[test]
    fn test_operation_type_display() {
        assert_eq!(DeltaOperationType::Add.to_string(), "ADD");
        assert_eq!(DeltaOperationType::Update.to_string(), "UPDATE");
        assert_eq!(DeltaOperationType::Tag.to_string(), "TAG");
        assert_eq!(DeltaOperationType::Remove.to_string(), "REMOVE");
    }

    #[test]
    fn test_operation_type_serde() {
        let json = serde_json::to_string(&DeltaOperationType::Add).unwrap();
        assert_eq!(json, r#""ADD""#);
        let deserialized: DeltaOperationType = serde_json::from_str(r#""TAG""#).unwrap();
        assert_eq!(deserialized, DeltaOperationType::Tag);
    }
}
