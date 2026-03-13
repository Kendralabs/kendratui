//! Session-level cost tracking for LLM API usage.
//!
//! Uses ModelInfo pricing ($ per million tokens) to compute cost from
//! the usage dict returned by each LLM API call.
//!
//! Ported from `opendev/core/runtime/cost_tracker.py`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Token usage from a single LLM call.
///
/// Maps to the usage dict returned by OpenAI/Anthropic APIs.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Anthropic prompt-caching: tokens read from cache.
    pub cache_read_input_tokens: u64,
    /// Anthropic prompt-caching: tokens written to cache.
    pub cache_creation_input_tokens: u64,
}

impl TokenUsage {
    /// Parse from a serde_json::Value (the `usage` field in API responses).
    pub fn from_json(value: &serde_json::Value) -> Self {
        Self {
            prompt_tokens: value
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            completion_tokens: value
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            cache_read_input_tokens: value
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            cache_creation_input_tokens: value
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        }
    }
}

/// Pricing info needed for cost computation.
///
/// Prices are in USD per 1 million tokens.
#[derive(Debug, Clone)]
pub struct PricingInfo {
    pub input_price_per_million: f64,
    pub output_price_per_million: f64,
}

/// Tracks cumulative token usage and estimated cost for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostTracker {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub call_count: u64,
}

/// Anthropic charges higher rates for prompts over 200K tokens.
const OVER_200K_THRESHOLD: u64 = 200_000;
const OVER_200K_MULTIPLIER: f64 = 1.5;
/// Cache read tokens are typically 10% of input price.
const CACHE_READ_DISCOUNT: f64 = 0.1;

