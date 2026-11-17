use std::{
    collections::HashSet,
    fs::{DirBuilder, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::Path,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

use regex::Regex;
use rusqlite::{
    Connection, OpenFlags, Params, Row, params, params_from_iter,
    types::{Type, ValueRef},
};
use serde_json::{Map, Value, json};

use crate::value::{float_or_zero, json_text, round_decimal, scalar_text, truthy};

pub const INFO_FIELDS: [&str; 9] = [
    "id",
    "source",
    "created_source",
    "cwd",
    "model",
    "parent_session_id",
    "started_at",
    "ended_at",
    "end_reason",
];
pub const MESSAGE_FIELDS: [&str; 8] = [
    "role",
    "content",
    "tool_call_id",
    "tool_calls",
    "tool_name",
    "timestamp",
    "message_uid",
    "_compressed_summary",
];
pub const PROMPT: &str = "[Hermes prompt]";

pub fn tool(name: &Value, args: &Value) -> (Value, Value) {
    static PATCH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\*\*\* (?:Update|Add|Delete) File:\s*(.+?)\s*$").unwrap()
    });
    let mut name = name.clone();
    let mut args = args
        .as_object()
        .cloned()
        .map(Value::Object)
        .unwrap_or(json!({}));
    if name == "tool_call" && args["name"].is_string() {
        name = args["name"].clone();
        args = args["arguments"]
            .as_object()
            .cloned()
            .map(Value::Object)
            .unwrap_or(json!({}));
    }
    if name == "patch"
        && let Some(patch) = args["patch"].as_str()
    {
        let paths = PATCH
            .captures_iter(patch)
            .map(|c| c[1].to_owned())
            .collect::<Vec<_>>();
        return (
            json!("apply_patch"),
            json!({"path": paths.first().cloned().unwrap_or_default(), "paths": paths}),
        );
    }
    let mapped = match name.as_str() {
        Some("terminal") => json!("bash"),
        Some("read_file") => json!("read"),
        Some("write_file") => json!("write"),
        Some("patch") => json!("edit"),
        Some("search_files") => json!("grep"),
        _ => name,
    };
    (mapped, args)
}

pub fn json_object(text: &Value) -> Option<Value> {
    let text = text.as_str()?;
    let start = if text.trim_start().starts_with("<untrusted_tool_result") {
        text.find('{')?
    } else {
        text.len() - text.trim_start().len()
    };
    if !text[start..].starts_with('{') {
        return None;
    }
    serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()?
        .ok()
        .filter(Value::is_object)
}

pub fn result(text: &Value) -> (String, Option<i64>, bool) {
    let raw = if text.is_string() || truthy(text) {
        scalar_text(text)
    } else {
        String::new()
    };
    let Some(object) = json_object(text) else {
        return (raw, None, false);
    };
    let code = object["exit_code"].as_i64();
    let failed =
        truthy(&object["error"]) || code.is_some_and(|n| n != 0) || object["success"] == false;
    let parts = ["output", "content", "error"]
        .iter()
        .filter_map(|key| object[*key].as_str().filter(|s| !s.is_empty()))
        .collect::<Vec<_>>();
    (
        if parts.is_empty() {
            raw
        } else {
            parts.join("\n")
        },
        code,
        failed,
    )
}

pub fn load(path: &Path) -> (Value, Vec<Value>, Vec<Value>) {
    let data = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .unwrap_or(Value::Null);
    let info = data["info"]
        .as_object()
        .cloned()
        .map(Value::Object)
        .unwrap_or(json!({}));
    let rows = |key: &str| {
        data[key]
            .as_array()
            .map(|rows| rows.iter().filter(|r| r.is_object()).cloned().collect())
            .unwrap_or_default()
    };
    (info, rows("messages"), rows("usage"))
}

