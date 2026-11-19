use super::{ALLOWLIST, Analyzer, Error, Item, Result, TEST_ONLY, UNREFERENCED};
use crate::value::{json_pretty, scalar_text, truthy};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};

fn field<'a>(item: &'a Value, key: &str) -> Result<&'a Value> {
    item.get(key).ok_or_else(|| Error::new("KeyError", key))
}
pub fn counts(items: &[Value]) -> Result<Value> {
    let mut counts = BTreeMap::new();
    for item in items {
        *counts
            .entry((
                scalar_text(field(item, "kind")?),
                scalar_text(field(item, "status")?),
            ))
            .or_insert(0usize) += 1;
    }
    let mut result = Map::new();
    for ((kind, status), count) in counts {
        result.insert(format!("{kind}/{status}"), json!(count));
    }
    Ok(Value::Object(result))
}
fn entry(item: &Value) -> Result<String> {
    Ok(format!(
        "- `{}:{}` {} `{}`",
        scalar_text(field(item, "path")?),
        scalar_text(field(item, "line")?),
        scalar_text(field(item, "kind")?),
        scalar_text(field(item, "name")?)
    ))
}
pub fn markdown(items: &[Value], fixed: bool) -> Result<String> {
    let mut out = vec!["# Dead-code sweep".to_owned(), String::new()];
    if items.is_empty() {
        out.push("Nothing unreferenced or test-only was found.".into());
        return Ok(out.join("\n") + "\n");
    }
    out.extend([format!("A name counts as used when it appears as a whole word in any tracked text file other than at its own definition. Allowlist false positives in `{ALLOWLIST}`."),String::new(),
        format!("| {:<8} | {:>12} | {:>9} |","kind","unreferenced","test-only"),"|----------|--------------|-----------|".into()]);
    for kind in ["function", "class", "method", "constant"] {
        let count = |status: &str| {
            items
                .iter()
                .filter(|item| item["kind"] == kind && item["status"] == status)
                .count()
        };
        out.push(format!(
            "| {kind:<8} | {:>12} | {:>9} |",
            count(UNREFERENCED),
            count(TEST_ONLY)
        ));
    }
    let mut first = "Unreferenced";
    if fixed {
        let removed: Vec<_> = items
            .iter()
            .filter(|item| truthy(&item["removed"]))
            .collect();
        out.extend([
            String::new(),
            format!("## Removed ({})", removed.len()),
            String::new(),
        ]);
        for item in removed {
            out.push(entry(item)?);
        }
        first = "Unreferenced, not removed";
    }
    for (status, title) in [
        (UNREFERENCED, first),
        (TEST_ONLY, "Test-only (left for a person)"),
    ] {
        let group: Vec<_> = items
            .iter()
            .filter(|item| item["status"] == status && (!fixed || !truthy(&item["removed"])))
            .collect();
        if group.is_empty() {
            continue;
        }
        out.extend([
            String::new(),
            format!("## {title} ({})", group.len()),
            String::new(),
        ]);
        for item in group {
            let mut why = Vec::new();
            if status == UNREFERENCED && !truthy(field(item, "removable")?) {
                why.push("its value calls something".into());
            }
            if truthy(&item["note"]) {
                why.push(scalar_text(&item["note"]));
            }
            let tail = if why.is_empty() {
                String::new()
            } else {
                format!(" ({})", why.join("; "))
            };
            out.push(entry(item)? + &tail);
        }
    }
    Ok(out.join("\n") + "\n")
}
pub fn public(items: &[Item]) -> Vec<Value> {
    items
        .iter()
        .map(|item| {
            let mut item = serde_json::to_value(item).unwrap();
            item.as_object_mut()
                .unwrap()
                .retain(|key, _| !key.starts_with('_'));
            item
        })
        .collect()
}

#[derive(Default, Serialize)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub argument_exit: bool,
}
fn usage(program: &str) -> String {
    format!("usage: {program} [-h] [--root ROOT] [--json] [--fix]\n")
}
pub fn run(program: &str, args: &[String], default_root: &Path, python: &OsStr) -> Result<Output> {
    let mut root = PathBuf::from(default_root);
    let mut fixed = false;
    let mut as_json = false;
    let mut index = 0;
    let fail = |message: &str| Output {
        code: 2,
        argument_exit: true,
        stderr: format!("{}{program}: error: {message}\n", usage(program)),
        ..Output::default()
    };
    let mut unknown = Vec::new();
    while index < args.len() {
        let arg = &args[index];
        index += 1;
        if arg == "--" {
            unknown.extend_from_slice(&args[index - 1..]);
            break;
        }
        let (name, value) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(key, value)| (key, Some(value)));
        let matches: Vec<_> = ["--root", "--json", "--fix", "--help"]
            .into_iter()
            .filter(|option| {
                *option == name || (name.starts_with("--") && option.starts_with(name))
            })
            .collect();
        let name = if name == "-h" {
            "--help"
        } else if matches.len() == 1 {
            matches[0]
        } else if matches.len() > 1 {
            return Ok(fail(&format!(
                "ambiguous option: {name} could match {}",
                matches.join(", ")
            )));
        } else {
            unknown.push(arg.clone());
            continue;
        };
        if name == "--root" {
            let value = if let Some(value) = value {
                value.to_owned()
            } else if index < args.len() && !args[index].starts_with('-') {
                let value = args[index].clone();
                index += 1;
                value
            } else {
                return Ok(fail("argument --root: expected one argument"));
            };
            root = PathBuf::from(value);
            continue;
        }
        if let Some(value) = value {
            return Ok(fail(&format!(
                "argument {name}: ignored explicit argument '{value}'"
            )));
        }
        match name {
            "--json" => as_json = true,
            "--fix" => fixed = true,
            "--help" => {
                return Ok(Output {
                    stdout: format!(
                        "{}\nList definitions nothing else in the repository names, and optionally delete\nthem.\n\noptions:\n  -h, --help   show this help message and exit\n  --root ROOT  the repository to scan (default: this one)\n  --json       print JSON instead of Markdown\n  --fix        delete the unreferenced definitions (never the test-only ones)\n",
                        usage(program)
                    ),
                    argument_exit: true,
                    ..Output::default()
                });
            }
            _ => unreachable!(),
        }
    }
    if !unknown.is_empty() {
        return Ok(fail(&format!(
            "unrecognized arguments: {}",
            unknown.join(" ")
        )));
    }
    let mut analyzer = Analyzer::new(python)?;
    let items = if fixed {
        analyzer.sweep(&root, 20)?
    } else {
        analyzer.scan(&root)?
    };
    let items = public(&items);
    let text = if as_json {
        json_pretty(&json!({"counts":counts(&items)?,"items":items}), false) + "\n"
    } else {
        markdown(&items, fixed)?
    };
    Ok(Output {
        stdout: analyzer.stdout().to_owned() + &text,
        stderr: analyzer.stderr().into(),
        ..Output::default()
    })
}
