use std::{path::Path, sync::LazyLock};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{
    rules::Signals,
    trace::{Call, calls::load_trajectory},
    value::{round_decimal, scalar_text, truthy},
};

pub static TEST_RUN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"(?i)(runtests(\.py)?|\bpytest\b|py\.test|manage\.py\s+test|python\s+-m\s+unittest|\btox\b|\bnox\b|\btrial\b|python\d?\s+-c\b)").unwrap()
});
pub static ERR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
    r"(?im)(Traceback \(most recent call last\)|\b\w*Error\b|\bFAILED\b|\bassert\b|Exception|errors during collection|",
    r"^# fail [1-9]|^not ok \d|\b[1-9]\d* (?:failing|failures?)\b|Failures: [1-9]|^(?-i:--- FAIL: |FAIL\t|FAIL$))")).unwrap()
});

pub fn canon_tool(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "bash" | "shell" => "bash",
        "read" | "view" => "view",
        "grep" => "grep",
        "glob" => "glob",
        "edit" | "multiedit" | "str_replace" | "apply_patch" => "edit",
        "write" | "create" => "create",
        _ => {
            return if name.is_empty() {
                "?".into()
            } else {
                name.into()
            };
        }
    }
    .into()
}

pub fn basename(path: &str) -> String {
    path.replace('\\', "/")
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .into()
}

pub fn failure_sig(text: &str) -> Option<String> {
    static NORMALISE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"0x[0-9a-f]+|\d+|/\S+").unwrap());
    let line = text
        .split([
            '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
            '\u{2029}',
        ])
        .find(|line| ERR_RE.is_match(line))?;
    let normalised = NORMALISE.replace_all(line, "");
    Some(format!("{:x}", md5::compute(normalised.trim().as_bytes()))[..10].into())
}

pub type CompletedCall = (String, Value, String);

fn id_key(id: &Value) -> String {
    id.to_string()
}

