//! Published workload provenance and the runnable manifests for new experiments.
use super::{CommandOutput, adapters::Error, paths::relative};
use crate::value::{json_pretty, python_repr, python_type};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

type Result<T> = std::result::Result<T, Error>;
pub type Groups = IndexMap<String, Vec<String>>;

static DATA: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("assets/workloads.json")).expect("embedded workload data")
});

pub fn constants() -> &'static Value {
    &DATA["constants"]
}

pub fn domains() -> Vec<String> {
    serde_json::from_value(constants()["DOMAINS"].clone()).expect("embedded domains")
}

fn invalid(kind: &'static str, message: impl Into<String>) -> Error {
    Error::Invalid(kind, message.into())
}
fn key(message: impl Into<String>) -> Error {
    invalid("KeyError", message)
}

fn tuple_repr(values: &[String]) -> String {
    let items: Vec<_> = values.iter().map(|s| python_repr(&json!(s))).collect();
    format!(
        "({}{})",
        items.join(", "),
        if values.len() == 1 { "," } else { "" }
    )
}

pub fn groups(domain: &str) -> Result<Groups> {
    let data = match domain {
        "swebench" => constants()["SWEBENCH_INSTANCES"].clone(),
        "webbench" => constants()["WEBBENCH_TASKS"].clone(),
        "creddata" => json!({"creddata":constants()["CREDDATA_INSTANCES"]}),
        "reposvul" => json!({"reposvul":constants()["REPOSVUL_INSTANCES"]}),
        _ => return Err(key(domain)),
    };
    Ok(serde_json::from_value(data).expect("embedded groups"))
}

pub fn instances(domain: &str, group: Option<&str>) -> Result<Vec<String>> {
    let groups = groups(domain).map_err(|_| {
        key(format!(
            "unknown domain {}; expected one of {}",
            python_repr(&json!(domain)),
            tuple_repr(&domains())
        ))
    })?;
    if let Some(group) = group {
        return groups.get(group).cloned().ok_or_else(|| {
            key(format!(
                "{domain}: unknown group {}; expected one of {}",
                python_repr(&json!(group)),
                tuple_repr(&groups.keys().cloned().collect::<Vec<_>>())
            ))
        });
    }
    Ok(groups.into_values().flatten().collect())
}

pub fn runnable_instances(domain: &str) -> Result<Groups> {
    let mut groups = groups(domain)?;
    if domain == "swebench" {
        for (group, ids) in &mut groups {
            if let Some(added) = constants()["SWEBENCH_ADDED"][group].as_array() {
                ids.extend(
                    added
                        .iter()
                        .map(|id| id.as_str().expect("embedded instance").into()),
                );
            }
            ids.retain(|id| constants()["SWEBENCH_EXCLUDED"].get(id).is_none());
        }
    }
    Ok(groups)
}

pub fn task_counts(domain: &str) -> Result<IndexMap<String, usize>> {
    Ok(groups(domain)?
        .into_iter()
        .map(|(group, ids)| (group, ids.len()))
        .collect())
}

pub fn total_tasks(domain: &str) -> Result<usize> {
    Ok(instances(domain, None)?.len())
}

pub fn swebench_image_tag(instance_id: &str) -> String {
    format!(
        "swebench/sweb.eval.x86_64.{}:latest",
        instance_id.replace("__", "_1776_")
    )
}

/// Indices retain the caller's task records and their order, including repeated records.
pub fn filter_task_indices(
    tasks: &[Value],
    domain: &str,
    group: Option<&str>,
) -> Result<Vec<usize>> {
    let wanted: BTreeSet<_> = instances(domain, group)?.into_iter().collect();
    let mut indices = Vec::new();
    for (index, task) in tasks.iter().enumerate() {
        let task = task.as_object().ok_or_else(|| {
            invalid(
                "AttributeError",
                format!("'{}' object has no attribute 'get'", python_type(task)),
            )
        })?;
        let instance = task.get("instance_id").unwrap_or(&Value::Null);
        if instance.is_array() || instance.is_object() {
            return Err(invalid(
                "TypeError",
                format!(
                    "cannot use '{}' as a set element (unhashable type: '{}')",
                    python_type(instance),
                    python_type(instance)
                ),
            ));
        }
        if instance.as_str().is_some_and(|id| wanted.contains(id)) {
            indices.push(index);
        }
    }
    Ok(indices)
}

