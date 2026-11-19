//! SWE-bench dataset selection and the files consumed by the benchmark harness.
use super::{
    CommandOutput,
    swebench::{Failure, Result, Runtime},
    workloads,
};
use crate::{
    plugin::cli::parser::is_option,
    value::{json_text, python_repr, python_type},
};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub const SPLITS: &[&str] = &[
    "princeton-nlp/SWE-bench_Verified",
    "princeton-nlp/SWE-bench",
];
pub const GRADING_FIELDS: &[&str] = &[
    "repo",
    "base_commit",
    "environment_setup_commit",
    "version",
    "test_patch",
    "patch",
    "FAIL_TO_PASS",
    "PASS_TO_PASS",
];

pub fn constants() -> Value {
    json!({"splits":SPLITS,"grading_fields":GRADING_FIELDS})
}

fn field<'a>(row: &'a Value, key: &str) -> Result<&'a Value> {
    match row {
        Value::Object(row) => row.get(key).ok_or_else(|| Failure::new("KeyError", key)),
        Value::Array(_) => Err(Failure::new(
            "TypeError",
            "list indices must be integers or slices, not str",
        )),
        Value::String(_) => Err(Failure::new(
            "TypeError",
            "string indices must be integers, not 'str'",
        )),
        value => Err(Failure::new(
            "TypeError",
            format!("'{}' object is not subscriptable", python_type(value)),
        )),
    }
}

/// Rust requests each row so a malformed row stops its iterator before the next read.
pub fn build(
    rt: &mut dyn Runtime,
    wanted: &BTreeSet<String>,
) -> Result<(Vec<Value>, Map<String, Value>)> {
    let mut found = Map::new();
    rt.call("rows_begin", Value::Null)?;
    loop {
        let split = rt.call("next_split", Value::Null)?;
        let split = split
            .as_bool()
            .ok_or_else(|| Failure::new("RuntimeError", "dataset cursor must return a boolean"))?;
        if !split {
            break;
        }
        loop {
            let mut next = rt.call("next_row", Value::Null)?;
            let done = next["done"]
                .as_bool()
                .ok_or_else(|| Failure::new("RuntimeError", "invalid dataset cursor response"))?;
            if done {
                break;
            }
            let row = next
                .get_mut("row")
                .ok_or_else(|| Failure::new("RuntimeError", "dataset cursor omitted its row"))?
                .take();
            let id = field(&row, "instance_id")?;
            if id.is_array() || id.is_object() {
                return Err(Failure::new(
                    "TypeError",
                    format!(
                        "cannot use '{}' as a set element (unhashable type: '{}')",
                        python_type(id),
                        python_type(id)
                    ),
                ));
            }
            if let Some(id) = id.as_str().filter(|id| wanted.contains(*id)) {
                found.entry(id.to_owned()).or_insert(row);
            }
        }
    }
    let order = workloads::runnable_instances("swebench")
        .map_err(|e| Failure::new(e.kind(), e.to_string()))?;
    let order: Vec<_> = order
        .into_values()
        .flatten()
        .filter(|id| found.contains_key(id))
        .collect();
    let mut tasks = Vec::new();
    for id in &order {
        tasks.push(json!({"instance_id":id,"repo":field(&found[id],"repo")?,"problem_statement":field(&found[id],"problem_statement")?}));
    }
    let mut grading = Map::new();
    for id in order {
        let mut row = Map::new();
        for key in GRADING_FIELDS {
            row.insert((*key).into(), field(&found[&id], key)?.clone());
        }
        grading.insert(id, Value::Object(row));
    }
    Ok((tasks, grading))
}

