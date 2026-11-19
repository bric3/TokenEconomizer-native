use super::job::{self, Channel};
use serde_json::{Value, json};
use std::path::Path;
use tokeneconomizer::experiments::datasets;
use tokeneconomizer::experiments::swebench::{
    self, Failure, Runtime, Task, codex_config, codex_install, codex_trace, containers, controls,
    grading, guard_install, official, providers, settings, setup, wrappers,
};

struct Transport(Channel);
impl Runtime for Transport {
    fn call(&mut self, method: &str, arguments: Value) -> swebench::Result<Value> {
        let reply = self
            .0
            .call(method, arguments)
            .map_err(|e| Failure::new("RuntimeError", e))?;
        if let Some(error) = reply.get("error") {
            return Err(serde_json::from_value(error.clone())
                .map_err(|e| Failure::new("ValueError", e.to_string()))?);
        }
        Ok(reply["value"].clone())
    }
}
pub(super) fn failure(error: Failure) -> Value {
    let mut value = json!({"raised":error.kind,"message":error.message});
    if let Some(details) = error.details {
        value["details"] = *details;
    }
    value
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if let Some(method) = op.strip_prefix("experiment.datasets.") {
        if method == "constants" {
            return Ok(Some(datasets::constants()));
        }
        if method == "resume" {
            return job::advance(
                args["job"].as_u64().ok_or("missing dataset job")?,
                args.get("reply").cloned(),
            )
            .map(Some);
        }
        if !matches!(method, "build" | "swebench_main") {
            return Ok(None);
        }
        let method = method.to_owned();
        let args = args.clone();
        return job::start("dataset_export", move |channel| {
            let result = if method == "build" {
                serde_json::from_value(args["wanted"].clone())
                    .map_err(|e| Failure::new("TypeError", e.to_string()))
                    .and_then(|wanted| datasets::build(&mut Transport(channel), &wanted))
                    .map(|value| json!(value))
            } else {
                serde_json::from_value::<Vec<String>>(args["argv"].clone())
                    .map_err(|e| Failure::new("TypeError", e.to_string()))
                    .and_then(|argv| {
                        datasets::swebench_command(
                            &mut Transport(channel),
                            args["program"]
                                .as_str()
                                .unwrap_or("te-bench export-swebench"),
                            &argv,
                            Path::new(args["process_cwd"].as_str().unwrap_or(".")),
                        )
                    })
                    .map(|output| json!(output))
            };
            result.unwrap_or_else(failure)
        })
        .map(Some);
    }
    let Some(method) = op.strip_prefix("experiment.swebench.") else {
        return Ok(None);
    };
    if method == "resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing task job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    if method == "answer_key_attempts" {
        return Ok(Some(codex_trace::answer_key_attempts(
            Path::new(args["run_dir_path"].as_str().unwrap_or("")),
            Path::new(args["process_cwd"].as_str().unwrap_or(".")),
        )));
    }
    if let Some(method) = method.strip_prefix("control.") {
        let args = args.clone();
        let method = method.to_owned();
        return job::start("swebench_control", move |channel| {
            controls::dispatch(&method, &args, &mut Transport(channel)).unwrap_or_else(failure)
        })
        .map(Some);
    }
    if let Some(method) = method.strip_prefix("install.") {
        let args = args.clone();
        let method = method.to_owned();
        return job::start("codex_install", move |channel| {
            codex_install::dispatch(&method, &args, &mut Transport(channel)).unwrap_or_else(failure)
        })
        .map(Some);
    }
    if let Some(agent) = method.strip_prefix("setup.") {
        let agent = agent.to_owned();
        return job::start("swebench_setup", move |channel| {
            setup::construct(&agent, &mut Transport(channel)).unwrap_or_else(failure)
        })
        .map(Some);
    }
    if let Some(method) = method.strip_prefix("provider.") {
        let method = method.to_owned();
        return job::start("experiment_provider", move |channel| {
            providers::dispatch(&method, &mut Transport(channel)).unwrap_or_else(failure)
        })
        .map(Some);
    }
    if let Some(method) = method.strip_prefix("guard.") {
        let args = args.clone();
        let method = method.to_owned();
        return job::start("swebench_guard", move |channel| {
            guard_install::dispatch(&method, &args, &mut Transport(channel)).unwrap_or_else(failure)
        })
        .map(Some);
    }
    if let Some(agent) = method.strip_prefix("runner.") {
        let args = args.clone();
        let agent = agent.to_owned();
        return job::start("swebench_runner", move |channel| {
            wrappers::run(&agent, &args, &mut Transport(channel)).unwrap_or_else(failure)
        })
        .map(Some);
    }
    if let Some(method) = method.strip_prefix("codex.") {
        let args = args.clone();
        let method = method.to_owned();
        return job::start("swebench_codex", move |channel| {
            codex_config::dispatch(&method, &args, &mut Transport(channel)).unwrap_or_else(failure)
        })
        .map(Some);
    }
    if let Some(method) = method.strip_prefix("container.") {
        let args = args.clone();
        let method = method.to_owned();
        return job::start("swebench_container", move |channel| {
            containers::dispatch(&method, &args, &mut Transport(channel)).unwrap_or_else(failure)
        })
        .map(Some);
    }
    if matches!(
        method,
        "run_task"
            | "grade"
            | "eval_exec_failed"
            | "grader.eval_script"
            | "grader.score"
            | "write_run_config"
            | "patch_excludes"
    ) {
        let args = args.clone();
        let method = method.to_owned();
        return job::start("swebench_task", move |channel| {
            let mut runtime = Transport(channel);
            let result = match method.as_str() {
                "patch_excludes" => settings::patch_excludes(&mut runtime),
                "write_run_config" => serde_json::from_value::<settings::RunConfig>(args)
                    .map_err(|e| Failure::new("ValueError", e.to_string()))
                    .and_then(|input| settings::write_run_config(&input, &mut runtime)),
                "grader.eval_script" => official::eval_script(&args["instance_id"], &mut runtime),
                "grader.score" => official::score(
                    &args["instance_id"],
                    &args["model_patch"],
                    &args["log_path"],
                    &mut runtime,
                ),
                "grade" => serde_json::from_value::<grading::Grade>(args)
                    .map_err(|e| Failure::new("ValueError", e.to_string()))
                    .and_then(|input| grading::grade(&input, &mut runtime)),
                "eval_exec_failed" => serde_json::from_value::<grading::ExecFailure>(args)
                    .map_err(|e| Failure::new("ValueError", e.to_string()))
                    .and_then(|input| grading::eval_exec_failed(&input, &mut runtime)),
                _ => serde_json::from_value::<Task>(args)
                    .map_err(|e| Failure::new("ValueError", e.to_string()))
                    .and_then(|input| swebench::run_task(&input, &mut runtime)),
            };
            result.unwrap_or_else(failure)
        })
        .map(Some);
    }
    Ok(Some(match method {
        "env_activation_enabled" => json!(settings::env_activation_enabled(
            args["env"].as_object().ok_or("expected environment")?
        )),
        "network_tool_block" => json!(settings::network_tool_block(
            args["env"].as_object().ok_or("expected environment")?
        )),
        "_cpu_limit_args" => {
            settings::cpu_limit_args(args["env"].as_object().ok_or("expected environment")?)
                .map(|args| json!(args))
                .unwrap_or_else(failure)
        }
        "choose_project_env" => {
            settings::choose_project_env(&args["env_bins"]).unwrap_or_else(failure)
        }
        "activation_env" => settings::activation_env(&args["env_bin"], &args["container_path"])
            .unwrap_or_else(failure),
        "image_for" => settings::image_for(&args["instance_id"])
            .map(|image| json!(image))
            .unwrap_or_else(failure),
        "default_grading_path" => {
            official::default_grading_path(args["env"].as_object().ok_or("expected environment")?)
                .map(|path| json!(path))
                .unwrap_or_else(failure)
        }
        "swebench_repo" => official::swebench_repo(
            args["env"].as_object().ok_or("expected environment")?,
            Path::new(args["process_cwd"].as_str().ok_or("expected process cwd")?),
        )
        .map(|path| json!(path))
        .unwrap_or_else(failure),
        "grader.path" => official::grading_path(
            &args["grading_path"],
            args["env"].as_object().ok_or("expected environment")?,
        )
        .unwrap_or_else(failure),
        "grader.load" => official::load(
            Path::new(args["path"].as_str().ok_or("expected grading path")?),
            Path::new(args["process_cwd"].as_str().ok_or("expected process cwd")?),
        )
        .unwrap_or_else(failure),
        "grader.instance" => serde_json::from_value::<official::Grader>(args["self"].clone())
            .map_err(|e| Failure::new("ValueError", e.to_string()))
            .and_then(|grader| {
                grader.instance(
                    args["instance_id"]
                        .as_str()
                        .ok_or_else(|| Failure::new("TypeError", "expected string instance_id"))?,
                )
            })
            .unwrap_or_else(failure),
        "row_from_segment" => {
            swebench::row_from_segment(args["agent"].as_str().unwrap_or("claude"), &args["result"])
                .unwrap_or_else(failure)
        }
        "cell_of" => json!(swebench::cell_of(
            args["run_dir_path"].as_str().unwrap_or("")
        )),
        "build_prompt" => swebench::build_prompt(&args["task"], &args["injected"])
            .map(|v| json!(v))
            .unwrap_or_else(failure),
        "container_name" => json!(swebench::container_name(
            args["instance_id"].as_str().unwrap_or(""),
            &args["arm"],
            &args["agent"],
            &args["model"],
            Path::new(args["run_dir_path"].as_str().unwrap_or("")),
            Path::new(args["process_cwd"].as_str().unwrap_or(""))
        )),
        _ => return Err(format!("unknown task operation: {method}")),
    }))
}