pub fn completed_calls(line: &str, pending: &mut Map<String, Value>) -> Vec<CompletedCall> {
    let Ok(event) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    if !event.is_object() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let event_type = event["type"].as_str().unwrap_or("");
    if event_type == "item.completed" {
        let item = &event["item"];
        if item["type"] == "command_execution" {
            out.push((
                "bash".into(),
                json!({"command": item["command"].as_str().unwrap_or("")}),
                item["aggregated_output"].as_str().unwrap_or("").into(),
            ));
        } else if item["type"] == "file_change" {
            let changes = item
                .get("changes")
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or(json!([]));
            let paths = changes
                .as_array()
                .map(|changes| {
                    changes
                        .iter()
                        .filter(|c| c.is_object() && truthy(&c["path"]))
                        .map(|c| c["path"].clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            out.push(("edit".into(), json!({"path": paths.first().cloned().unwrap_or(json!("")), "paths": paths, "changes": changes}), String::new()));
        }
        return out;
    }
    if event_type == "tool.execution_start" {
        let data = &event["data"];
        pending.insert(id_key(&data["toolCallId"]), json!({"name": data["toolName"], "args": data.get("arguments").filter(|v| truthy(v)).cloned().unwrap_or(json!({}))}));
    } else if event_type == "tool.execution_complete" {
        let data = &event["data"];
        let info = pending
            .remove(&id_key(&data["toolCallId"]))
            .unwrap_or(json!({"name": "?", "args": {}}));
        out.push((
            canon_tool(info["name"].as_str().unwrap_or("")),
            info.get("args")
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or(json!({})),
            data["result"]["content"].as_str().unwrap_or("").into(),
        ));
    } else if event_type == "assistant" {
        if let Some(blocks) = event["message"]["content"].as_array() {
            for block in blocks.iter().filter(|b| b["type"] == "tool_use") {
                pending.insert(id_key(&block["id"]), json!({"name": block["name"], "args": block.get("input").filter(|v| truthy(v)).cloned().unwrap_or(json!({}))}));
            }
        }
    } else if event_type == "user"
        && let Some(blocks) = event["message"]["content"].as_array()
    {
        for block in blocks.iter().filter(|b| b["type"] == "tool_result") {
            let info = pending
                .remove(&id_key(&block["tool_use_id"]))
                .unwrap_or(json!({"name": "?", "args": {}}));
            let content = &block["content"];
            let text = content
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .filter(|p| p.is_object())
                        .map(|p| p["text"].as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| content.as_str().unwrap_or("").into());
            out.push((
                canon_tool(info["name"].as_str().unwrap_or("")),
                info.get("args")
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or(json!({})),
                text,
            ));
        }
    }
    out
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SpiralState {
    pub turns: u64,
    pub edit_counts: Map<String, Value>,
    pub edit_order: Vec<String>,
    pub error_outputs: u64,
    pub test_runs: u64,
    pub fail_sigs: Map<String, Value>,
    pub repeated_fail_sigs: u64,
    pub recent: Vec<String>,
}

impl SpiralState {
    pub fn edit_churn(&self) -> u64 {
        self.edit_counts
            .values()
            .filter_map(Value::as_u64)
            .max()
            .unwrap_or(0)
    }
    pub fn distinct_edit_files(&self) -> usize {
        self.edit_counts.len()
    }
    pub fn stagnation(&self) -> f64 {
        let total = self
            .fail_sigs
            .values()
            .filter_map(Value::as_f64)
            .sum::<f64>();
        if total == 0.0 {
            0.0
        } else {
            self.repeated_fail_sigs as f64 / total
        }
    }

    pub fn update(
        &mut self,
        tool_name: &str,
        args: &Value,
        result_text: &str,
        failed: Option<bool>,
    ) {
        static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
        self.turns = self.turns.saturating_add(1);
        let name = canon_tool(tool_name);
        let mut action = name.clone();
        if ["edit", "str_replace", "create"].contains(&name.as_str()) {
            let paths = args["paths"]
                .as_array()
                .filter(|paths| !paths.is_empty())
                .cloned()
                .unwrap_or_else(|| {
                    vec![
                        args.get("path")
                            .filter(|v| truthy(v))
                            .or_else(|| args.get("filePath").filter(|v| truthy(v)))
                            .unwrap_or(&args["file_path"])
                            .clone(),
                    ]
                });
            let mut names = Vec::new();
            for path in paths.iter().filter_map(Value::as_str) {
                let name = basename(path);
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
            for name in &names {
                self.edit_order.push(name.clone());
                let n = self
                    .edit_counts
                    .get(name)
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .saturating_add(1);
                self.edit_counts.insert(name.clone(), n.into());
            }
            if !names.is_empty() {
                action = format!("edit {}", names.join(", "));
            }
        }
        let command = if name == "bash" {
            args["command"].as_str().unwrap_or("")
        } else {
            ""
        };
        if !command.is_empty() {
            let short = SPACE
                .replace_all(command.trim(), " ")
                .chars()
                .take(90)
                .collect::<String>();
            action = format!("$ {short}");
            if TEST_RUN_RE.is_match(command) {
                self.test_runs = self.test_runs.saturating_add(1);
            }
        } else if name == "grep" || name == "glob" {
            let pattern = if truthy(&args["pattern"]) {
                scalar_text(&args["pattern"])
            } else {
                String::new()
            };
            action = format!("{name} {}", pattern.chars().take(40).collect::<String>());
        } else if name == "view" {
            let path = args
                .get("path")
                .filter(|v| truthy(v))
                .unwrap_or(&args["file_path"])
                .as_str()
                .unwrap_or("");
            action = format!("view {}", basename(path));
        }
        let mut fail = None;
        let can_fail = if ["view", "grep", "glob"].contains(&name.as_str()) {
            failed == Some(true)
        } else {
            failed != Some(false)
        };
        if !result_text.is_empty() && can_fail && ERR_RE.is_match(result_text) {
            self.error_outputs = self.error_outputs.saturating_add(1);
            if let Some(signature) = failure_sig(result_text) {
                if self.fail_sigs.contains_key(&signature) {
                    self.repeated_fail_sigs = self.repeated_fail_sigs.saturating_add(1);
                }
                let n = self
                    .fail_sigs
                    .get(&signature)
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .saturating_add(1);
                self.fail_sigs.insert(signature.clone(), n.into());
                fail = Some(signature);
            }
        }
        let tag = fail.map(|sig| format!("  [err#{sig}]")).unwrap_or_default();
        self.recent
            .push(format!("{:>3}: {action}{tag}", self.turns));
        if self.recent.len() > 40 {
            self.recent.drain(..self.recent.len() - 40);
        }
    }

    pub fn recent_digest(&self, n: i64, envelope: Option<f64>) -> String {
        let mut head = format!(
            "turns={} edit_churn={} distinct_edit_files={} error_outputs={} test_runs={} repeated_identical_failures={} stagnation={:.2}",
            self.turns,
            self.edit_churn(),
            self.distinct_edit_files(),
            self.error_outputs,
            self.test_runs,
            self.repeated_fail_sigs,
            self.stagnation()
        );
        if let Some(envelope) = envelope.filter(|n| *n != 0.0) {
            head.push_str(&format!("\nrepo_context: tasks here usually RESOLVE within ~{envelope:.0} turns; this trajectory is at {} turns = {:.1}x that typical length. Tests are still NOT green. A trajectory far past the typical resolved length with recurring failures is likely a dead-end loop, even if it touches many files.", self.turns, self.turns as f64 / envelope));
        }
        let start = if n > 0 {
            self.recent.len().saturating_sub(n as usize)
        } else if n < 0 {
            n.unsigned_abs().min(self.recent.len() as u64) as usize
        } else {
            0
        };
        format!(
            "{head}\nRecent actions (oldest->newest):\n{}",
            self.recent[start..].join("\n")
        )
    }

    pub fn signals(&self) -> Signals {
        Signals {
            turns: self.turns,
            edit_churn: self.edit_churn(),
            distinct_edit_files: self.distinct_edit_files() as u64,
            error_outputs: self.error_outputs,
            stagnation: self.stagnation(),
            repeated_fail_sigs: self.repeated_fail_sigs,
            test_runs: self.test_runs,
            edit_counts: self.edit_counts.clone(),
        }
    }

    pub fn snapshot(&self) -> Value {
        json!({"turns": self.turns, "edit_churn": self.edit_churn(), "distinct_edit_files": self.distinct_edit_files(),
            "error_outputs": self.error_outputs, "test_runs": self.test_runs, "stagnation": round_decimal(self.stagnation(), 3),
            "repeated_fail_sigs": self.repeated_fail_sigs})
    }
}

pub fn state_from_calls(calls: &[Call]) -> SpiralState {
    let mut state = SpiralState::default();
    for (name, args, result, ok) in calls {
        state.update(
            name.as_str().unwrap_or(""),
            args,
            result,
            if ok.is_null() {
                None
            } else {
                Some(!truthy(ok))
            },
        );
    }
    state
}

pub fn state_from_trajectory(path: &Path) -> SpiralState {
    state_from_calls(&load_trajectory(path))
}
