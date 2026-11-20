use serde_json::{Map, Value, json};
use std::path::Path;
use tokeneconomizer::plugin::hosts::session::{self, Environment};

pub fn host_call(
    host: &str,
    variables: &Map<String, Value>,
    env: &Environment,
    method: &str,
    args: &Value,
    runtime: Option<&mut (dyn session::Runtime + 'static)>,
) -> session::Result<Value> {
    let mut native = session::NativeRuntime { environment: env };
    let runtime: &mut dyn session::Runtime = match runtime {
        Some(runtime) => runtime,
        None => &mut native,
    };
    let path = || env.path(Path::new(args[0].as_str().unwrap()));
    Ok(match method {
        "history" => json!(session::history(
            host,
            variables,
            &args[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>(),
            env
        )?),
        "log_head" => session::log_head(host, &path()),
        "safe_session_id" => json!(session::safe_session_id(host, &args[0])),
        "folder_matches" => json!(session::folder_matches(
            host,
            Path::new(args[0].as_str().unwrap()),
            &args[1]
        )?),
        "prompt_count" => json!(session::prompt_count(host, &path())),
        "prompt_turns" => json!(session::prompt_turns(host, &path())),
        "turn_calls" => json!(session::turn_calls(
            &args[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        )),
        "turn_interrupted" => json!(session::turn_interrupted(
            &args[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        )),
        "turn_answered" => json!(session::turn_answered(
            &args[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        )),
        "mechanism_trace" => json!(session::mechanism_trace(
            host,
            args[0].as_str(),
            Path::new(args[1].as_str().unwrap()),
            env
        )?),
        "session_usage" => session::session_usage(host, &path())?,
        "complete" => session::complete(
            host,
            args[0].as_str(),
            args[1].as_str().unwrap(),
            args[2].as_str().map(Path::new),
            &args[3],
            env,
            &session::Limits::default(),
            runtime,
        )?,
        "settle" => {
            session::settle(host, args[0].as_str().map(Path::new), 15.0, 60.0, runtime)?;
            Value::Null
        }
        "last_active" => json!(session::last_active(host, &path(), &args[1])?),
        _ => panic!("unexpected host operation: {method} {args}"),
    })
}
