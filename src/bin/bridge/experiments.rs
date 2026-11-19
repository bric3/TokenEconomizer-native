use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tokeneconomizer::experiments::{
    adapters::{self, Adapter, Error},
    arms,
    driver::parse_env,
    scaffolds,
    swebench::providers,
};
use tokeneconomizer::{ledger, microskills::distiller};

fn path(args: &Value, name: &str) -> Option<PathBuf> {
    args[name].as_str().map(PathBuf::from)
}
fn strings(args: &Value, name: &str) -> Result<Vec<String>, String> {
    serde_json::from_value(args[name].clone()).map_err(|e| e.to_string())
}
fn response(result: Result<Value, Error>) -> Value {
    match result {
        Ok(value) => json!({"value":value}),
        Err(error) => {
            let mut value = json!({"raised":error.kind(),"message":error.to_string()});
            if let Error::Utf8(error) = error {
                let utf8 = error.utf8_error();
                value["bytes"] = json!(error.as_bytes());
                value["start"] = json!(utf8.valid_up_to());
                value["end"] = json!(
                    utf8.valid_up_to()
                        + utf8
                            .error_len()
                            .unwrap_or(error.as_bytes().len() - utf8.valid_up_to())
                );
            }
            value
        }
    }
}
pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op == "experiment.providers.constants" {
        return Ok(Some(providers::constants()));
    }
    if op == "experiment.optimizer.constants" {
        return Ok(Some(providers::optimizer_constants()));
    }
    if op == "experiment.scaffolds.constants" {
        return Ok(Some(scaffolds::constants().map_err(|error| error.message)?));
    }
    if op == "experiment.driver.constants" {
        return Ok(Some(
            json!({"overhead_keys":ledger::OVERHEAD_KEYS,"propose_every":distiller::PROPOSE_EVERY,"description":"Run one TokenEconomizer arm over a task stream.\n\nUse --runner module:factory for a task harness. The default stub creates synthetic transcripts, grades and optimizer answers for wiring checks."}),
        ));
    }
    if op == "experiment.environment.parse" {
        return Ok(Some(json!(parse_env(
            args["text"].as_str().ok_or("expected environment text")?
        ))));
    }
    if op == "experiment.arms.registry" {
        let registry = arms::registry();
        let scaffolds: IndexMap<_, _> = registry
            .iter()
            .filter_map(|(name, arm)| arm.scaffold.map(|scaffold| (*name, scaffold)))
            .collect();
        return Ok(Some(
            json!({"arms":registry,"order":arms::ARM_ORDER,"scaffolds":scaffolds}),
        ));
    }
    if op == "experiment.arms.config" {
        return Ok(Some(json!(
            arms::get(args["name"].as_str().ok_or("expected arm name")?).map(|arm| arm.config)
        )));
    }
    let Some(method) = op.strip_prefix("experiment.adapters.") else {
        return Ok(None);
    };
    let value = match method {
        "constants" => {
            json!({"SWEBENCH_TASKS_ENV":adapters::SWEBENCH_TASKS_ENV,"PROJECTS_ROOT_ENV":adapters::PROJECTS_ROOT_ENV,
            "CREDDATA_ROOT_ENV":adapters::CREDDATA_ROOT_ENV,"REPOSVUL_ROOT_ENV":adapters::REPOSVUL_ROOT_ENV,
            "NETWORK_TOOLS":adapters::NETWORK_TOOLS,"CREDDATA_TASK_PROMPT":adapters::CREDDATA_TASK_PROMPT,
            "CREDDATA_CLASSIFY_DOMAIN":adapters::CREDDATA_CLASSIFY_DOMAIN,"REPOSVUL_TASK_PROMPT":adapters::REPOSVUL_TASK_PROMPT,"REPOSVUL_CLASSIFY_DOMAIN":adapters::REPOSVUL_CLASSIFY_DOMAIN})
        }
        "redact_gold_pointers" => json!(adapters::redact_gold_pointers(
            args["text"].as_str().unwrap_or("")
        )),
        "untool_exclusions" => {
            let used: BTreeSet<_> = strings(args, "used_tools")?.into_iter().collect();
            json!(adapters::untool_exclusions(
                &used,
                &strings(args, "extra_inapplicable")?
            ))
        }
        "new" => {
            let env = args["env"].as_object().ok_or("expected environment")?;
            let data = &args["arguments"];
            let adapter = match args["name"].as_str().unwrap_or("") {
                "swebench" => Adapter::swebench(path(data, "tasks_path"), env),
                "webbench" => Adapter::webbench(
                    data["project"].as_str().ok_or("expected project")?.into(),
                    path(data, "projects_root"),
                    env,
                ),
                "creddata" => Adapter::creddata(
                    path(data, "creddata_root"),
                    serde_json::from_value(data.get("repo_ids").cloned().unwrap_or(Value::Null))
                        .map_err(|e| e.to_string())?,
                    env,
                ),
                "reposvul" => Adapter::reposvul(
                    path(data, "reposvul_root"),
                    serde_json::from_value(
                        data.get("instance_ids").cloned().unwrap_or(Value::Null),
                    )
                    .map_err(|e| e.to_string())?,
                    env,
                ),
                name => return Err(format!("unknown dataset adapter: {name}")),
            };
            json!(adapter)
        }
        _ => {
            let mut adapter: Adapter =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            let cwd = Path::new(args["process_cwd"].as_str().unwrap_or(""));
            let path = cwd.join(args["path"].as_str().unwrap_or(""));
            let run_dir = cwd.join(args["run_dir_path"].as_str().unwrap_or(""));
            // File I/O follows the calling test process; public path methods keep their lexical values.
            if matches!(method, "problem_statements" | "repo_ids" | "instance_ids") {
                let root = match &mut adapter {
                    Adapter::Swebench { tasks_path } => tasks_path,
                    Adapter::Webbench { projects_root, .. } => projects_root,
                    Adapter::Creddata { creddata_root, .. } => creddata_root,
                    Adapter::Reposvul { reposvul_root, .. } => reposvul_root,
                };
                if let Some(root) = root.as_mut().filter(|p| !p.as_os_str().is_empty()) {
                    *root = cwd.join(&*root);
                }
            }
            let agent = args["agent"].as_str().unwrap_or("");
            response(match method {
                "tasks_path" => adapter.tasks_path().map(|p| json!(p)),
                "meta_dir" => adapter.meta_dir().map(|p| json!(p)),
                "instances_dir" => adapter.instances_dir().map(|p| json!(p)),
                "repo_ids" => adapter.repo_ids().map(|v| json!(v)),
                "instance_ids" => adapter.instance_ids().map(|v| json!(v)),
                "problem_statements" => adapter.problem_statements().map(|v| json!(v)),
                "classify_domain" => Ok(json!(adapter.classify_domain())),
                "env_vocabulary" => Ok(json!(adapter.env_vocabulary())),
                "load_trajectory" => Ok(json!(adapter.load_trajectory(&path))),
                "tool_usage" => Ok(json!(adapter.tool_usage(&path))),
                "parse_catalog" => Ok(json!(adapter.parse_catalog(&run_dir, agent))),
                "parse_catalog_with_source" => {
                    Ok(json!(adapter.parse_catalog_with_source(&run_dir, agent)))
                }
                "protected_tools" => Ok(json!(adapter.protected_tools().names)),
                "inapplicable_tools" => Ok(json!(adapter.inapplicable_tools())),
                "hardcoded_exclusions" => Ok(json!(
                    adapter
                        .hardcoded_exclusions(&strings(args, "used_tools")?.into_iter().collect())
                )),
                "spiral_snapshot" => Ok(adapter.spiral_snapshot(&path)),
                _ => return Err(format!("unknown dataset adapter operation: {method}")),
            })
        }
    };
    Ok(Some(value))
}