pub fn prompts(messages: &[Value]) -> Vec<usize> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        if message["role"] != "user" || truthy(&message["_compressed_summary"]) {
            continue;
        }
        let key = if truthy(&message["message_uid"]) {
            json!(["uid", message["message_uid"]])
        } else {
            json!(["fields", message["timestamp"], message["content"]])
        }
        .to_string();
        if truthy(&message["content"])
            && !scalar_text(&message["content"]).trim().is_empty()
            && seen.insert(key)
        {
            out.push(index);
        }
    }
    out
}

pub fn session_info(path: &Path) -> Value {
    load(path).0
}
pub fn prompt_count(path: &Path) -> usize {
    prompts(&load(path).1).len()
}

pub fn tool_calls(message: &Value) -> Vec<Value> {
    message["tool_calls"]
        .as_str()
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|calls| {
            calls
                .as_array()
                .map(|calls| calls.iter().filter(|c| c.is_object()).cloned().collect())
        })
        .unwrap_or_default()
}

pub fn session_to_claude(path: &Path, out_path: &Path) -> Result<Option<usize>, String> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Ok(None);
    }
    let (info, messages, _) = load(path);
    if messages.is_empty() {
        return Ok(None);
    }
    let prompts = prompts(&messages).into_iter().collect::<HashSet<_>>();
    let mut seen_calls = HashSet::new();
    let mut seen_results = HashSet::new();
    let mut out = BufWriter::new(File::create(out_path).map_err(|e| e.to_string())?);
    let mut n = 0;
    for (index, message) in messages.iter().enumerate() {
        let event = if message["role"] == "user" && prompts.contains(&index) {
            json!({"type": "user", "message": {"role": "user", "content": scalar_text(&message["content"])}})
        } else if message["role"] == "assistant" {
            let mut blocks = Vec::new();
            if truthy(&message["content"]) {
                blocks.push(json!({"type": "text", "text": message["content"]}));
            }
            for call in tool_calls(message) {
                let id = call
                    .get("id")
                    .filter(|v| truthy(v))
                    .unwrap_or(&call["call_id"]);
                if !seen_calls.insert(id.to_string()) {
                    continue;
                }
                let function = &call["function"];
                let args = function["arguments"]
                    .as_str()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(json!({}));
                let (name, args) = tool(&function["name"], &args);
                blocks.push(json!({"type": "tool_use", "id": id, "name": name, "input": args}));
                n += 1;
            }
            if blocks.is_empty() {
                continue;
            }
            let mut event = json!({"type": "assistant", "message": {"role": "assistant", "content": blocks, "model": info["model"]}});
            if let Some(cwd) = info["cwd"].as_str().filter(|s| !s.is_empty()) {
                event["cwd"] = cwd.into();
            }
            event
        } else if message["role"] == "tool"
            && seen_results.insert(message["tool_call_id"].to_string())
        {
            let (text, _, failed) = result(&message["content"]);
            json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": message["tool_call_id"], "content": text, "is_error": failed}]}})
        } else {
            continue;
        };
        writeln!(out, "{}", json_text(&event)).map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(Some(n))
}

pub fn int_field(row: &Value, key: &str) -> f64 {
    row[key]
        .as_number()
        .filter(|n| n.is_i64() || n.is_u64())
        .and_then(|n| n.as_f64())
        .unwrap_or(0.0)
}

pub fn row_usage(row: &Value) -> Value {
    json!({"input_tokens": int_field(row, "input_tokens"), "cache_read_input_tokens": int_field(row, "cache_read_tokens"),
        "cache_creation_input_tokens": int_field(row, "cache_write_tokens"), "output_tokens": int_field(row, "output_tokens"), "model": row["model"]})
}

