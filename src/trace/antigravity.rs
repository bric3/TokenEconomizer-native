use std::{
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::LazyLock,
};

use regex::Regex;
use serde_json::{Map, Value, json};

use super::jsonl::records;
use crate::value::{json_text_utf8, scalar_text, truthy};

pub fn tool(name: &Value, args: &Value) -> (String, Value) {
    let files = ["TargetFile", "AbsolutePath", "Path"];
    let (mapped, keys): (&str, Vec<(&str, &[&str])>) = match name.as_str().unwrap_or("") {
        "run_command" => ("Bash", vec![("command", &["CommandLine"])]),
        "view_file" | "read_file" => ("Read", vec![("file_path", &files)]),
        "write_file" | "write_to_file" => ("Write", vec![("file_path", &files)]),
        "edit_file" | "replace_file_content" | "multi_replace_file_content" | "sed_file" => {
            ("Edit", vec![("file_path", &files)])
        }
        "grep_search" => (
            "Grep",
            vec![("pattern", &["Query"]), ("path", &["SearchPath"])],
        ),
        "find_by_name" => (
            "Glob",
            vec![("pattern", &["Pattern"]), ("path", &["SearchDirectory"])],
        ),
        "list_dir" => ("Glob", vec![("path", &["DirectoryPath"])]),
        _ => {
            let mut out = Map::new();
            if let Some(args) = args.as_object() {
                for (key, value) in args {
                    if ["toolAction", "toolSummary", "WaitMsBeforeAsync"].contains(&key.as_str()) {
                        continue;
                    }
                    let value = value
                        .as_str()
                        .map(|s| json!(s.chars().take(2000).collect::<String>()))
                        .unwrap_or_else(|| value.clone());
                    out.insert(key.clone(), value);
                }
            }
            return (
                if truthy(name) {
                    scalar_text(name)
                } else {
                    "?".into()
                },
                out.into(),
            );
        }
    };
    let out = keys
        .into_iter()
        .map(|(key, names)| {
            (
                key.to_owned(),
                names
                    .iter()
                    .find_map(|name| args[*name].as_str())
                    .unwrap_or("")
                    .into(),
            )
        })
        .collect::<Map<_, _>>();
    (mapped.into(), out.into())
}

pub fn result(row: &Value) -> (String, Option<i64>, bool) {
    static PREFIX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:(?:Created At|Completed At):|A pre-tool hook changed the arguments)")
            .unwrap()
    });
    static EXIT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"Command exited with code (-?\d+)").unwrap());
    let content = row["content"].as_str().unwrap_or("").replace("\r\n", "\n");
    let mut lines = content.split('\n').peekable();
    while lines.peek().is_some_and(|line| PREFIX.is_match(line)) {
        lines.next();
    }
    let mut text = lines
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_owned();
    let code = row["exit_code"].as_i64().or_else(|| {
        let head = text.chars().take(400).collect::<String>();
        EXIT.captures(&head).and_then(|c| c[1].parse().ok())
    });
    let failed = ["ERROR", "INVALID"]
        .iter()
        .any(|status| row["status"] == *status)
        || code.is_some_and(|code| code != 0);
    if let Some(error) = row["error"].as_str().filter(|s| !s.is_empty()) {
        text = format!("{error}\n{text}").trim_matches('\n').into();
    }
    (text, code, failed)
}

pub fn prompt_text(row: &Value) -> String {
    static USER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?s)<USER_REQUEST>\s*(.*?)\s*</USER_REQUEST>").unwrap());
    let content = row["content"].as_str().unwrap_or("");
    USER.captures(content)
        .map(|c| c[1].trim().into())
        .unwrap_or_else(|| {
            content
                .split("<ADDITIONAL_METADATA>")
                .next()
                .unwrap_or("")
                .trim()
                .into()
        })
}

pub fn read_rows(path: &Path, offset: u64) -> Vec<(u64, Value)> {
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let Ok(metadata) = file.metadata() else {
        return Vec::new();
    };
    let offset = if offset > metadata.len() { 0 } else { offset };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return Vec::new();
    }
    let mut data = Vec::new();
    if file.read_to_end(&mut data).is_err() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0;
    let mut pos = 0;
    while start < data.len() {
        let relative = data[start..]
            .iter()
            .position(|b| matches!(b, b'\r' | b'\n'));
        let mut end = relative.map_or(data.len(), |n| start + n + 1);
        if end < data.len() && data[end - 1] == b'\r' && data[end] == b'\n' {
            end += 1;
        }
        let raw = &data[start..end];
        let row = match serde_json::from_slice::<Value>(raw) {
            Ok(row) => row,
            Err(_) if raw.last() != Some(&b'\n') => break,
            Err(_) => Value::Null,
        };
        pos += raw.len() as u64;
        if row.is_object() {
            out.push((offset + pos, row));
        }
        start = end;
    }
    out
}

