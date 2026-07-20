use super::*;
use serde_json::json;

#[test]
fn test_parse_chat_completions_sse_reasoning_content() {
    let data = json!({
        "choices": [
            {
                "index": 0,
                "delta": {
                    "reasoning_content": "Thinking about the user's question..."
                }
            }
        ]
    });

    let event = ChatCompletionsAdapter::parse_chat_completions_sse(&data);
    assert!(event.is_some());
    match event.unwrap() {
        crate::streaming::StreamEvent::ReasoningDelta(text) => {
            assert_eq!(text, "Thinking about the user's question...");
        }
        other => panic!("Expected ReasoningDelta, got {:?}", other),
    }
}

#[test]
fn test_parse_chat_completions_sse_reasoning() {
    let data = json!({
        "choices": [
            {
                "index": 0,
                "delta": {
                    "reasoning": "Another thinking field value"
                }
            }
        ]
    });

    let event = ChatCompletionsAdapter::parse_chat_completions_sse(&data);
    assert!(event.is_some());
    match event.unwrap() {
        crate::streaming::StreamEvent::ReasoningDelta(text) => {
            assert_eq!(text, "Another thinking field value");
        }
        other => panic!("Expected ReasoningDelta, got {:?}", other),
    }
}

#[test]
fn test_parse_chat_completions_sse_standard_content() {
    let data = json!({
        "choices": [
            {
                "index": 0,
                "delta": {
                    "content": "Hello world!"
                }
            }
        ]
    });

    let event = ChatCompletionsAdapter::parse_chat_completions_sse(&data);
    assert!(event.is_some());
    match event.unwrap() {
        crate::streaming::StreamEvent::TextDelta(text) => {
            assert_eq!(text, "Hello world!");
        }
        other => panic!("Expected TextDelta, got {:?}", other),
    }
}
