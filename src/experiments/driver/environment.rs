use super::{
    Failure, Result, Runtime, Text, Value, append, boolean, copy, exit, formatted, function,
    invoke, json, literal, method, returned, truthy,
};
use crate::python_json::Node;
use crate::value::python_space;
use indexmap::IndexMap;

fn whitespace(point: &u32) -> bool {
    char::from_u32(*point).is_some_and(python_space)
}
fn trim_start(points: &[u32]) -> &[u32] {
    &points[points
        .iter()
        .position(|point| !whitespace(point))
        .unwrap_or(points.len())..]
}
fn trim(points: &[u32]) -> &[u32] {
    let points = trim_start(points);
    &points[..points
        .iter()
        .rposition(|point| !whitespace(point))
        .map_or(0, |last| last + 1)]
}
fn parse_lines(lines: impl IntoIterator<Item = Text>) -> IndexMap<Text, Text> {
    let mut output = IndexMap::new();
    let export = Text::from("export ");
    for raw in lines {
        let mut line = trim(&raw.0);
        if line.first() == Some(&u32::from(b'#')) || !line.contains(&u32::from(b'=')) {
            continue;
        }
        if let Some(remainder) = line.strip_prefix(export.0.as_slice()) {
            line = trim_start(remainder);
        }
        let separator = line
            .iter()
            .position(|point| *point == u32::from(b'='))
            .unwrap();
        let key = trim(&line[..separator]);
        if key.is_empty() {
            continue;
        }
        let mut value = trim(&line[separator + 1..]);
        if value.len() >= 2
            && (value[0] == u32::from(b'\'') || value[0] == u32::from(b'"'))
            && value.last() == value.first()
        {
            value = &value[1..value.len() - 1];
        }
        if !value.is_empty() {
            output.insert(Text(key.to_vec()), Text(value.to_vec()));
        }
    }
    output
}

pub fn parse_env(text: &str) -> IndexMap<String, String> {
    parse_lines(
        text.split([
            '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
            '\u{2029}',
        ])
        .map(Text::from),
    )
    .into_iter()
    .map(|(key, value)| (key.lossy(), value.lossy()))
    .collect()
}

fn parse(rt: &mut dyn Runtime) -> Result<Value> {
    let lines: Vec<Vec<u32>> = serde_json::from_value(rt.call("driver_env_lines", json!([]))?)
        .map_err(|error| Failure::new("TypeError", error.to_string()))?;
    let output = parse_lines(lines.into_iter().map(Text))
        .into_iter()
        .map(|(key, value)| (key, Node::String(value)))
        .collect();
    Ok(json!({"provider_encoded":Node::Dict(output).json(false)}))
}
fn env_get(rt: &mut dyn Runtime, key: &str) -> Result<()> {
    function(rt, "os.environ.get")?;
    invoke(rt, &[key], &[])
}
fn env_path(rt: &mut dyn Runtime) -> Result<Value> {
    copy(rt, "explicit", "path")?;
    if !boolean(rt, "explicit")? {
        literal(rt, "key", json!("TOKENECONOMIZER_ENV_FILE"))?;
        env_get(rt, "key")?;
        copy(rt, "explicit", "called")?;
    }
    if boolean(rt, "explicit")? {
        return Ok(returned("explicit"));
    }
    function(rt, "_REPO_ROOT")?;
    copy(rt, "root", "function")?;
    literal(rt, "filename", json!(".env"))?;
    function(rt, "os.path.join")?;
    invoke(rt, &["root", "filename"], &[])?;
    copy(rt, "local", "called")?;
    function(rt, "os.path.dirname")?;
    invoke(rt, &["root"], &[])?;
    copy(rt, "parent_root", "called")?;
    function(rt, "os.path.join")?;
    invoke(rt, &["parent_root", "filename"], &[])?;
    copy(rt, "parent", "called")?;
    function(rt, "os.path.exists")?;
    invoke(rt, &["local"], &[])?;
    if boolean(rt, "called")? {
        return Ok(returned("local"));
    }
    function(rt, "os.path.exists")?;
    invoke(rt, &["parent"], &[])?;
    Ok(returned(if boolean(rt, "called")? {
        "parent"
    } else {
        "local"
    }))
}
fn load_env(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "env_path")?;
    invoke(rt, &["path"], &[])?;
    copy(rt, "p", "called")?;
    let read = (|| {
        rt.call("eval_builtin", json!(["open"]))?;
        literal(rt, "encoding", json!("utf-8"))?;
        invoke(rt, &["p"], &[("encoding", "encoding")])?;
        rt.call("task_enter", json!(["input"]))?;
        let parsed = (|| {
            function(rt, "_parse")?;
            copy(rt, "parser", "function")?;
            method(rt, "input_handle", "read", &[])?;
            copy(rt, "document", "called")?;
            copy(rt, "function", "parser")?;
            invoke(rt, &["document"], &[])?;
            copy(rt, "parsed", "called")
        })();
        match parsed {
            Ok(()) => {
                rt.call("task_exit", json!(["input", null]))?;
            }
            Err(error) => {
                rt.call("task_exit", json!(["input", error]))?;
                if !boolean(rt, "exit_result")? {
                    return Err(error);
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = read {
        if truthy(&rt.call("exception_matches", json!([error, ["FileNotFoundError"]]))?) {
            return Ok(json!({}));
        }
        return Err(error);
    }
    literal(rt, "applied", json!({}))?;
    rt.call("workspace_local", json!(["parsed", "parsed"]))?;
    method(rt, "parsed", "items", &[])?;
    rt.call("workspace_iter", json!(["called", "iterator"]))?;
    while truthy(&rt.call("workspace_next", json!(["iterator", "entry"]))?) {
        rt.call("workspace_pair", json!(["entry", "key", "value"]))?;
        let apply = if boolean(rt, "override")? {
            true
        } else {
            env_get(rt, "key")?;
            !boolean(rt, "called")?
        };
        if apply {
            function(rt, "os.environ")?;
            rt.call("workspace_put", json!(["function", "key", "value"]))?;
            rt.call("workspace_put", json!(["applied", "key", "value"]))?;
        }
    }
    Ok(returned("applied"))
}
fn require_keys(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "missing", json!([]))?;
    rt.call("workspace_iter", json!(["names", "iterator"]))?;
    while truthy(&rt.call("workspace_next", json!(["iterator", "key"]))?) {
        env_get(rt, "key")?;
        if !boolean(rt, "called")? {
            rt.call("eval_append", json!(["missing", "key"]))?;
        }
    }
    if boolean(rt, "missing")? {
        literal(rt, "separator", json!(", "))?;
        method(rt, "separator", "join", &["missing"])?;
        let mut message = Text::from("missing credential(s): ");
        message.0.extend(formatted(rt, "called", "")?.0);
        append(&mut message, "\nset them in ");
        function(rt, "env_path")?;
        invoke(rt, &[], &[])?;
        message.0.extend(formatted(rt, "called", "")?.0);
        append(
            &mut message,
            " (copy .env.example) or export them in your shell",
        );
        return exit(rt, message);
    }
    Ok(Value::Null)
}
pub(super) fn run(name: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match name {
        "_parse" => parse(rt),
        "env_path" => env_path(rt),
        "load_env" => load_env(rt),
        _ => require_keys(rt),
    }
}
