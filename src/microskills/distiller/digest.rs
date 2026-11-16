use std::{path::Path, sync::LazyLock};

use fancy_regex::Regex as FancyRegex;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Value, json};

use crate::microskills::observe::repository_top;
use crate::{
    lifecycle::edited_files,
    trace::{calls, jsonl},
    value::{json_text, truthy},
};

pub type Call = (String, Value, String);
pub type Command = (String, String);

pub static PYTEST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-m\s+pytest|\bpytest\b|\bpy\.test\b").unwrap());
pub static REPRO_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"python[0-9.]*\s+-c|/tmp/\S*\.py|reproduce|repro").unwrap());
static BADARG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"unrecognized arguments|no such option|usage:|error: argument").unwrap()
});
pub static CHECK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
    r"\b(?:pytest|py\.test|tox|nox|unittest|runtests\.py|jest|vitest|mocha|rspec|phpunit|ctest|",
    r"tsc|eslint|prettier|ruff|flake8|pylint|mypy|pyright|golangci-lint|cmake|ninja|bazel)\b|",
    r"(?:^|[\s;&|(])make\b|\b(?:npm|yarn|pnpm|bun)\s+(?:run\s+)?(?:test|build|lint|check|typecheck)\b|",
    r"\b(?:cargo|go|mvn|gradle|gradlew|dotnet|mix|swift)\s+(?:test|build|check|vet|clippy|compile|package|verify)\b")).unwrap()
});
static TEST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
    r"\b(?:pytest|py\.test|tox|nox|unittest|runtests\.py|jest|vitest|mocha|rspec|phpunit|ctest)\b|",
    r"\b(?:make|npm|yarn|pnpm|bun|cargo|go|mvn|gradle|gradlew|dotnet|mix|swift|rake)\s+(?:run\s+)?test\b")).unwrap()
});
static MISSING_CMD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)command not found|: not found\b|is not recognized as").unwrap()
});
static MISSING_DEP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
    r"(?i)ModuleNotFoundError|No module named|ImportError|Cannot find module|ERR_MODULE_NOT_FOUND|",
    r"cannot find package|could not find (?:package|crate|module|gem)|unresolved import|",
    r"cannot load such file|undefined reference to|fatal error: [^\n]*: No such file")).unwrap()
});
static BAD_USAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^usage:|unrecognized arguments?|no such option|unknown (?:option|flag|command)|invalid option|unexpected argument|error: argument").unwrap()
});