pub fn parse_swebench(
    program: &str,
    args: &[String],
) -> std::result::Result<PathBuf, CommandOutput> {
    let usage = format!("usage: {program} [-h] --out-dir OUT_DIR\n");
    let fail = |message: &str| CommandOutput {
        exit: Some(2),
        stderr: format!("{usage}{program}: error: {message}\n"),
        ..CommandOutput::default()
    };
    let mut out = None;
    let mut unknown = Vec::new();
    let mut position = 0;
    while position < args.len() {
        let arg = &args[position];
        position += 1;
        if arg == "--" {
            unknown.extend_from_slice(&args[position..]);
            break;
        }
        let (name, explicit) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(a, b)| (a, Some(b)));
        if name == "-h" || (name.starts_with("--h") && "--help".starts_with(name)) {
            if let Some(value) = explicit {
                return Err(fail(&format!(
                    "argument -h/--help: ignored explicit argument {}",
                    python_repr(&json!(value))
                )));
            }
            // shortcut: retain the source help's default width until the CLI formatting audit.
            return Err(CommandOutput {
                exit: Some(0),
                stdout: format!("{usage}{}", include_str!("datasets_help.txt")),
                ..CommandOutput::default()
            });
        }
        if name.starts_with("--o") && "--out-dir".starts_with(name) {
            if let Some(value) = explicit {
                out = Some(PathBuf::from(value));
            } else if position < args.len() && !is_option(&args[position]) && args[position] != "--"
            {
                out = Some(PathBuf::from(&args[position]));
                position += 1;
            } else {
                return Err(fail("argument --out-dir: expected one argument"));
            }
        } else {
            unknown.push(arg.clone());
        }
    }
    let out = out.ok_or_else(|| fail("the following arguments are required: --out-dir"))?;
    if !unknown.is_empty() {
        return Err(fail(&format!(
            "unrecognized arguments: {}",
            unknown.join(" ")
        )));
    }
    Ok(out)
}

pub fn export_swebench(rt: &mut dyn Runtime, out: &Path, cwd: &Path) -> Result<CommandOutput> {
    rt.call("dataset_splits", json!({"paths":SPLITS,"split":"test"}))?;
    let wanted: BTreeSet<_> = workloads::runnable_instances("swebench")
        .map_err(|e| Failure::new(e.kind(), e.to_string()))?
        .into_values()
        .flatten()
        .collect();
    let (tasks, grading) = build(rt, &wanted)?;
    let missing: Vec<_> = wanted
        .iter()
        .filter(|id| !grading.contains_key(*id))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(Failure::new(
            "SystemExit",
            format!(
                "{} evaluated ids are in neither split: {} ...",
                missing.len(),
                python_repr(&json!(&missing[..missing.len().min(5)]))
            ),
        ));
    }
    // An empty output argument raises the filesystem's missing-path error.
    if out.as_os_str().is_empty() {
        fs::metadata(out).map_err(|e| Failure::for_path(e, out))?;
    }
    fs::create_dir_all(cwd.join(out)).map_err(|e| Failure::for_path(e, out))?;
    for (name, contents) in [
        ("swebench_tasks.json", json_text(&json!(tasks))),
        ("swebench_grading.json", json_text(&json!(grading))),
    ] {
        let path = out.join(name);
        fs::write(cwd.join(&path), contents).map_err(|e| Failure::for_path(e, &path))?;
    }
    let mut tags = String::new();
    for task in &tasks {
        tags.push_str(&workloads::swebench_image_tag(
            task["instance_id"].as_str().unwrap(),
        ));
        tags.push('\n');
    }
    let path = out.join("image_tags.txt");
    fs::write(cwd.join(&path), tags).map_err(|e| Failure::for_path(e, &path))?;
    Ok(CommandOutput {
        stdout: format!(
            "wrote {} tasks, their grading fields and image tags to {}\n",
            tasks.len(),
            out.display()
        ),
        ..CommandOutput::default()
    })
}

pub fn swebench_command(
    rt: &mut dyn Runtime,
    program: &str,
    args: &[String],
    cwd: &Path,
) -> Result<CommandOutput> {
    match parse_swebench(program, args) {
        Ok(out) => export_swebench(rt, &out, cwd),
        Err(output) => Ok(output),
    }
}
