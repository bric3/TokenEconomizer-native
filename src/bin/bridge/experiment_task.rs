use super::job::{self, Channel};
use serde_json::{Value, json};
use std::path::Path;
use tokeneconomizer::experiments::swebench::{self, Failure, Runtime, Task};

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
    json!({"raised":error.kind,"message":error.message})
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
    if method == "run_task" {
        let args = args.clone();
        return job::start("swebench_task", move |channel| {
            let result = serde_json::from_value::<Task>(args)
                .map_err(|e| Failure::new("ValueError", e.to_string()))
                .and_then(|input| swebench::run_task(&input, &mut Transport(channel)));
            result.unwrap_or_else(failure)
        })
        .map(Some);
    }
    Ok(Some(match method {
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