pub fn is_result(row: &Value) -> bool {
    row["source"] == "MODEL"
        && !["PLANNER_RESPONSE", "FINISH"]
            .iter()
            .any(|kind| row["type"] == *kind)
}

fn close(items: &mut Vec<Value>, calls: &mut Option<Vec<Value>>, results: &mut Vec<Value>) {
    for (spec, row) in calls
        .take()
        .unwrap_or_default()
        .into_iter()
        .zip(std::mem::take(results))
    {
        if row["status"] == "RUNNING" {
            continue;
        }
        let (text, code, failed) = result(&row);
        let (name, args) = tool(&spec["name"], &spec["args"]);
        items.push(
            json!({"kind": "tool", "name": name, "args": args, "cwd": spec["args"]["Cwd"].as_str(),
            "content": text, "exit_code": code, "is_error": failed}),
        );
    }
}

pub fn read(path: &Path, offset: u64, after: i64, final_read: bool) -> (Vec<Value>, u64, i64) {
    let rows = read_rows(path, offset)
        .into_iter()
        .filter(|(_, row)| row["step_index"].as_i64().is_none_or(|step| step > after))
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    let mut done = 0;
    let mut calls = None;
    let mut results = Vec::new();
    for (index, (_, row)) in rows.iter().enumerate() {
        if row["type"] == "PLANNER_RESPONSE" || row["type"] == "USER_INPUT" {
            close(&mut items, &mut calls, &mut results);
            done = index + 1;
            if row["type"] == "USER_INPUT" {
                items.push(json!({"kind": "prompt", "text": prompt_text(row)}));
            } else if let Some(specs) = row["tool_calls"].as_array() {
                let specs = specs
                    .iter()
                    .filter(|c| {
                        c.is_object()
                            && (!truthy(&c["args"]) || c["args"].is_object())
                            && c["name"] != "finish"
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !specs.is_empty() {
                    done = index;
                    calls = Some(specs);
                }
            }
        } else if calls.is_some() {
            if is_result(row) {
                results.push(row.clone());
            }
        } else {
            done = index + 1;
        }
    }
    if calls
        .as_ref()
        .is_some_and(|calls| final_read || results.len() >= calls.len())
    {
        close(&mut items, &mut calls, &mut results);
        done = rows.len();
    }
    if done == 0 {
        return (items, offset, after);
    }
    (
        items,
        rows[done - 1].0,
        rows[done - 1].1["step_index"].as_i64().unwrap_or(after),
    )
}

pub fn session_to_claude(path: &Path, out_path: &Path) -> Option<usize> {
    let items = read(path, 0, -1, true).0;
    if items.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for (n, item) in items.iter().enumerate() {
        if item["kind"] == "prompt" {
            if truthy(&item["text"]) {
                out.push(json!({"type": "user", "message": {"content": [{"type": "text", "text": item["text"]}]}}));
            }
            continue;
        }
        let id = format!("agy-{n}");
        out.push(json!({"type": "assistant", "cwd": item["cwd"], "message": {"content": [{"type": "tool_use", "id": id, "name": item["name"], "input": item["args"]}]}}));
        out.push(json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": id,
            "content": item["content"], "is_error": item["is_error"]}]}}));
    }
    let mut file = BufWriter::new(File::create(out_path).ok()?);
    for event in &out {
        writeln!(file, "{}", json_text_utf8(event)).ok()?;
    }
    file.flush().ok()?;
    Some(out.len())
}

pub fn session_usage(path: &Path) -> Value {
    let mut totals = json!({"input_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0, "output_tokens": 0});
    let mut requests = 0;
    for row in records(path).filter(|r| r["type"] == "PLANNER_RESPONSE") {
        let mut seen = false;
        for (source, target) in [
            ("input_tokens", "input_tokens"),
            ("cache_read_tokens", "cache_read_input_tokens"),
            ("output_tokens", "output_tokens"),
        ] {
            if let Some(n) = row[source]
                .as_number()
                .filter(|n| n.is_i64() || n.is_u64())
                .and_then(|n| n.as_f64())
            {
                totals[target] = (totals[target].as_f64().unwrap() + n).into();
                seen = true;
            }
        }
        requests += i64::from(seen);
    }
    if requests == 0 {
        return json!({});
    }
    totals["requests"] = requests.into();
    totals["model"] = "antigravity".into();
    totals
}

pub fn prompt_count(path: &Path) -> usize {
    records(path)
        .filter(|row| row["type"] == "USER_INPUT" && row["source"] == "USER_EXPLICIT")
        .count()
}
