use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tokeneconomizer::experiments::{
    adapters::{self, Adapter, Error},
    aggregate, arms, compare,
    driver::parse_env,
    launch, scaffolds,
    swebench::Failure,
    swebench::providers,
    swebench::setup,
    swebench::web_contract,
    sweep, workloads,
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
    if op == "experiment.launch.constants" {
        return Ok(Some(json!({"functions":launch::FUNCTIONS})));
    }
    if op == "experiment.launch.function" {
        let context: launch::Context =
            serde_json::from_value(args["context"].clone()).map_err(|e| e.to_string())?;
        return context
            .function(
                args["name"].as_str().ok_or("missing launch function")?,
                &strings(args, "args")?,
            )
            .map(|v| Some(json!(v)));
    }
    if let Some(method) = op.strip_prefix("experiment.compare.") {
        let cwd = Path::new(args["process_cwd"].as_str().unwrap_or("."));
        let result = match method {
            "load_cell" => {
                compare::load_cell(Path::new(args["cell"].as_str().ok_or("missing cell")?), cwd)
                    .map(|cell| json!(cell))
            }
            "cost" => compare::cost(&args["r"]),
            "describe" => {
                let rows =
                    serde_json::from_value(args["rows"].clone()).map_err(|e| e.to_string())?;
                let configs =
                    serde_json::from_value(args["cfgs"].clone()).map_err(|e| e.to_string())?;
                compare::describe(
                    &rows,
                    &configs,
                    args["label"].as_str().ok_or("missing label")?,
                )
                .map(|text| json!(text))
            }
            "main" => compare::command(
                args["program"].as_str().ok_or("missing program")?,
                &strings(args, "argv")?,
                cwd,
            )
            .map(|output| json!(output)),
            _ => return Ok(None),
        };
        return Ok(Some(result.unwrap_or_else(|mut error| {
            let stdout = error.stdout.take();
            let mut value = super::task_dispatch::failure(error);
            if let Some(stdout) = stdout {
                value["stdout"] = *stdout;
            }
            value
        })));
    }
    if let Some(method) = op.strip_prefix("experiment.sweep.") {
        let root = Path::new(args["root"].as_str().unwrap_or("."));
        let cwd = Path::new(args["process_cwd"].as_str().unwrap_or("."));
        let result = match method {
            "constants" => Ok(sweep::constants()),
            "group_sizes" => sweep::group_sizes(
                args["domain"].as_str().ok_or("expected domain")?,
                path(args, "manifest").as_deref(),
                root,
                cwd,
            )
            .map(|sizes| json!(sizes)),
            "cells" => Ok(json!(sweep::cells(
                &strings(args, "arms")?,
                &strings(args, "repos")?,
                args["seeds"].as_array().ok_or("expected seeds")?
            ))),
            "estimate" => serde_json::from_value::<sweep::Sweep>(args.clone())
                .map_err(|e| Failure::new("ValueError", e.to_string()))
                .and_then(|sweep| sweep.estimate(root, cwd)),
            "_fmt_estimate" => {
                sweep::format_estimate(&args["e"], args["show_cost"].as_bool().unwrap_or(false))
                    .map(|text| json!(text))
            }
            "main" => sweep::command(
                args["program"].as_str().ok_or("expected program")?,
                &strings(args, "argv")?,
                root,
                cwd,
            )
            .map(|output| json!(output)),
            _ => return Ok(None),
        };
        return Ok(Some(result.unwrap_or_else(super::task_dispatch::failure)));
    }
    if let Some(method) = op.strip_prefix("experiment.workloads.") {
        let domain = args["domain"].as_str().unwrap_or("");
        let group = args["group"].as_str();
        let value = match method {
            "constants" => Ok(workloads::constants().clone()),
            "instances" => workloads::instances(domain, group).map(|ids| json!(ids)),
            "runnable_instances" => workloads::runnable_instances(domain).and_then(|groups| {
                if let Some(group) = group {
                    groups
                        .get(group)
                        .map(|ids| json!(ids))
                        .ok_or_else(|| Error::Invalid("KeyError", group.into()))
                } else {
                    Ok(json!(groups))
                }
            }),
            "task_counts" => workloads::task_counts(domain).map(|counts| json!(counts)),
            "total_tasks" => workloads::total_tasks(domain).map(|count| json!(count)),
            "swebench_image_tag" => Ok(json!(workloads::swebench_image_tag(
                args["instance_id"].as_str().ok_or("expected instance_id")?
            ))),
            "filter_tasks" => workloads::filter_task_indices(
                args["tasks"].as_array().ok_or("expected tasks")?,
                domain,
                group,
            )
            .map(|indices| json!(indices)),
            "build" => workloads::manifest(domain),
            "path_for" => Ok(json!(
                workloads::path_for(
                    Path::new(args["out"].as_str().ok_or("expected out")?),
                    domain
                )
                .to_string_lossy()
            )),
            "main" => workloads::export_command(
                args["program"].as_str().ok_or("expected program")?,
                &strings(args, "argv")?,
                Path::new(args["out"].as_str().ok_or("expected out")?),
                Path::new(args["base"].as_str().ok_or("expected base")?),
            )
            .map(|output| json!(output)),
            _ => return Ok(None),
        };
        return Ok(Some(response(value)));
    }
    if let Some(method) = op.strip_prefix("experiment.aggregate.") {
        let text = |key: &str| args[key].as_str().ok_or_else(|| format!("expected {key}"));
        let mut output = Vec::new();
        let value = match method {
            "constants" => {
                json!({"fields":aggregate::FIELDS,"usd_per_effective_token":aggregate::USD_PER_EFFECTIVE_TOKEN,"agents_without_vendor_usd":aggregate::AGENTS_WITHOUT_VENDOR_USD})
            }
            "usage_incomplete" => json!(aggregate::usage_incomplete(&args["d"])),
            "not_graded" => json!(aggregate::not_graded(&args["d"])),
            "vendor_usd" => aggregate::vendor_usd(&args["d"]),
            "vendor_cost_missing" => json!(aggregate::vendor_cost_missing(&args["d"])),
            "normalize_repo" => json!(aggregate::normalize_repo(text("rs")?, text("inst")?)),
            "cell_of" => json!(
                aggregate::cell_of(
                    Path::new(text("path")?),
                    Path::new(text("root")?),
                    text("arm")?
                )
                .map_err(|e| e.to_string())?
            ),
            "_run_order" => json!(aggregate::run_order(Path::new(text("rj")?))),
            "discover_arms" => json!(
                aggregate::discover_arms(Path::new(text("root")?)).map_err(|e| e.to_string())?
            ),
            "collect" => {
                let rows = aggregate::collect(
                    Path::new(text("root")?),
                    &strings(args, "arms")?,
                    &mut output,
                )?;
                json!({"rows":rows,"stdout":String::from_utf8(output).unwrap()})
            }
            "summarize" => json!(
                aggregate::summarize(args["rows"].as_array().ok_or("expected rows")?)?
                    .into_iter()
                    .map(|(key, value)| json!({"key":key,"value":value}))
                    .collect::<Vec<_>>()
            ),
            "main" => json!(aggregate::command(
                text("program")?,
                &strings(args, "argv")?
            )?),
            _ => return Ok(None),
        };
        return Ok(Some(value));
    }
    if op == "experiment.providers.constants" {
        return Ok(Some(providers::constants()));
    }
    if op == "experiment.optimizer.constants" {
        return Ok(Some(providers::optimizer_constants()));
    }
    if op == "experiment.scaffolds.constants" {
        return Ok(Some(scaffolds::constants().map_err(|error| error.message)?));
    }
    if op == "experiment.harness.constants" {
        return Ok(Some(setup::constants()));
    }
    if op == "experiment.webbench.constants" {
        return Ok(Some(web_contract::constants()));
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