pub fn manifest(domain: &str) -> Result<Value> {
    let groups = runnable_instances(domain)?;
    let counts: IndexMap<_, _> = groups
        .iter()
        .map(|(group, ids)| (group.clone(), ids.len()))
        .collect();
    let seed = match domain {
        "swebench" => "SWEBENCH_SEEDS",
        "webbench" => "WEBBENCH_SEEDS",
        "creddata" => "CREDDATA_SEEDS",
        "reposvul" => "REPOSVUL_SEEDS",
        _ => return Err(key(domain)),
    };
    let mut doc = json!({
        "domain":domain,"description":DATA["descriptions"][domain],
        "source":"experiments/te_bench/workloads.py (regenerate: python scripts/export_workloads.py)",
        "group_field":DATA["group_fields"][domain],"seeds":constants()[seed],
        "total_tasks":counts.values().sum::<usize>(),"task_counts":counts,"groups":groups,
    });
    if domain == "swebench" {
        let mut excluded = constants()["SWEBENCH_EXCLUDED"].clone();
        excluded.sort_all_objects();
        if !excluded.as_object().unwrap().is_empty() {
            doc["excluded"] = excluded;
        }
        let mut added = constants()["SWEBENCH_ADDED"].clone();
        added.sort_all_objects();
        for ids in added.as_object_mut().unwrap().values_mut() {
            ids.as_array_mut()
                .unwrap()
                .sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        }
        if !added.as_object().unwrap().is_empty() {
            doc["added"] = added;
        }
        let mut full = constants()["SWEBENCH_FULL_SPLIT_ONLY"]
            .as_array()
            .unwrap()
            .clone();
        full.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        doc["full_split_only"] = json!(full);
        doc["notes"] = DATA["notes"].clone();
    }
    Ok(doc)
}

pub fn path_for(out: &Path, domain: &str) -> PathBuf {
    out.join(format!("{domain}.json"))
}

pub fn export(out: &Path, base: &Path, check: bool) -> Result<CommandOutput> {
    fs::create_dir_all(out).map_err(Error::Io)?;
    let mut stdout = String::new();
    let mut stale = Vec::new();
    let domains = domains();
    for domain in &domains {
        let doc = manifest(domain)?;
        let path = path_for(out, domain);
        let text = format!("{}\n", json_pretty(&doc, false));
        let label = relative(&path, base)
            .map_err(Error::Io)?
            .to_string_lossy()
            .into_owned();
        if check {
            let current = if path.exists() {
                let bytes = fs::read(&path).map_err(Error::Io)?;
                Some(
                    String::from_utf8(bytes)
                        .map_err(Error::Utf8)?
                        .replace("\r\n", "\n")
                        .replace('\r', "\n"),
                )
            } else {
                None
            };
            if current.as_deref() != Some(text.as_str()) {
                stale.push(label);
            }
        } else {
            fs::write(&path, text).map_err(Error::Io)?;
            stdout.push_str(&format!(
                "wrote {label}  ({} tasks, {} groups)\n",
                doc["total_tasks"],
                doc["groups"].as_object().unwrap().len()
            ));
        }
    }
    let code = if check && !stale.is_empty() {
        stdout.push_str("STALE (run: python scripts/export_workloads.py):\n  ");
        stdout.push_str(&stale.join("\n  "));
        stdout.push('\n');
        1
    } else {
        if check {
            stdout.push_str(&format!(
                "workloads/*.json up to date ({} domains)\n",
                domains.len()
            ));
        }
        0
    };
    Ok(CommandOutput {
        stdout,
        returncode: Some(code),
        ..CommandOutput::default()
    })
}

pub fn export_command(
    program: &str,
    args: &[String],
    out: &Path,
    base: &Path,
) -> Result<CommandOutput> {
    let usage = format!("usage: {program} [-h] [--check]\n");
    let fail = |message: &str| CommandOutput {
        exit: Some(2),
        stderr: format!("{usage}{program}: error: {message}\n"),
        ..CommandOutput::default()
    };
    let mut check = false;
    let mut unknown = Vec::new();
    let mut positional_only = false;
    for arg in args {
        if !positional_only
            && (arg == "-h" || arg == "--help" || arg == "--h" || arg == "--he" || arg == "--hel")
        {
            // shortcut: retain the source help's default width until the CLI formatting audit.
            return Ok(CommandOutput {
                exit: Some(0),
                stdout: format!("{usage}{}", include_str!("workloads_help.txt")),
                ..CommandOutput::default()
            });
        }
        if !positional_only && arg == "--" {
            positional_only = true;
            continue;
        }
        if !positional_only && arg.starts_with("--c") && "--check".starts_with(arg.as_str()) {
            check = true;
        } else if !positional_only && arg.starts_with("--check=") {
            return Ok(fail(&format!(
                "argument --check: ignored explicit argument {}",
                python_repr(&json!(&arg[8..]))
            )));
        } else {
            unknown.push(arg.clone());
        }
    }
    if !unknown.is_empty() {
        return Ok(fail(&format!(
            "unrecognized arguments: {}",
            unknown.join(" ")
        )));
    }
    export(out, base, check)
}
