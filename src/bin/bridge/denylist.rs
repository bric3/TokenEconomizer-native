use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::{
    io,
    path::{Path, PathBuf},
};
use tokeneconomizer::{
    plugin::denylist::{self, Error, Runtime},
    store::Store,
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn strings(args: &Value, key: &str) -> Result<Vec<String>, Error> {
    if args[key].is_null() {
        Ok(Vec::new())
    } else {
        serde_json::from_value(args[key].clone()).map_err(|e| Error::Refused(e.to_string()))
    }
}
fn response(result: Result<Value, Error>) -> Value {
    match result {
        Ok(value) => json!({"value":value}),
        Err(Error::Io(error)) => crate::storage_dispatch::response(Err(error)),
        Err(Error::Refused(error)) => json!({"raised":"RuntimeError","message":error}),
    }
}

struct Transport {
    flags: Value,
    channel: Channel,
}
impl Transport {
    fn call(&self, method: &str, args: Value) -> Result<Value, Error> {
        let reply = self.channel.call(method, args).map_err(Error::Refused)?;
        if let Some(message) = reply["error"].as_str() {
            let error = match reply["error_kind"].as_str() {
                Some("PermissionError") => Error::Io(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    message.to_string(),
                )),
                Some("FileNotFoundError") => {
                    Error::Io(io::Error::new(io::ErrorKind::NotFound, message.to_string()))
                }
                _ => Error::Refused(message.to_string()),
            };
            return Err(error);
        }
        Ok(reply["value"].clone())
    }
}
impl Runtime for Transport {
    fn write_record(&mut self, store: &Store, doc: &Value) -> Result<(), Error> {
        if self.flags["store_write"] == true {
            self.call("store_write", json!(["untool_applied.json", doc]))?;
        } else {
            store.write("untool_applied.json", doc)?;
        }
        Ok(())
    }
    fn write_json(&mut self, path: &Path, doc: &Value) -> Result<(), Error> {
        if self.flags["write_json"] == true {
            self.call("write_json", json!([path, doc]))?;
            Ok(())
        } else {
            denylist::write_json(path, doc)
        }
    }
    fn write_text(&mut self, path: &Path, value: &str) -> Result<(), Error> {
        if self.flags["_write_text"] == true {
            self.call("_write_text", json!([path, value]))?;
            Ok(())
        } else {
            denylist::write_text(path, value)
        }
    }
    fn git_tracked(&mut self, root: &Path, relative: &str) -> Result<bool, Error> {
        if self.flags["_git_tracked"] == true {
            Ok(self.call("_git_tracked", json!([root, relative]))? == true)
        } else {
            Ok(denylist::git_tracked(root, relative))
        }
    }
    fn codex_config_error(
        &mut self,
        root: &Path,
        cli: &str,
        timeout: f64,
    ) -> Result<Option<String>, Error> {
        if self.flags["codex_config_error"] == true {
            Ok(self
                .call("codex_config_error", json!([root, cli, timeout]))?
                .as_str()
                .map(String::from))
        } else {
            Ok(denylist::codex_config_error(root, cli, timeout))
        }
    }
}

