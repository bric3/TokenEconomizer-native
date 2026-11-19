//! Export the driver task list in the registry's dependency order.
use super::field;
use crate::value::python_space;
use crate::{
    experiments::{
        CommandOutput,
        swebench::{Failure, Result, official},
        workloads,
    },
    plugin::cli::parser::is_option,
    python_json::Node,
    value::{python_repr, python_type},
};
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

pub use crate::experiments::adapters::PROJECTS_ROOT_ENV as PROJECTS_ENV;
pub const TASKS_ENV: &str = "TOKENECONOMIZER_WEBBENCH_TASKS";

pub fn constants() -> Value {
    json!({"PROJECTS_ENV":PROJECTS_ENV,"TASKS_ENV":TASKS_ENV})
}

pub fn build(projects_root: &Path, cwd: &Path) -> Result<Vec<Value>> {
    let groups =
        workloads::groups("webbench").map_err(|e| Failure::new(e.kind(), e.to_string()))?;
    let mut rows = Vec::new();
    for (project, ids) in groups {
        let path = projects_root.join(&project).join("tasks.jsonl");
        if !cwd.join(&path).is_file() {
            return Err(Failure::new(
                "SystemExit",
                format!(
                    "{project}: no tasks.jsonl at {}\n  yml-only project? run: python scripts/webbench_tasks_from_yaml.py {project}",
                    path.display()
                ),
            ));
        }
        let mut by_id = Map::new();
        // shortcut: whole-file decoding changes compound error order; stream if that fidelity is required.
        for line in official::load_text(&path, cwd)?.split('\n') {
            let line = line.trim_matches(python_space);
            if line.is_empty() {
                continue;
            }
            let row = official::parse(line)?;
            let id = field(&row, "id")?;
            if id.is_array() || id.is_object() {
                return Err(Failure::new(
                    "TypeError",
                    format!(
                        "cannot use '{}' as a dict key (unhashable type: '{}')",
                        python_type(id),
                        python_type(id)
                    ),
                ));
            }
            if let Some(id) = id.as_str() {
                by_id.insert(id.to_owned(), row);
            }
        }
        let missing: Vec<_> = ids.iter().filter(|id| !by_id.contains_key(*id)).collect();
        if !missing.is_empty() {
            return Err(Failure::new(
                "SystemExit",
                format!(
                    "{project}: tasks.jsonl is missing {} named by WEBBENCH_TASKS",
                    python_repr(&json!(missing))
                ),
            ));
        }
        for id in ids {
            rows.push(json!({"instance_id":id,"problem_statement":by_id[&id].get("description").cloned().unwrap_or(json!("")),"repo":project}));
        }
    }
    Ok(rows)
}

pub fn command(
    program: &str,
    args: &[String],
    env: &Map<String, Value>,
    cwd: &Path,
) -> Result<CommandOutput> {
    let usage = format!(
        "usage: {program} [-h] [--out OUT]\n{}[--projects-root PROJECTS_ROOT] [--check]\n",
        " ".repeat(program.len() + 8)
    );
    let fail = |message: String| CommandOutput {
        exit: Some(2),
        stderr: format!("{usage}{program}: error: {message}\n"),
        ..CommandOutput::default()
    };
    let mut out = None;
    let mut projects = None;
    let mut check = false;
    let mut unknown = Vec::new();
    let mut position = 0;
    while position < args.len() {
        let arg = &args[position];
        position += 1;
        if arg == "--" {
            unknown.push(arg.clone());
            unknown.extend_from_slice(&args[position..]);
            break;
        }
        let (name, explicit) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(a, b)| (a, Some(b)));
        let option = if name == "-h" || (name.starts_with("--h") && "--help".starts_with(name)) {
            "-h/--help"
        } else if name.starts_with("--o") && "--out".starts_with(name) {
            "--out"
        } else if name.starts_with("--p") && "--projects-root".starts_with(name) {
            "--projects-root"
        } else if name.starts_with("--c") && "--check".starts_with(name) {
            "--check"
        } else {
            unknown.push(arg.clone());
            continue;
        };
        if matches!(option, "-h/--help" | "--check") {
            if let Some(value) = explicit {
                return Ok(fail(format!(
                    "argument {option}: ignored explicit argument {}",
                    python_repr(&json!(value))
                )));
            }
            if option == "-h/--help" {
                // shortcut: use the source help's default width until the CLI formatting audit.
                return Ok(CommandOutput {
                    exit: Some(0),
                    stdout: format!("{usage}{}", include_str!("webbench_help.txt")),
                    ..CommandOutput::default()
                });
            }
            check = true;
        } else {
            let value = if let Some(value) = explicit {
                value.to_owned()
            } else if position < args.len() && args[position] != "--" && !is_option(&args[position])
            {
                position += 1;
                args[position - 1].clone()
            } else {
                return Ok(fail(format!("argument {option}: expected one argument")));
            };
            if option == "--out" {
                out = Some(value);
            } else {
                projects = Some(value);
            }
        }
    }
    if !unknown.is_empty() {
        return Ok(fail(format!(
            "unrecognized arguments: {}",
            unknown.join(" ")
        )));
    }
    let select = |value: Option<String>, key: &str| {
        value.filter(|s| !s.is_empty()).or_else(|| {
            env.get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
    };
    let projects = select(projects, PROJECTS_ENV).ok_or_else(|| {
        Failure::new(
            "SystemExit",
            format!("no Web-Bench projects root: set ${PROJECTS_ENV} or pass --projects-root"),
        )
    })?;
    let out = select(out, TASKS_ENV).ok_or_else(|| {
        Failure::new(
            "SystemExit",
            format!("no output path: set ${TASKS_ENV} or pass --out"),
        )
    })?;
    let rows = build(Path::new(&projects), cwd)?;
    let want = Node::from(&json!(rows)).json_utf8(true) + "\n";
    let path = Path::new(&out);
    let have = if cwd.join(path).is_file() {
        Some(official::load_text(path, cwd)?)
    } else {
        None
    };
    let projects = workloads::groups("webbench").expect("embedded WebBench groups");
    let mut names: Vec<_> = projects.keys().cloned().collect();
    names.sort();
    let summary = format!(
        "{} tasks, {} projects: {}\n",
        rows.len(),
        projects.len(),
        names.join(" ")
    );
    let stdout = if have.as_ref() == Some(&want) {
        format!("ok  {out}\n    {summary}")
    } else if check {
        return Err(Failure::new(
            "SystemExit",
            format!(
                "STALE  {out}  ({})\n       regenerate with: python scripts/export_webbench_tasks.py",
                if have.is_none() {
                    "missing"
                } else {
                    "differs from the registry"
                }
            ),
        ));
    } else {
        if out.contains('\0') {
            return Err(Failure::new("ValueError", "embedded null byte"));
        }
        fs::write(cwd.join(path), want).map_err(|e| Failure::for_path(e, path))?;
        format!("wrote {out}\n      {summary}")
    };
    Ok(CommandOutput {
        stdout,
        ..CommandOutput::default()
    })
}
