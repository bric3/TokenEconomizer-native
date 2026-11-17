mod status;
pub use status::{status, status_explanations};
mod diagnostics;
mod entry;
pub use entry::main;
mod learning;
pub mod parser;
mod repositories;
mod tools;
pub use diagnostics::{doctor, netloc, recent_host, rtk_hooks};
pub use learning::{
    facts, fold_command, forget_facts, history_command, judge_command, map_command, print_history,
    session_context, short, when,
};
pub use repositories::{print_moves, repositories};
pub use tools::{toolprune, toolprune_show, wait_and_apply};

use super::{config, context::LiveContext, denylist, fold, hosts, paths, repos};
use crate::{
    envelope,
    value::{json_pretty, json_text, python_repr, scalar_text, truthy},
};
use serde_json::{Value, json};
use std::{fmt, fs, io, path::Path};

#[derive(Debug)]
pub struct Error {
    pub kind: String,
    pub message: String,
    pub parents: Vec<String>,
}
impl Error {
    pub fn new(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            parents: Vec::new(),
        }
    }
    pub fn is(&self, kind: &str) -> bool {
        self.kind == kind || self.parents.iter().any(|parent| parent == kind)
    }
}
impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::new("RuntimeError", message)
    }
}
impl From<&str> for Error {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self {
            kind: match error.kind() {
                io::ErrorKind::NotFound => "FileNotFoundError",
                io::ErrorKind::PermissionDenied => "PermissionError",
                io::ErrorKind::InvalidData => "ValueError",
                io::ErrorKind::IsADirectory => "IsADirectoryError",
                io::ErrorKind::NotADirectory => "NotADirectoryError",
                _ => "OSError",
            }
            .into(),
            message: error.to_string(),
            parents: if error.kind() == io::ErrorKind::InvalidData {
                Vec::new()
            } else {
                vec!["OSError".into()]
            },
        }
    }
}
impl From<denylist::Error> for Error {
    fn from(error: denylist::Error) -> Self {
        match error {
            denylist::Error::Io(error) => error.into(),
            denylist::Error::Refused(error) => error.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

pub trait Runtime {
    fn context(&mut self, args: &Value) -> Result<LiveContext>;
    fn pinned_context(&mut self, args: &Value, session: &Value) -> Result<LiveContext>;
    fn global(&mut self, method: &str, args: Value) -> Result<Value>;
    fn call(&mut self, ctx: &LiveContext, method: &str, args: Value) -> Result<Value>;
    fn say(&mut self, text: &str) -> Result<()>;
}

fn whitespace(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
fn text(value: &Value) -> String {
    scalar_text(value)
}
fn field<'a>(value: &'a Value, key: &str, default: &'a Value) -> &'a Value {
    value.get(key).unwrap_or(default)
}
pub fn words(value: &Value) -> Result<Vec<String>> {
    value
        .as_array()
        .ok_or_else(|| Error::from("expected a list"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(String::from)
                .ok_or_else(|| Error::from("expected text"))
        })
        .collect()
}

pub fn session_note(ctx: &LiveContext, env: &Value) -> Result<Value> {
    let sid = hosts::env_session_id(&ctx.host, env);
    let pin = if sid.is_empty() {
        None
    } else {
        repos::pinned(&ctx.root, &ctx.host, &json!(sid))?
    };
    Ok(match pin.filter(|repo| repo.key != ctx.repo.key) {
        Some(pin) => json!(format!(
            "this {} session started in {} and is recorded under {} (`te --cwd {} status`)",
            ctx.host,
            pin.root.display(),
            pin.label,
            pin.root.display()
        )),
        None => Value::Null,
    })
}

pub fn proposal(ctx: &LiveContext, unt: &Value) -> Result<Vec<String>> {
    let mut mine = denylist::applied_for(&ctx.store, &ctx.repo.root, &ctx.host)?;
    mine.extend(denylist::kept_for(&ctx.store, &ctx.repo.root, &ctx.host)?);
    let items = words(field(unt, "proposal", &json!([])))?;
    Ok(fold::unprotected(
        ctx,
        &items
            .into_iter()
            .filter(|item| !mine.contains(item))
            .collect::<Vec<_>>(),
    ))
}

pub fn split_argline(line: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut started = false;
    for c in line.chars() {
        if quote == Some(c) {
            quote = None;
        } else if quote.is_some() {
            token.push(c);
        } else if matches!(c, '\'' | '"') {
            quote = Some(c);
            started = true;
        } else if matches!(c, ' ' | '\t' | '\r' | '\n') {
            if started {
                args.push(std::mem::take(&mut token));
                started = false;
            }
        } else {
            token.push(c);
            started = true;
        }
    }
    if quote.is_some() {
        return Err(Error::new("ValueError", "No closing quotation"));
    }
    if started {
        args.push(token);
    }
    Ok(args)
}
pub fn expand_argline(argv: &[String]) -> Result<Vec<String>> {
    for (i, arg) in argv.iter().enumerate() {
        let (line, consumed) = if arg == "--argline" {
            let Some(line) = argv.get(i + 1) else {
                continue;
            };
            (line.as_str(), 2)
        } else if let Some(line) = arg.strip_prefix("--argline=") {
            (line, 1)
        } else {
            continue;
        };
        let mut out = argv[..i].to_vec();
        out.extend(split_argline(line)?);
        out.extend_from_slice(&argv[i + consumed..]);
        return Ok(out);
    }
    Ok(argv.to_vec())
}

pub fn unwritable(path: &Path, pid: u32) -> Option<String> {
    let probe = path.join(format!(".write-test-{pid}"));
    (|| -> io::Result<()> {
        fs::create_dir_all(path)?;
        fs::File::create(&probe)?;
        fs::remove_file(probe)
    })()
    .err()
    .map(|error| {
        let message = error.to_string();
        message
            .split(" (os error ")
            .next()
            .unwrap_or(&message)
            .to_string()
    })
}
pub fn refusal(path: &Path, why: &str, argv: &[String]) -> String {
    let args = if argv.is_empty() {
        "...".into()
    } else {
        argv.join(" ")
    };
    format!(
        "not changed: TokenEconomizer cannot write its state in {} ({why}). Inside a sandboxed agent, run this command with permission to write outside the workspace (Codex asks you to approve it), or run it from a terminal as `~/.tokeneconomizer/bin/te {args}`.",
        path.display()
    )
}

pub fn config_view(cfg: &Value) -> Result<Value> {
    let mut d = cfg.as_object().cloned().ok_or("expected configuration")?;
    let warnings = d.shift_remove("warnings").unwrap_or(json!([]));
    let pinned = d
        .shift_remove("loopguard_thresholds")
        .ok_or("missing guard thresholds")?;
    let mut view = json!({"enabled":d.shift_remove("enabled").ok_or("missing enabled setting")?});
    for (name, setting, keys) in [
        (
            "microskills",
            "microskills",
            &[
                "microskills",
                "microskills_max_facts",
                "microskills_max_tokens",
                "microskills_propose_every",
            ][..],
        ),
        (
            "toolprune",
            "toolprune",
            &[
                "toolprune",
                "toolprune_evidence",
                "toolprune_min_sessions",
                "toolprune_sessions",
                "toolprune_quiet_minutes",
                "toolprune_protect",
            ][..],
        ),
        (
            "loopguard",
            "loopguard",
            &["loopguard", "loopguard_aggr", "loopguard_nudges"][..],
        ),
        (
            "history",
            "history_import",
            &[
                "history_import",
                "history_max_sessions",
                "history_days",
                "history_dirs",
            ][..],
        ),
        ("droid", "droid_sidecall", &["droid_sidecall"][..]),
    ] {
        let on = if matches!(name, "history" | "droid") {
            d.get(setting).is_some_and(truthy)
        } else {
            d.get(setting).is_some_and(|v| v != "off")
        };
        let mut section = json!({"state":if on {"on"} else {"off"}});
        for key in keys {
            section[*key] = d
                .shift_remove(*key)
                .ok_or_else(|| format!("missing setting: {key}"))?;
        }
        if name == "loopguard" {
            section["thresholds"] = envelope::base_thresholds(
                section["loopguard_aggr"].as_str().unwrap_or("default"),
                &pinned,
            );
            let mut keys: Vec<_> = pinned
                .as_object()
                .ok_or("expected pinned thresholds")?
                .keys()
                .cloned()
                .collect();
            keys.sort();
            section["pinned"] = json!(keys);
        }
        view[name] = section;
    }
    view["other"] = json!(d);
    view["warnings"] = warnings;
    Ok(view)
}

pub fn config_command(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let ctx = runtime.context(args)?;
    if args["action"] == "show" {
        runtime.say(&json_pretty(&config_view(&ctx.config)?, false))?;
        return Ok(0);
    }
    let dir = if truthy(&args["globally"]) {
        &ctx.root
    } else {
        ctx.store.base.parent().ok_or("state has no parent")?
    };
    let path = dir.join("config.json");
    if truthy(&runtime.call(&ctx, "refuse_unwritable", json!([dir]))?) {
        return Ok(1);
    }
    let mut updates = serde_json::Map::new();
    let pairs = words(&args["pairs"])?;
    let setting = if args["action"] == "unset" {
        runtime.call(
            &ctx,
            "unset_config",
            json!([
                path,
                pairs
                    .iter()
                    .map(|s| s.trim_matches(whitespace))
                    .collect::<Vec<_>>()
            ]),
        )
    } else {
        for kv in pairs {
            let Some((key, value)) = kv.split_once('=') else {
                runtime.say(&format!(
                    "expected KEY=VALUE, got {}",
                    python_repr(&json!(kv))
                ))?;
                return Ok(2);
            };
            updates.insert(
                key.trim_matches(whitespace).into(),
                json!(value.trim_matches(whitespace)),
            );
        }
        if updates.is_empty() {
            runtime.say("nothing to set: give KEY=VALUE (see `te config show` for the keys)")?;
            return Ok(2);
        }
        runtime.call(&ctx, "write_config", json!([path, updates]))
    };
    let cur = match setting {
        Ok(value) => value,
        Err(error) if error.is("ValueError") => {
            runtime.say(&format!("rejected: {}", error.message))?;
            return Ok(2);
        }
        Err(error) => return Err(error),
    };
    let mut sorted = cur.clone();
    sorted.sort_all_objects();
    runtime.say(&format!("{}: {}", path.display(), json_text(&sorted)))?;
    if args["action"] == "unset" {
        return Ok(0);
    }
    runtime.say("applies from the next hook call; a new fact-size cap applies once the next session has been folded")?;
    if updates.keys().any(|key| {
        matches!(
            config::canonical(key).as_str(),
            "microskills" | "loopguard" | "droid_sidecall"
        )
    }) && truthy(&runtime.call(&ctx, "resume", json!([]))?)
    {
        let name = runtime.call(&ctx, "call_name", json!([]))?;
        runtime.say(&format!(
            "the plugin's model calls through `{}` will be tried again",
            text(&name)
        ))?;
    }
    if text(field(&cur, "microskills", &json!(""))).contains("distiller") {
        let every = cur
            .get("microskills_propose_every")
            .filter(|v| truthy(v))
            .unwrap_or(&ctx.config["microskills_propose_every"]);
        runtime.say(&format!("note: micro-skills calls a model through this agent every {} sessions to propose facts, charged to the session's cost; `microskills=envfacts` learns without model calls", text(every)))?;
    }
    if cur["loopguard"] == "stop" {
        runtime.say("note: `stop` acts as `nudge`: the plugin never stops a turn; a turn still stuck after the redirect gets a notice saying how to stop it")?;
    }
    Ok(0)
}