pub fn load_calls(path: &Path) -> Vec<Call> {
    let lines: Vec<_> = jsonl::lines(path).collect();
    if lines.iter().any(|line| {
        line.contains("\"item.completed\"")
            || line.contains("\"tool.execution_start\"")
            || line.contains("\"tool.execution_complete\"")
    }) {
        return calls::load_trajectory(path)
            .into_iter()
            .map(|(name, args, out, _)| (name.as_str().unwrap_or("").to_lowercase(), args, out))
            .collect();
    }
    let mut uses: IndexMap<String, (String, Value)> = IndexMap::new();
    let mut results = std::collections::HashMap::new();
    for line in lines {
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let event_type = event.get("type").map_or(Some("assistant"), Value::as_str);
        if !event.is_object()
            || !event["message"].is_object()
            || !event_type.is_some_and(|kind| ["assistant", "user"].contains(&kind))
        {
            continue;
        }
        for block in event["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b.is_object())
        {
            let kind = block["type"].as_str().unwrap_or("");
            let id = if kind == "tool_use" {
                &block["id"]
            } else {
                &block["tool_use_id"]
            };
            let Some(id) = id.as_str() else {
                continue;
            };
            if kind == "tool_use" && !uses.contains_key(id) {
                let args = if block["input"].is_object() {
                    block["input"].clone()
                } else {
                    json!({})
                };
                uses.insert(
                    id.into(),
                    (block["name"].as_str().unwrap_or("").to_owned(), args),
                );
            } else if kind == "tool_result" {
                let content = &block["content"];
                let text = if let Some(parts) = content.as_array() {
                    parts
                        .iter()
                        .filter(|p| p.is_object())
                        .filter_map(|p| p["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    content
                        .as_str()
                        .map(String::from)
                        .unwrap_or_else(|| json_text(content))
                };
                results.insert(id.to_owned(), text);
            }
        }
    }
    uses.into_iter()
        .map(|(id, (name, args))| {
            (
                name.to_lowercase(),
                args,
                results.remove(&id).unwrap_or_default(),
            )
        })
        .collect()
}

pub fn bash_cmds(calls: &[Call]) -> Vec<Command> {
    calls
        .iter()
        .filter(|(tool, _, _)| ["bash", "shell", "stop_bash"].contains(&tool.as_str()))
        .map(|(_, args, out)| {
            let command = args["command"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| args["cmd"].as_str())
                .unwrap_or("");
            (command.into(), out.clone())
        })
        .collect()
}

pub fn templates<'a>(commands: impl IntoIterator<Item = &'a str>, cap: i64) -> Vec<String> {
    static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
    static PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/testbed/\S+").unwrap());
    let mut out = Vec::new();
    for command in commands {
        let command = command.trim();
        if command.is_empty() {
            continue;
        }
        let head: String = SPACE.replace_all(command, " ").chars().take(80).collect();
        let head = PATH.replace_all(&head, "<path>").into_owned();
        if !out.contains(&head) {
            out.push(head);
        }
        if out.len() as i64 >= cap {
            break;
        }
    }
    out
}

pub fn relative(text: &str, roots: &[String], home: &str) -> String {
    static CD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^cd \.\s*(?:&&|;)\s*").unwrap());
    let mut text = text.to_owned();
    for root in roots.iter().filter(|r| !r.is_empty()) {
        text = text.replace(&(root.clone() + "/"), "").replace(root, ".");
    }
    text = CD.replace(&text, "").into_owned();
    if !home.is_empty() && home != "/" {
        text = text.replace(home, "~");
    }
    text
}

pub fn command_failed(out: &str) -> bool {
    static EXIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\bexit(?:ed with)?(?: code| status)?[:=\s]+(\d+)").unwrap()
    });
    static FAILED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)Traceback \(most recent call last\)|^\w*Error\b").unwrap()
    });
    if let Some(m) = EXIT.captures(out) {
        return m[1].chars().any(|c| c != '0');
    }
    MISSING_CMD.is_match(out) || MISSING_DEP.is_match(out) || FAILED.is_match(out)
}