pub fn session_usage(path: &Path) -> Result<Value, String> {
    let mut totals = json!({"input_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0, "output_tokens": 0});
    let mut cost = 0.0;
    let mut requests = 0.0;
    let mut models = Map::new();
    for row in load(path).2 {
        let usage = row_usage(&row);
        let calls = int_field(&row, "api_call_count");
        requests += calls;
        cost += float_or_zero(&row["estimated_cost_usd"])?;
        for (key, total) in totals.as_object_mut().unwrap() {
            *total = (total.as_f64().unwrap() + usage[key].as_f64().unwrap()).into();
        }
        if let Some(model) = usage["model"].as_str().filter(|s| !s.is_empty()) {
            let n = models.get(model).and_then(Value::as_f64).unwrap_or(0.0) + calls.max(1.0);
            models.insert(model.into(), n.into());
        }
    }
    let mut model = Value::Null;
    let mut most = f64::NEG_INFINITY;
    for (name, n) in models {
        if n.as_f64().unwrap() > most {
            most = n.as_f64().unwrap();
            model = name.into();
        }
    }
    totals["requests"] = requests.into();
    totals["cost_usd_vendor"] = round_decimal(cost, 6).into();
    totals["model"] = model;
    Ok(totals)
}

fn connect(path: &Path) -> rusqlite::Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(5))?;
    Ok(connection)
}

fn read_row(row: &Row<'_>, names: &[String], fields: Option<&[&str]>) -> rusqlite::Result<Value> {
    let mut out = Map::new();
    for (index, name) in names.iter().enumerate() {
        if fields.is_some_and(|fields| !fields.contains(&name.as_str())) {
            continue;
        }
        let value = match row.get_ref(index)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => n.into(),
            ValueRef::Real(n) => json!(n),
            ValueRef::Text(text) => std::str::from_utf8(text)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(e))
                })?
                .into(),
            ValueRef::Blob(_) if fields.is_some() => Value::Null,
            ValueRef::Blob(_) => {
                return Err(rusqlite::Error::InvalidColumnType(
                    index,
                    name.clone(),
                    Type::Blob,
                ));
            }
        };
        out.insert(name.clone(), value);
    }
    Ok(out.into())
}

fn query<P: Params>(
    connection: &Connection,
    sql: &str,
    params: P,
    fields: Option<&[&str]>,
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = connection.prepare(sql)?;
    let names = statement
        .column_names()
        .map(String::from)
        .collect::<Vec<_>>();
    let mut rows = statement.query(params)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(read_row(row, &names, fields)?);
    }
    Ok(out)
}

fn session(connection: &Connection, session_id: &str) -> rusqlite::Result<Option<Value>> {
    Ok(query(
        connection,
        "SELECT * FROM sessions WHERE id = ?",
        [session_id],
        Some(&INFO_FIELDS),
    )?
    .into_iter()
    .next())
}

fn lineage(connection: &Connection, session_id: &str) -> rusqlite::Result<Vec<Value>> {
    let mut tip = session(connection, session_id)?;
    let mut seen = HashSet::new();
    while let Some(row) = tip.take() {
        if row["end_reason"] != "compression" || !seen.insert(row["id"].to_string()) {
            tip = Some(row);
            break;
        }
        let children = query(
            connection,
            "SELECT id FROM sessions WHERE parent_session_id = ? AND source IS NOT 'subagent' ORDER BY started_at DESC LIMIT 1",
            [row["id"].as_str().unwrap_or("")],
            None,
        )?;
        if let Some(child) = children.first() {
            tip = session(connection, child["id"].as_str().unwrap_or(""))?;
        } else {
            tip = Some(row);
            break;
        }
    }
    ancestors(connection, tip)
}

fn ancestors(connection: &Connection, mut tip: Option<Value>) -> rusqlite::Result<Vec<Value>> {
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    while let Some(row) = tip.take() {
        if !seen.insert(row["id"].to_string()) {
            break;
        }
        if truthy(&row["parent_session_id"]) {
            tip = session(connection, row["parent_session_id"].as_str().unwrap_or(""))?
                .filter(|parent| parent["end_reason"] == "compression");
        }
        rows.push(row);
    }
    rows.reverse();
    Ok(rows)
}

pub(crate) fn gateway_lineage(db_path: &Path, session_id: &str) -> rusqlite::Result<Vec<Value>> {
    // Gateway events name the current leaf; export readers also search for newer children.
    let connection = connect(db_path)?;
    connection.busy_timeout(Duration::from_secs(2))?;
    ancestors(&connection, session(&connection, session_id)?)
}

