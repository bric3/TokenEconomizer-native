use serde_json::Value;

use crate::{pricing, value::sum_numbers};

pub const OVERHEAD_KEYS: [&str; 6] = [
    "optimizer_tokens",
    "untool_classify_tokens",
    "untool_judge_tokens",
    "codex_untool_adapt_tokens",
    "spiral_judge_tokens",
    "spiral_nudge_writer_tokens",
];

pub fn overhead(row: &Value) -> f64 {
    // shortcut: results use f64, retain integer result types during the numeric JSON-codec audit.
    sum_numbers(OVERHEAD_KEYS.iter().map(|key| &row[*key]))
}

pub fn effective_exec(row: &Value) -> Result<f64, pricing::PricingError> {
    pricing::effective_tokens(row, None)
}
