pub mod calls;
pub mod catalog;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod grok;
pub mod jsonl;
pub mod opencode;
pub mod pi;

pub type Call = (
    serde_json::Value,
    serde_json::Value,
    String,
    serde_json::Value,
);
pub mod antigravity;