impl CostTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
            call_count: 0,
        }
    }

    /// Record token usage from a single LLM call.
    ///
    /// Returns the incremental cost for this call in USD.
    pub fn record_usage(&mut self, usage: &TokenUsage, pricing: Option<&PricingInfo>) -> f64 {
        self.total_input_tokens += usage.prompt_tokens;
        self.total_output_tokens += usage.completion_tokens;
        self.call_count += 1;

        let incremental_cost = if let Some(p) = pricing {
            if p.input_price_per_million > 0.0 || p.output_price_per_million > 0.0 {
                self.compute_cost(usage, p)
            } else {
                0.0
            }
        } else {
            0.0
        };

        self.total_cost_usd += incremental_cost;

        debug!(
            call = self.call_count,
            input = usage.prompt_tokens,
            output = usage.completion_tokens,
            cost_delta = format!("${:.6}", incremental_cost),
            cost_total = format!("${:.6}", self.total_cost_usd),
            "cost_tracker: recorded usage"
        );

        incremental_cost
    }

    fn compute_cost(&self, usage: &TokenUsage, pricing: &PricingInfo) -> f64 {
        // Handle tiered pricing for inputs over 200K tokens
        let input_cost = if usage.prompt_tokens > OVER_200K_THRESHOLD {
            let base = (OVER_200K_THRESHOLD as f64 / 1_000_000.0) * pricing.input_price_per_million;
            let over = ((usage.prompt_tokens - OVER_200K_THRESHOLD) as f64 / 1_000_000.0)
                * (pricing.input_price_per_million * OVER_200K_MULTIPLIER);
            base + over
        } else {
            (usage.prompt_tokens as f64 / 1_000_000.0) * pricing.input_price_per_million
        };

        // Cache read tokens at 10% of input price
        let cache_cost = if usage.cache_read_input_tokens > 0 {
            (usage.cache_read_input_tokens as f64 / 1_000_000.0)
                * (pricing.input_price_per_million * CACHE_READ_DISCOUNT)
        } else {
            0.0
        };

        let output_cost =
            (usage.completion_tokens as f64 / 1_000_000.0) * pricing.output_price_per_million;

        input_cost + output_cost + cache_cost
    }

    /// Format the total cost for display.
    pub fn format_cost(&self) -> String {
        if self.total_cost_usd < 0.01 {
            format!("${:.4}", self.total_cost_usd)
        } else {
            format!("${:.2}", self.total_cost_usd)
        }
    }

    /// Export cost data for session metadata persistence.
    pub fn to_metadata(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        map.insert(
            "total_cost_usd".into(),
            serde_json::json!(round_f64(self.total_cost_usd, 6)),
        );
        map.insert(
            "total_input_tokens".into(),
            serde_json::json!(self.total_input_tokens),
        );
        map.insert(
            "total_output_tokens".into(),
            serde_json::json!(self.total_output_tokens),
        );
        map.insert("api_call_count".into(), serde_json::json!(self.call_count));
        map
    }

    /// Restore cost state from session metadata (for `--continue` sessions).
    pub fn restore_from_metadata(&mut self, metadata: &serde_json::Value) {
        let cost_data = match metadata.get("cost_tracking") {
            Some(v) => v,
            None => return,
        };

        self.total_cost_usd = cost_data
            .get("total_cost_usd")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        self.total_input_tokens = cost_data
            .get("total_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        self.total_output_tokens = cost_data
            .get("total_output_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        self.call_count = cost_data
            .get("api_call_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        debug!(
            cost = format!("${:.6}", self.total_cost_usd),
            calls = self.call_count,
            "cost_tracker: restored from metadata"
        );
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Round an f64 to N decimal places.
fn round_f64(value: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pricing() -> PricingInfo {
        PricingInfo {
            input_price_per_million: 3.0,   // $3 per 1M input tokens
            output_price_per_million: 15.0, // $15 per 1M output tokens
        }
    }

    #[test]
    fn test_basic_cost_tracking() {
        let mut tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            ..Default::default()
        };
        let cost = tracker.record_usage(&usage, Some(&test_pricing()));

        // input: 1000/1M * $3 = $0.003
        // output: 500/1M * $15 = $0.0075
        let expected = 0.003 + 0.0075;
        assert!((cost - expected).abs() < 1e-9);
        assert_eq!(tracker.total_input_tokens, 1000);
        assert_eq!(tracker.total_output_tokens, 500);
        assert_eq!(tracker.call_count, 1);
    }

    #[test]
    fn test_tiered_pricing_over_200k() {
        let mut tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 250_000,
            completion_tokens: 100,
            ..Default::default()
        };
        let cost = tracker.record_usage(&usage, Some(&test_pricing()));

        // First 200K at base rate: 200000/1M * $3 = $0.60
        // Remaining 50K at 1.5x: 50000/1M * $4.5 = $0.225
        // Output: 100/1M * $15 = $0.0015
        let expected = 0.60 + 0.225 + 0.0015;
        assert!((cost - expected).abs() < 1e-9);
    }

    #[test]
    fn test_cache_read_tokens() {
        let mut tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 100,
            cache_read_input_tokens: 5000,
            ..Default::default()
        };
        let cost = tracker.record_usage(&usage, Some(&test_pricing()));

        // input: 1000/1M * $3 = $0.003
        // cache: 5000/1M * $0.3 = $0.0015
        // output: 100/1M * $15 = $0.0015
        let expected = 0.003 + 0.0015 + 0.0015;
        assert!((cost - expected).abs() < 1e-9);
    }

    #[test]
    fn test_no_pricing_tracks_tokens_only() {
        let mut tracker = CostTracker::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            ..Default::default()
        };
        let cost = tracker.record_usage(&usage, None);
        assert_eq!(cost, 0.0);
        assert_eq!(tracker.total_input_tokens, 1000);
        assert_eq!(tracker.total_output_tokens, 500);
        assert_eq!(tracker.total_cost_usd, 0.0);
    }

    #[test]
    fn test_cumulative_tracking() {
        let mut tracker = CostTracker::new();
        let pricing = test_pricing();
        let usage1 = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 200,
            ..Default::default()
        };
        let usage2 = TokenUsage {
            prompt_tokens: 2000,
            completion_tokens: 300,
            ..Default::default()
        };
        tracker.record_usage(&usage1, Some(&pricing));
        tracker.record_usage(&usage2, Some(&pricing));

        assert_eq!(tracker.total_input_tokens, 3000);
        assert_eq!(tracker.total_output_tokens, 500);
        assert_eq!(tracker.call_count, 2);
    }

    #[test]
    fn test_format_cost_small() {
        let mut tracker = CostTracker::new();
        tracker.total_cost_usd = 0.005;
        assert_eq!(tracker.format_cost(), "$0.0050");
    }

    #[test]
    fn test_format_cost_large() {
        let mut tracker = CostTracker::new();
        tracker.total_cost_usd = 1.234;
        assert_eq!(tracker.format_cost(), "$1.23");
    }

    #[test]
    fn test_to_metadata_and_restore() {
        let mut tracker = CostTracker::new();
        tracker.total_input_tokens = 5000;
        tracker.total_output_tokens = 2000;
        tracker.total_cost_usd = 0.123456;
        tracker.call_count = 3;

        let metadata = tracker.to_metadata();

        let mut restored = CostTracker::new();
        let meta_json = serde_json::json!({
            "cost_tracking": metadata,
        });
        restored.restore_from_metadata(&meta_json);

        assert_eq!(restored.total_input_tokens, 5000);
        assert_eq!(restored.total_output_tokens, 2000);
        assert!((restored.total_cost_usd - 0.123456).abs() < 1e-9);
        assert_eq!(restored.call_count, 3);
    }

    #[test]
    fn test_restore_missing_cost_tracking() {
        let mut tracker = CostTracker::new();
        tracker.total_input_tokens = 100;
        // No cost_tracking key — should be a no-op
        tracker.restore_from_metadata(&serde_json::json!({}));
        assert_eq!(tracker.total_input_tokens, 100);
    }

    #[test]
    fn test_token_usage_from_json() {
        let json = serde_json::json!({
            "prompt_tokens": 1500,
            "completion_tokens": 300,
            "cache_read_input_tokens": 800,
        });
        let usage = TokenUsage::from_json(&json);
        assert_eq!(usage.prompt_tokens, 1500);
        assert_eq!(usage.completion_tokens, 300);
        assert_eq!(usage.cache_read_input_tokens, 800);
        assert_eq!(usage.cache_creation_input_tokens, 0);
    }

    #[test]
    fn test_round_f64() {
        assert_eq!(round_f64(1.23456789, 6), 1.234568);
        assert_eq!(round_f64(0.0, 2), 0.0);
    }
}
