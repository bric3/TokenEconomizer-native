use std::path::Path;

use serde_json::{Map, Value, json};
use tokeneconomizer::plugin::{
    config::{self, LiveConfig, SettingValue},
    paths::{self, RepoId},
};
#[path = "live_adapter.rs"]
mod adapter_dispatch;
#[path = "cli.rs"]
mod cli_dispatch;
#[path = "context.rs"]
mod context_dispatch;
#[path = "controller.rs"]
mod controller_dispatch;
#[path = "denylist.rs"]
mod denylist_dispatch;
#[path = "fold.rs"]
mod fold_dispatch;
#[path = "history.rs"]
mod history_dispatch;
#[path = "hook.rs"]
mod hook_dispatch;
#[path = "host_session.rs"]
mod host_session_dispatch;
#[path = "hosts.rs"]
mod hosts_dispatch;
#[path = "live_judge.rs"]
mod judge_dispatch;
#[path = "hookqueue.rs"]
mod queue_dispatch;
#[path = "repomap.rs"]
mod repomap_dispatch;
#[path = "repos.rs"]
mod repos_dispatch;
#[path = "sidecall.rs"]
mod sidecall_dispatch;
#[path = "toolcheck.rs"]
mod toolcheck_dispatch;

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn object(value: &Value) -> Result<Map<String, Value>, String> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "expected object".into())
}
fn setting(args: &Value) -> Result<SettingValue, String> {
    if let Some(value) = args["nonfinite"].as_str() {
        value
            .parse::<f64>()
            .map(SettingValue::Float)
            .map_err(|e| e.to_string())
    } else {
        Ok(SettingValue::Json(args["value"].clone()))
    }
}

