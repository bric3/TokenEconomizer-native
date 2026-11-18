use super::job::{self, Channel};
use serde_json::{Value, json};
use std::path::Path;
use tokeneconomizer::experiments::swebench::{
    self, Failure, Runtime, Task, grading, official, settings,
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
fn failure(error: Failure) -> Value {
    let mut value = json!({"raised":error.kind,"message":error.message});
    if let Some(details) = error.details {
        value["details"] = *details;
    }
    value
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
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
