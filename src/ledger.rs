use serde_json::Value;

use crate::{pricing, value::number};

pub const OVERHEAD_KEYS: [&str; 6] = [
    "optimizer_tokens",
    "untool_classify_tokens",
    "untool_judge_tokens",
    "codex_untool_adapt_tokens",
    "spiral_judge_tokens",
    "spiral_nudge_writer_tokens",
];

pub fn overhead(row: &Value) -> f64 {
    OVERHEAD_KEYS
        .iter()
        .map(|key| number(&row[*key]).unwrap_or(0.0))
        .sum()
}

pub fn effective_exec(row: &Value) -> Result<f64, pricing::PricingError> {
    pricing::effective_tokens(row, None)
}
