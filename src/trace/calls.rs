use std::{collections::BTreeMap, path::Path};

use super::{Call, claude, codex, copilot, jsonl};

pub fn strip_rtk_prefix(command: &str) -> String {
    let command = command.trim_start();
    let Some(rest) = command
        .strip_prefix("rtk")
        .filter(|s| s.starts_with(char::is_whitespace))
    else {
        return command.into();
    };
    let mut rest = rest.trim_start();
    if rest.is_empty() {
        return command.into();
    }
    if let Some(proxy) = rest
        .strip_prefix("proxy")
        .filter(|s| s.starts_with(char::is_whitespace))
    {
        let proxy = proxy.trim_start();
        if !proxy.is_empty() {
            rest = proxy;
        }
    }
    rest.into()
}

pub fn is_codex_exec(lines: &[String]) -> bool {
    lines.iter().any(|line| {
        line.replace(' ', "")
            .contains("\"type\":\"item.completed\"")
    })
}

pub fn load_trajectory(path: &Path) -> Vec<Call> {
    let lines = jsonl::lines(path).collect::<Vec<_>>();
    if is_codex_exec(&lines) {
        return codex::exec_calls(&lines);
    }
    if lines.iter().any(|line| {
        line.contains("\"tool_use\"")
            || line.contains("\"type\": \"assistant\"")
            || line.contains("\"type\":\"assistant\"")
    }) {
        return claude::calls(&lines)
            .into_iter()
            .map(|(name, args, result, ok, _)| (name, args, result, ok.into()))
            .collect();
    }
    copilot::event_calls(&lines)
}

pub fn call_cwds(path: &Path) -> Vec<Option<String>> {
    let calls = load_trajectory(path);
    if calls.is_empty() || !path.exists() {
        return vec![None; calls.len()];
    }
    let lines = jsonl::lines(path).collect::<Vec<_>>();
    if is_codex_exec(&lines) {
        return vec![None; calls.len()];
    }
    let cwds = claude::calls(&lines)
        .into_iter()
        .map(|c| c.4)
        .collect::<Vec<_>>();
    if cwds.len() == calls.len() {
        return cwds;
    }
    vec![copilot::session_cwd(&lines); calls.len()]
}

pub fn tool_usage(path: &Path) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for (name, _, _, _) in load_trajectory(path) {
        if let Some(name) = name.as_str().filter(|name| !name.is_empty()) {
            *counts.entry(name.into()).or_default() += 1;
        }
    }
    counts
}
