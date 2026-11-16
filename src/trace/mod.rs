pub mod calls;
pub mod catalog;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod droid;
pub mod grok;
pub mod hermes;
pub mod jsonl;
pub mod opencode;
pub mod pi;
pub mod time;

pub type Call = (
    serde_json::Value,
    serde_json::Value,
    String,
    serde_json::Value,
);
pub mod antigravity;