pub fn kept(row: &Value, fields: &[String]) -> Value {
    fields
        .iter()
        .filter_map(|key| row.get(key).map(|value| (key.clone(), value.clone())))
        .collect::<Map<_, _>>()
        .into()
}

pub fn private_message(message: &Value) -> Value {
    let mut row = MESSAGE_FIELDS
        .iter()
        .filter_map(|key| {
            message
                .get(*key)
                .map(|value| ((*key).into(), value.clone()))
        })
        .collect::<Map<String, Value>>();
    if row.get("role").is_some_and(|role| role == "tool") {
        return row.into();
    }
    let typed = row.get("role").is_some_and(|role| role == "user")
        && row
            .get("_compressed_summary")
            .is_none_or(|summary| !truthy(summary))
        && row
            .get("content")
            .is_some_and(|content| truthy(content) && !scalar_text(content).trim().is_empty());
    row.insert("content".into(), if typed { PROMPT } else { "" }.into());
    row.into()
}

fn write_export(path: &Path, document: &Value) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "export path requires a parent directory",
            )
        })?;
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(parent)?;
    let temporary = parent.join(format!(
        "{}.{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut owned = false;
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temporary)?;
        owned = true;
        file.write_all(json_text(document).as_bytes())?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if owned && result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn export(db_path: &Path, session_id: &str, path: &Path) -> bool {
    let document = (|| -> rusqlite::Result<Option<Value>> {
        let connection = connect(db_path)?;
        let chain = lineage(&connection, session_id)?;
        if chain.is_empty() {
            return Ok(None);
        }
        let ids = chain
            .iter()
            .map(|r| r["id"].as_str().unwrap_or(""))
            .collect::<Vec<_>>();
        let marks = vec!["?"; ids.len()].join(",");
        let messages = query(
            &connection,
            &format!("SELECT * FROM messages WHERE session_id IN ({marks}) ORDER BY id"),
            params_from_iter(ids.iter()),
            Some(&MESSAGE_FIELDS),
        )?;
        // Subagents contribute their spending; their messages belong to their own session.
        let usage = query(
            &connection,
            &format!(
                "SELECT * FROM session_model_usage WHERE session_id IN ({marks}) OR session_id IN (SELECT id FROM sessions WHERE parent_session_id IN ({marks}) AND source = 'subagent')"
            ),
            params_from_iter(ids.iter().chain(ids.iter())),
            None,
        )?;
        Ok(Some(
            json!({"info": chain.last().unwrap(), "messages": messages.iter().map(private_message).collect::<Vec<_>>(), "usage": usage}),
        ))
    })();
    match document {
        Ok(Some(document)) => write_export(path, &document).is_ok(),
        _ => false,
    }
}

pub fn session_row(db_path: &Path, session_id: &str) -> (Option<Value>, Vec<Value>) {
    let result = (|| -> rusqlite::Result<_> {
        let connection = connect(db_path)?;
        let row = query(
            &connection,
            "SELECT id, title, ended_at FROM sessions WHERE id = ?",
            [session_id],
            None,
        )?
        .into_iter()
        .next();
        let usage = query(
            &connection,
            "SELECT * FROM session_model_usage WHERE session_id = ?",
            [session_id],
            None,
        )?;
        Ok((row, usage))
    })();
    result.unwrap_or((None, Vec::new()))
}

pub fn find_session(db_path: &Path, source: &str, prompt: &str, since: f64) -> Option<String> {
    let connection = connect(db_path).ok()?;
    let rows = query(&connection, "SELECT s.id FROM sessions s JOIN messages m ON m.session_id = s.id WHERE (s.source = ? OR s.created_source = ?) AND s.started_at >= ? AND m.role = 'user' AND m.content = ? ORDER BY s.started_at DESC LIMIT 1", params![source, source, since, prompt], None).ok()?;
    rows.first()?["id"].as_str().map(String::from)
}