pub fn error_signature(out: &str, roots: &[String], home: &str) -> Result<String, String> {
    static ERROR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(?:error|failed|failure|fatal|exception|traceback|cannot|not found|denied|refused|panic)\b").unwrap()
    });
    static HEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"0x[0-9a-fA-F]+").unwrap());
    static PATH: LazyLock<FancyRegex> =
        LazyLock::new(|| FancyRegex::new(r#"(?<![\w.~])/[^\s'"`:,)]+"#).unwrap());
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d+\b").unwrap());
    for line in out.split([
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
        '\u{2029}',
    ]) {
        if ERROR.is_match(line) || MISSING_CMD.is_match(line) {
            let text = relative(line.trim(), roots, home);
            let text = HEX.replace_all(&text, "<hex>");
            let mut masked = String::new();
            let mut start = 0;
            for m in PATH.find_iter(text.as_ref()) {
                let m = m.map_err(|e| e.to_string())?;
                masked.push_str(&text[start..m.start()]);
                masked.push_str("<path>");
                start = m.end();
            }
            masked.push_str(&text[start..]);
            let text = NUMBER.replace_all(&masked, "N");
            return Ok(text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(120)
                .collect());
        }
    }
    Ok(String::new())
}

pub fn build_digest(
    _repo: &str,
    path: &Path,
    iid: &str,
    resolved: Value,
    generic: bool,
    home: &str,
) -> Result<Value, String> {
    if generic {
        return build_generic_digest(path, iid, resolved, home);
    }
    static OTHER_TEST: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"runtests\.py|unittest|nosetests|tox").unwrap());
    static IMPORT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"ImportError|ModuleNotFoundError|No module named").unwrap());
    let calls = load_calls(path);
    let commands = bash_cmds(&calls);
    let cmd_blob = commands
        .iter()
        .map(|(c, _)| c.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let out_blob = commands
        .iter()
        .map(|(_, o)| o.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let tests: Vec<_> = commands
        .iter()
        .filter(|(c, _)| PYTEST_RE.is_match(c) || OTHER_TEST.is_match(c))
        .map(|(c, _)| c.as_str())
        .collect();
    let test_invocations = templates(tests, 12).into_iter().take(6).collect::<Vec<_>>();
    Ok(
        json!({"iid":iid,"resolved":if resolved.is_null() { Value::Null } else { json!(truthy(&resolved)) },
        "edited_files":edited_files(calls.iter().map(|(n,a,_)| (n.as_str(),a)),true),"n_cmds":commands.len(),
        "cmd_templates":templates(commands.iter().map(|(c,_)| c.as_str()),12),"test_invocations":test_invocations,
        "repro_written":commands.iter().any(|(c,_)| REPRO_RE.is_match(c) && !PYTEST_RE.is_match(c)),"importerror":IMPORT.is_match(&out_blob),"badarg":BADARG.is_match(&out_blob),"_cmd_blob":cmd_blob,"_out_blob":out_blob}),
    )
}

pub fn build_generic_digest(
    path: &Path,
    iid: &str,
    resolved: Value,
    home: &str,
) -> Result<Value, String> {
    let calls = load_calls(path);
    let commands = bash_cmds(&calls);
    let cwds = calls::call_cwds(path);
    let start = cwds.into_iter().flatten().find(|s| !s.is_empty());
    let mut roots: Vec<_> = start
        .iter()
        .cloned()
        .chain(start.as_deref().and_then(repository_top))
        .collect();
    roots.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    roots.dedup();
    let out_blob = commands
        .iter()
        .map(|(_, o)| o.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let mut failed = Vec::new();
    for (command, out) in &commands {
        if !command_failed(out) {
            continue;
        }
        let command = templates([relative(command, &roots, home).as_str()], 1)
            .into_iter()
            .next()
            .unwrap_or_default();
        let row = json!({"cmd":command,"error":error_signature(out,&roots,home)?});
        if !failed.contains(&row) {
            failed.push(row);
        }
    }
    let relative_commands: Vec<_> = commands
        .iter()
        .map(|(c, _)| relative(c, &roots, home))
        .collect();
    let checks: Vec<_> = commands
        .iter()
        .zip(&relative_commands)
        .filter(|((c, _), _)| CHECK_RE.is_match(c))
        .map(|(_, c)| c.as_str())
        .collect();
    Ok(
        json!({"iid":iid,"resolved":if resolved.is_null() { Value::Null } else { json!(truthy(&resolved)) },
        "edited_files":edited_files(calls.iter().map(|(n,a,_)| (n.as_str(),a)),false),"n_cmds":commands.len(),
        "cmd_templates":templates(relative_commands.iter().map(String::as_str),12),"check_commands":templates(checks,12).into_iter().take(6).collect::<Vec<_>>(),
        "failed_commands":failed.into_iter().take(6).collect::<Vec<_>>(),"missing_command":MISSING_CMD.is_match(&out_blob),"missing_dependency":MISSING_DEP.is_match(&out_blob),"bad_usage":BAD_USAGE.is_match(&out_blob),
        "_ran_tests":commands.iter().any(|(c,_)| TEST.is_match(c)),"_cmd_blob":commands.iter().map(|(c,_)| c.as_str()).collect::<Vec<_>>().join("\n"),"_out_blob":out_blob}),
    )
}

pub fn reparse_cmds(digest: &Value) -> Vec<Command> {
    digest["_cmd_blob"]
        .as_str()
        .unwrap_or("")
        .split('\n')
        .enumerate()
        .map(|(i, c)| {
            (
                c.into(),
                if i == 0 {
                    digest["_out_blob"].as_str().unwrap_or("").into()
                } else {
                    String::new()
                },
            )
        })
        .collect()
}