fn execute(op: &str, args: &Value, runtime: &mut dyn Runtime) -> Result<Value, Error> {
    let root = Path::new(text(args, "repo_root"));
    let host = args["host"].as_str().unwrap_or("claude");
    let path = Path::new(text(args, "path"));
    let environment = args["env"].as_object().cloned().unwrap_or_default();
    let home = Path::new(text(args, "home"));
    let st = || Store::new(Path::new(text(&args["store"], "base"))).map_err(Error::Io);
    Ok(match op {
        "denylist.constants" => json!({"capabilities":denylist::CODEX_LIVE_CAPABILITIES}),
        "denylist.supports" => json!(denylist::supports(host)),
        "denylist.unit" => json!(denylist::unit(host, args["plural"] == true)),
        "denylist.settings_path" => json!(denylist::settings_path(root, host)),
        "denylist._read_settings" => denylist::read_settings(path)?,
        "denylist._guard_target" => {
            denylist::guard_target(root, path)?;
            Value::Null
        }
        "denylist.write_json" => {
            denylist::write_json(path, &args["obj"])?;
            Value::Null
        }
        "denylist._write_text" => {
            denylist::write_text(path, text(args, "text"))?;
            Value::Null
        }
        "denylist._exclude_from_git" => json!(denylist::exclude_from_git(
            root,
            args["rel"].as_str().unwrap_or(denylist::SETTINGS_REL)
        )?),
        "denylist.kept_for" => json!(denylist::kept_for(&st()?, root, host)?),
        "denylist.set_kept" => {
            denylist::set_kept(
                &st()?,
                root,
                host,
                &strings(args, "add")?,
                &strings(args, "remove")?,
                runtime,
            )?;
            Value::Null
        }
        "denylist.applied" => json!(denylist::applied(
            &st()?,
            args["path"].as_str().map(Path::new)
        )?),
        "denylist.applied_for" => json!(denylist::applied_for(&st()?, root, host)?),
        "denylist._record" => {
            denylist::record(&st()?, &strings(args, "items")?, path, runtime)?;
            Value::Null
        }
        "denylist._json_list_keys" => json!(denylist::json_list_keys(host)),
        "denylist._apply_json_list" => json!(denylist::apply_json_list(
            root,
            &strings(args, "items")?,
            &st()?,
            host,
            runtime
        )?),
        "denylist._restore_json_list" => json!(denylist::restore_json_list(
            root,
            &st()?,
            &strings(args, "items")?,
            host,
            runtime
        )?),
        "denylist._sections" => json!(denylist::toml::sections(&strings(args, "lines")?)),
        "denylist._key_set" => json!(denylist::toml::key_set(
            &strings(args, "lines")?,
            args["table"].as_str(),
            text(args, "key")
        )),
        "denylist._toml_insert" => json!(denylist::toml::insert(
            text(args, "text"),
            args["table"].as_str(),
            text(args, "key"),
            text(args, "value"),
            text(args, "tag")
        )),
        "denylist._toml_remove" => json!(denylist::toml::remove(
            text(args, "text"),
            &strings(args, "tags")?
        )),
        "denylist._toml_valid" => json!(denylist::toml::valid(text(args, "text"))),
        "denylist._git_tracked" => json!(denylist::git_tracked(root, text(args, "rel"))),
        "denylist.codex_config_error" => json!(denylist::codex_config_error(
            root,
            args["cli"].as_str().unwrap_or("codex"),
            args["timeout"].as_f64().unwrap_or(30.0)
        )),
        "denylist._apply_codex" => json!(denylist::apply_codex(
            root,
            &strings(args, "capabilities")?,
            &st()?,
            runtime
        )?),
        "denylist._restore_codex" => json!(denylist::restore_codex(
            root,
            &st()?,
            &strings(args, "capabilities")?,
            runtime
        )?),
        "denylist.copilot_trusted_folders" => {
            json!(denylist::copilot_trusted_folders(&environment, home))
        }
        "denylist.copilot_folder_trusted" => {
            json!(denylist::copilot_folder_trusted(path, &environment, home)?)
        }
        "denylist.codex_home" => json!(denylist::codex_home(&environment, home)),
        "denylist.codex_project_trusted" => {
            json!(denylist::codex_project_trusted(root, &environment, home)?)
        }
        "denylist.apply" => json!(denylist::apply(
            root,
            &strings(args, "tools")?,
            &st()?,
            host,
            runtime
        )?),
        "denylist.restore" => json!(denylist::restore(
            root,
            &st()?,
            &strings(args, "tools")?,
            host,
            runtime
        )?),
        "denylist.copilot_server_proposal" => json!(denylist::copilot_server_proposal(
            &strings(args, "raw")?,
            &strings(args, "catalog")?,
            &strings(args, "used")?,
            &strings(args, "servers")?
        )),
        _ => return Err(Error::Refused(format!("unknown deny-list operation: {op}"))),
    })
}

pub(super) fn dispatch(op: &str, args: &Value) -> Result<Value, String> {
    if op == "denylist.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing deny-list job")?,
            Some(args["reply"].clone()),
        );
    }
    let flags = &args["transport"];
    if !flags
        .as_object()
        .is_some_and(|m| m.values().any(|v| *v == true))
    {
        return Ok(response(execute(op, args, &mut denylist::NativeRuntime)));
    }
    let mut args = args.clone();
    for key in ["repo_root", "path"] {
        if let Some(path) = args[key].as_str().filter(|s| !s.is_empty()) {
            args[key] = json!(std::path::absolute(PathBuf::from(path)).map_err(|e| e.to_string())?);
        }
    }
    let op = op.to_string();
    let flags = args["transport"].clone();
    job::start("denylist", move |channel| {
        let mut runtime = Transport { flags, channel };
        response(execute(&op, &args, &mut runtime))
    })
}