fn paths_dispatch(op: &str, args: &Value) -> Result<Value, String> {
    let environment = args["env"].as_object().cloned().unwrap_or_default();
    let home = Path::new(text(args, "home"));
    let path = |key| Path::new(text(args, key));
    Ok(match op {
        "paths.state_root" => json!(paths::state_root(&environment, home)?),
        "paths.secure_root" => {
            paths::secure_root(&environment, home)?;
            Value::Null
        }
        "paths.launcher_path" => json!(paths::launcher_path(&environment, home)?),
        "paths.ensure_launcher" => json!(paths::ensure_launcher(
            &environment,
            home,
            args["windows_python"].as_str()
        )?),
        "paths.ensure_native_launcher" => json!(paths::ensure_native_launcher(
            &environment,
            home,
            path("target"),
            args["windows"].as_bool().unwrap_or(false)
        )?),
        "paths.home_dir" => json!(
            paths::home_dir(&environment, home)?
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
        "paths._gitdir_of_file" => json!(paths::gitdir_of_file(path("dot"))?),
        "paths.is_git_entry" => json!(paths::is_git_entry(path("dot"))?),
        "paths.find_repo_root" => json!(paths::find_repo_root(path("start"), &environment, home)?),
        "paths._git_common_dir" => json!(paths::git_common_dir(path("root"))?),
        "paths.origin_url" => json!(paths::origin_url(path("root"))?),
        "paths.normalize_remote" => json!(paths::normalize_remote(text(args, "url"))),
        "paths._slug" => json!(paths::slug(text(args, "text"))),
        "paths.digest" => json!(paths::digest(text(args, "canon"))),
        "paths._main_checkout" => json!(paths::main_checkout(path("root"))?),
        "paths.repo_identity" => json!(paths::repo_identity(path("cwd"), &environment, home)?),
        "paths.repo_state_dir" => {
            let repo: RepoId =
                serde_json::from_value(args["repo"].clone()).map_err(|e| e.to_string())?;
            json!(paths::repo_state_dir(
                &repo,
                text(args, "host"),
                &environment,
                home
            )?)
        }
        _ => return Err(format!("unknown repository path operation: {op}")),
    })
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op.starts_with("cli.") || op.starts_with("probe.") {
        return cli_dispatch::dispatch(op, args);
    }
    if op.starts_with("hook.") {
        return hook_dispatch::dispatch(op, args);
    }
    if op.starts_with("repomap.") {
        return repomap_dispatch::dispatch(op, args);
    }
    if op.starts_with("toolcheck.") {
        return toolcheck_dispatch::dispatch(op, args);
    }
    if op.starts_with("sidecall.") {
        return sidecall_dispatch::dispatch(op, args);
    }
    if op.starts_with("controller.") {
        return controller_dispatch::dispatch(op, args);
    }
    if op.starts_with("host_runtime.") {
        return host_session_dispatch::dispatch(op, args);
    }
    if op.starts_with("fold.") {
        return fold_dispatch::dispatch(op, args);
    }
    if op.starts_with("context.") || op.starts_with("winspawn.") {
        return context_dispatch::dispatch(op, args);
    }
    if op.starts_with("host_helpers.") {
        return hosts_dispatch::dispatch(op, args).map(Some);
    }
    if op.starts_with("history.") {
        return history_dispatch::dispatch(op, args);
    }
    if op.starts_with("hookqueue.") {
        return queue_dispatch::dispatch(op, args);
    }
    if op.starts_with("live_judge.") {
        return judge_dispatch::dispatch(op, args);
    }
    if !op.starts_with("denylist.")
        && !op.starts_with("live_adapter.")
        && !op.starts_with("paths.")
        && !op.starts_with("live_config.")
        && !op.starts_with("repos.")
        && !op.starts_with("applied.")
    {
        return Ok(None);
    }
    let previous = std::env::current_dir().map_err(|e| e.to_string())?;
    if let Some(cwd) = args["process_cwd"].as_str() {
        std::env::set_current_dir(cwd).map_err(|e| e.to_string())?;
    }
    let result = if op.starts_with("live_adapter.") {
        adapter_dispatch::dispatch(op, args).map(Some)
    } else if op.starts_with("denylist.") {
        denylist_dispatch::dispatch(op, args).map(Some)
    } else if op.starts_with("repos.") || op.starts_with("applied.") {
        repos_dispatch::dispatch(op, args).map(Some)
    } else if op.starts_with("paths.") {
        paths_dispatch(op, args).map(Some)
    } else {
        config_dispatch(op, args)
    };
    std::env::set_current_dir(previous).map_err(|e| e.to_string())?;
    result
}

fn config_dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !op.starts_with("live_config.") {
        return Ok(None);
    }
    let value = match op {
        "live_config.new" => {
            let mut cfg = serde_json::to_value(LiveConfig::default()).unwrap();
            cfg.as_object_mut()
                .unwrap()
                .extend(object(&args["fields"])?);
            cfg
        }
        "live_config.canonical" => json!(config::canonical(text(args, "key"))),
        "live_config._threshold_name" => json!(config::threshold_name(text(args, "key"))),
        "live_config._read" => json!(config::read(Path::new(text(args, "path")))),
        "live_config._write" => {
            config::write(Path::new(text(args, "path")), &args["obj"])?;
            Value::Null
        }
        "live_config._old_names_first" => {
            let pairs: Vec<(String, Value)> =
                serde_json::from_value(args["items"].clone()).map_err(|e| e.to_string())?;
            json!(config::old_names_first(&pairs.into_iter().collect()))
        }
        "live_config._drop_old_names" => {
            let mut current = object(&args["current"])?;
            config::drop_old_names(&mut current, text(args, "key"));
            json!(current)
        }
        "live_config.load_config" => json!(config::load_config(
            Path::new(text(args, "root")),
            args["repo_dir"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(Path::new),
            &object(&args["env"])?
        )),
        "live_config.write_config" => json!(config::write_config(
            Path::new(text(args, "path")),
            &object(&args["updates"])?
        )?),
        "live_config.unset_config" => {
            let keys: Vec<String> =
                serde_json::from_value(args["keys"].clone()).map_err(|e| e.to_string())?;
            json!(config::unset_config(Path::new(text(args, "path")), &keys)?)
        }
        "live_config.environment" => json!(
            config::environment()
                .into_iter()
                .map(|(key, _)| key)
                .chain([
                    "TE_SPIRAL_THRESHOLDS".into(),
                    "TE_LOOPGUARD_THRESHOLDS".into(),
                    "TE_DISABLE".into()
                ])
                .collect::<Vec<_>>()
        ),
        _ => {
            let mut cfg: LiveConfig =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            let value = match op {
                "live_config.set" => {
                    json!(cfg.set(text(args, "key"), setting(args)?, text(args, "source")))
                }
                "live_config._set_threshold" => json!(cfg.set_threshold(
                    text(args, "name"),
                    setting(args)?,
                    text(args, "source")
                )),
                "live_config.as_dict" => cfg.as_dict(),
                _ => return Err(format!("unknown live configuration operation: {op}")),
            };
            json!({"self":cfg,"value":value})
        }
    };
    Ok(Some(value))
}
