use super::{Failure, Result};
use crate::{
    trace::{calls::trajectory_from_lines, jsonl::TextReader},
    value::{round_decimal, scalar_text, truthy},
};
use regex::Regex;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

fn strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
}
fn path(value: &Value, cwd: &Path) -> Result<Option<(PathBuf, PathBuf)>> {
    if !truthy(value) {
        return Ok(None);
    }
    let text = value
        .as_str()
        .ok_or_else(|| Failure::new("TypeError", "invalid transcript path"))?;
    let display = PathBuf::from(text);
    let physical = cwd.join(&display);
    Ok(physical.exists().then_some((physical, display)))
}
pub fn unwrap_shell(command: &str) -> String {
    static WRAPPER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:/bin/|/usr/bin/)?(?:ba|z|da)?sh[\s\x1c-\x1f]+-l?c[\s\x1c-\x1f]+").unwrap()
    });
    let mut command = strip(command);
    if let Some(found) = WRAPPER.find(command) {
        command = strip(&command[found.end()..]);
        if command.len() >= 2
            && (command.starts_with('\'') && command.ends_with('\'')
                || command.starts_with('"') && command.ends_with('"'))
        {
            command = &command[1..command.len() - 1];
        }
    }
    strip(command).into()
}
pub fn rtk_compliance(value: &Value, cwd: &Path) -> Result<Value> {
    let mut fields =
        json!({"rtk_shell_commands":null,"rtk_prefixed_commands":null,"rtk_compliance":null});
    let Some((path, _)) = path(value, cwd)? else {
        return Ok(fields);
    };
    let read = || -> std::io::Result<Vec<String>> {
        let mut reader = TextReader::open(&path)?;
        let mut lines = Vec::new();
        loop {
            let line = reader.readline(usize::MAX)?;
            if line.is_empty() {
                break;
            }
            let line = strip(&line);
            if !line.is_empty() {
                lines.push(line.into());
            }
        }
        Ok(lines)
    };
    let Ok(lines) = read() else {
        return Ok(fields);
    };
    let (mut total, mut prefixed) = (0, 0);
    for (_, args, _, _) in trajectory_from_lines(&lines) {
        let Some(args) = args.as_object() else {
            continue;
        };
        let value = args
            .get("command")
            .filter(|v| truthy(v))
            .or_else(|| args.get("cmd").filter(|v| truthy(v)));
        let command = match value {
            Some(Value::Array(items)) => {
                items.iter().map(scalar_text).collect::<Vec<_>>().join(" ")
            }
            Some(value) => scalar_text(value),
            None => String::new(),
        };
        let command = unwrap_shell(&command);
        if command.is_empty() {
            continue;
        }
        total += 1;
        let first = command
            .split(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
            .next()
            .unwrap();
        if first == "rtk" || first.ends_with("/rtk") {
            prefixed += 1;
        }
    }
    fields["rtk_shell_commands"] = json!(total);
    fields["rtk_prefixed_commands"] = json!(prefixed);
    fields["rtk_compliance"] = if total == 0 {
        Value::Null
    } else {
        json!(round_decimal(f64::from(prefixed) / f64::from(total), 3))
    };
    Ok(fields)
}
fn first<'a>(items: impl IntoIterator<Item = &'a Value>) -> Value {
    items
        .into_iter()
        .find(|v| truthy(v))
        .cloned()
        .unwrap_or(json!(""))
}
pub fn mcp_tool_calls(value: &Value, server: &Value, cwd: &Path) -> Result<Value> {
    let mut fields = json!({"mcp_tool_calls":null,"mcp_tools_used":null,"mcp_unrecognised_items":null,"mcp_item_sample":null});
    let server_key = format!("{}_tool_calls", scalar_text(server));
    if truthy(server) {
        fields[&server_key] = Value::Null;
    }
    let Some((path, display)) = path(value, cwd)? else {
        return Ok(fields);
    };
    let mut reader = TextReader::open(&path).map_err(|e| Failure::for_path(e, &display))?;
    let (mut calls, mut unrecognised, mut for_server) = (0, 0, 0);
    let mut used = serde_json::Map::<String, Value>::new();
    let mut sample = Value::Null;
    loop {
        let line = reader
            .readline(usize::MAX)
            .map_err(|e| Failure::for_path(e, &display))?;
        if line.is_empty() {
            break;
        }
        let line = strip(&line);
        if !line.contains("mcp") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let item = &event["item"];
        if !item.is_object()
            || !scalar_text(&first([&item["type"]])).contains("mcp")
            || scalar_text(&first([&event["type"]])).ends_with("started")
        {
            continue;
        }
        let inv = item
            .get("invocation")
            .filter(|v| v.is_object())
            .unwrap_or(&Value::Null);
        let srv = first([&item["server"], &item["server_name"], &inv["server"]]);
        let tool = first([
            &item["tool"],
            &item["tool_name"],
            &item["name"],
            &inv["tool"],
        ]);
        if !truthy(&tool) {
            unrecognised += 1;
            if sample.is_null() {
                sample = json!(line.chars().take(400).collect::<String>());
            }
            continue;
        }
        calls += 1;
        let key = if truthy(&srv) {
            format!("{}/{}", scalar_text(&srv), scalar_text(&tool))
        } else {
            scalar_text(&tool)
        };
        let count = used.get(&key).and_then(Value::as_u64).unwrap_or(0) + 1;
        used.insert(key.clone(), json!(count));
        if truthy(server) {
            let filter = server
                .as_str()
                .ok_or_else(|| Failure::new("TypeError", "unsupported operand type(s) for +"))?;
            if srv == *server || key.starts_with(&format!("{filter}/")) || key.contains(filter) {
                for_server += 1;
            }
        }
    }
    fields["mcp_tool_calls"] = json!(calls);
    fields["mcp_tools_used"] = json!(used);
    fields["mcp_unrecognised_items"] = json!(unrecognised);
    fields["mcp_item_sample"] = sample;
    if truthy(server) {
        fields[&server_key] = json!(for_server);
    }
    Ok(fields)
}
