use super::{Error, Result, Runtime, field, text, words};
use crate::{
    plugin::{context::LiveContext, denylist, fold},
    value::{json_pretty, truthy},
};
use serde_json::{Map, Value, json};
use std::{fs, path::PathBuf, time::UNIX_EPOCH};

fn joined(value: &Value, empty: &str) -> Result<String> {
    let joined = words(value)?.join(", ");
    Ok(if joined.is_empty() {
        empty.into()
    } else {
        joined
    })
}
fn count(value: &Value) -> Result<i64> {
    // shortcut: displayed tool counters use i64, extend their range in the numeric parity audit.
    value
        .as_i64()
        .or_else(|| value.as_bool().map(i64::from))
        .ok_or_else(|| "tool counter must be an integer".into())
}
fn object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| "tool state must be an object".into())
}
fn items(value: &Value) -> Result<Vec<String>> {
    if truthy(value) {
        words(value)
    } else {
        Ok(Vec::new())
    }
}

pub fn toolprune_show(ctx: &LiveContext, unt: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let host = &ctx.host;
    object(unt)?;
    let zero = json!(0);
    runtime.say(&format!(
        "sessions observed: {} ({} prompts)  catalog: {}",
        text(field(unt, "sessions", &zero)),
        text(field(unt, "prompts", &zero)),
        text(
            unt.get("catalog_source")
                .filter(|v| truthy(v))
                .unwrap_or(&json!("none yet"))
        )
    ))?;
    let applied = runtime.call(ctx, "applied", json!([]))?;
    runtime.say(&format!(
        "applied ({} this plugin added to {}): {}",
        denylist::unit(host, true),
        denylist::settings_path(&ctx.repo.root, host).display(),
        joined(&applied, "-")?
    ))?;
    let proposal = runtime.call(ctx, "proposal", json!([unt]))?;
    runtime.say(&format!("proposed: {}", joined(&proposal, "-")?))?;
    if truthy(&unt["imported_sessions"]) {
        runtime.say(&format!(
            "of which {} from before the plugin ({} prompts), read from the agent's history",
            text(&unt["imported_sessions"]),
            text(field(unt, "imported_prompts", &zero))
        ))?;
    }
    let mut waiting = Vec::new();
    for tool in items(&unt["needs_apply"])? {
        if words(&runtime.call(ctx, "proposal", json!([unt]))?)?.contains(&tool) {
            waiting.push(tool);
        }
    }
    if !waiting.is_empty() && ctx.config["toolprune"] == "auto" {
        runtime.say(&format!("waiting for `toolprune apply`: {} (the evidence for them includes sessions from before the plugin, so `auto` does not write them)",waiting.join(", ")))?;
    }
    let mut pending = Vec::new();
    let held = unt.get("auto_pending").filter(|v| truthy(v));
    if let Some(held) = held {
        object(held)?;
        for tool in items(&held["items"])? {
            if words(&runtime.call(ctx, "proposal", json!([unt]))?)?.contains(&tool) {
                pending.push(tool);
            }
        }
    }
    if !pending.is_empty() && ctx.config["toolprune"] == "auto" {
        runtime.say(&format!("pending: written once this repository's sessions have been idle {} minutes, so that no conversation you resume re-reads its cached prompt (`toolprune apply` writes it now)",text(&ctx.config["toolprune_quiet_minutes"])))?;
    }
    let kept = runtime.call(ctx, "kept", json!([]))?;
    if truthy(&kept) {
        runtime.say(&format!(
            "kept, as you gave them back (`toolprune apply NAME` removes one again): {}",
            joined(&kept, "")?
        ))?;
    }
    if host == "codex" {
        if let Some(caps) = unt.get("capabilities").filter(|v| truthy(v)) {
            runtime.say("capabilities (removable from a project's .codex/config.toml):")?;
            let mut rows: Vec<_> = object(caps)?.iter().collect();
            rows.sort_by(|a, b| a.0.cmp(b.0));
            for (name, evidence) in rows {
                let loaded = count(&evidence["loaded_sessions"])?;
                let prompts = count(field(
                    evidence,
                    "loaded_prompts",
                    &evidence["loaded_sessions"],
                ))?;
                let unused = count(&evidence["unused_sessions"])?;
                let used = truthy(
                    evidence
                        .get("used")
                        .ok_or_else(|| Error::new("KeyError", "used"))?,
                );
                runtime.say(&format!("  {name:14} loaded in {loaded:3} sessions ({prompts:3} prompts), unused in {unused:3}{}",if used {"  USED (kept)"} else {""}))?;
            }
        }
        if runtime.call(ctx, "codex_trusted", json!([]))? == false {
            runtime.say("note: Codex reads a project's .codex/config.toml only in a TRUSTED project; trust this one in Codex first, or an applied proposal does nothing.")?;
        }
        return Ok(0);
    }
    if host == "copilot"
        && let Some(servers) = unt.get("servers").filter(|v| truthy(v))
    {
        runtime.say("MCP servers (Copilot can only remove a whole server):")?;
        let mut rows: Vec<_> = object(servers)?.iter().collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        for (name, evidence) in rows {
            let state = if truthy(
                evidence
                    .get("used")
                    .ok_or_else(|| Error::new("KeyError", "used"))?,
            ) {
                "USED (kept)".into()
            } else {
                format!(
                    "unused for {} sessions ({} prompts)",
                    text(field(evidence, "unused_sessions", &zero)),
                    text(field(evidence, "unused_prompts", &zero))
                )
            };
            let tools = count(&evidence["tools"])?;
            runtime.say(&format!("  {name:24} {tools:3} tools, {state}"))?;
        }
    }
    if truthy(&unt["streaks"]) && host == "claude" {
        runtime.say(&format!("how long each tool has gone unused (a tool never used is left out after {} prompts over {}+ sessions, or after {} sessions):",text(&ctx.config["toolprune_evidence"]),text(&ctx.config["toolprune_min_sessions"]),text(&ctx.config["toolprune_sessions"])))?;
        let empty = json!({});
        let prompts = unt
            .get("prompt_streaks")
            .filter(|v| truthy(v))
            .unwrap_or(&empty);
        object(prompts)?;
        let used = words(field(unt, "used_tools", &json!([])))?;
        let mut rows = object(&unt["streaks"])?
            .iter()
            .map(|(tool, n)| -> Result<_> {
                Ok((
                    tool,
                    count(n)?,
                    count(field(prompts, tool, &zero))?,
                    count(field(prompts, tool, n))?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));
        for (tool, sessions, _, prompts) in rows.into_iter().take(30) {
            runtime.say(&format!(
                "  {sessions:4} sessions {prompts:5} prompts  {tool} {}",
                if used.contains(tool) { "used" } else { "" }
            ))?;
        }
    }
    if !truthy(&unt["catalog_source"]) || unt["catalog_source"] == "observed" {
        if ctx
            .root
            .join("hosts")
            .join(host)
            .join("catalog.json")
            .exists()
        {
            runtime.say("note: the catalog `toolprune refresh-catalog` recorded joins this repository's evidence from its next session.")?;
        } else {
            runtime.say(&format!("note: without `te --host {host} toolprune refresh-catalog` only {} tools are known, which leaves the always-loaded tools out of the evidence.",if host == "claude" {"deferred and observed"} else {"observed"}))?;
        }
    }
    Ok(0)
}

struct Heartbeat(PathBuf);
impl Drop for Heartbeat {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub fn wait_and_apply(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let mut ctx = runtime.context(args)?;
    let beat = ctx.store.base.join("toolprune-waiter");
    let now = runtime.global("time", json!([]));
    let now = match now {
        Ok(now) => Some(now.as_f64().ok_or("clock must return a number")?),
        Err(error) if error.is("OSError") => None,
        Err(error) => return Err(error),
    };
    if let Some(now) = now
        && let Ok(modified) = fs::metadata(&beat).and_then(|m| m.modified())
    {
        let modified = modified
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or_else(|e| -e.duration().as_secs_f64());
        if now - modified < 2.0 * fold::WAITER_POLL_S {
            return Ok(0);
        }
    }
    ctx.store.ensure(&[] as &[&str])?;
    let pid = runtime.global("pid", json!([]))?;
    fs::write(&beat, text(&pid))?;
    let _heartbeat = Heartbeat(beat.clone());
    loop {
        let wait = runtime.call(&ctx, "apply_pending", json!([]))?;
        if wait.is_null() {
            return Ok(0);
        }
        let wait = wait.as_f64().ok_or("pending wait must be a number")?;
        let wait = wait.clamp(1.0, fold::WAITER_POLL_S);
        runtime.global("sleep", json!([wait]))?;
        runtime.global("touch", json!([beat]))?;
        ctx = runtime.context(args)?;
    }
}

pub fn toolprune(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let ctx = runtime.context(args)?;
    let host = &ctx.host;
    if !denylist::supports(host) {
        runtime.say(&format!(
            "tool pruning is not available on {host} (it declares only a handful of tools)"
        ))?;
        return Ok(1);
    }
    let unt = ctx.store.read("untool.json", json!({}))?;
    let unt = if truthy(&unt) { unt } else { json!({}) };
    let action = args["action"]
        .as_str()
        .ok_or("tool-pruning action must be text")?;
    if action == "show" {
        return toolprune_show(&ctx, &unt, runtime);
    }
    if ["apply", "restore"].contains(&action)
        && truthy(&runtime.call(&ctx, "refuse_unwritable", json!([ctx.store.base]))?)
    {
        return Ok(1);
    }
    let tools = || items(&args["tools"]);
    match action {
        "apply" => {
            let mut tools = tools()?;
            if tools.is_empty() {
                tools = words(&runtime.call(&ctx, "proposal", json!([unt]))?)?;
            }
            if tools.is_empty() {
                runtime.say("nothing to apply")?;
                return Ok(0);
            }
            if host == "codex" {
                let bad: Vec<_> = tools
                    .iter()
                    .filter(|tool| !denylist::CODEX_LIVE_CAPABILITIES.contains(&tool.as_str()))
                    .cloned()
                    .collect();
                if !bad.is_empty() {
                    runtime.say(&format!(
                        "not a removable Codex capability: {} (choose from {})",
                        bad.join(", "),
                        denylist::CODEX_LIVE_CAPABILITIES.join(", ")
                    ))?;
                    return Ok(2);
                }
            }
            let added = match runtime.call(&ctx, "deny_apply", json!([tools])) {
                Ok(added) => added,
                Err(error) if error.is("RuntimeError") => {
                    runtime.say(&format!("not applied: {error}"))?;
                    return Ok(1);
                }
                Err(error) => return Err(error),
            };
            runtime.call(&ctx, "set_kept", json!([[], tools]))?;
            runtime.say(&format!(
                "added: {}\nin {} -- takes effect from the next session.",
                joined(&added, "(already present)")?,
                denylist::settings_path(&ctx.repo.root, host).display()
            ))?;
            if host == "copilot" && truthy(&added) {
                if runtime.call(&ctx, "copilot_trusted", json!([]))? == false {
                    runtime.say("warning: Copilot reads a repository's .github/copilot settings only in a trusted folder, and this one is not. Trust it (Copilot asks when you start it here), or the change does nothing.")?;
                }
                let applied = runtime.call(&ctx, "applied", json!([]))?;
                let left = runtime.call(&ctx, "verify_copilot", json!([applied]))?;
                if !left.is_null() {
                    let mut entries: Vec<_> = object(&left)?.iter().collect();
                    entries.sort_by(|a, b| a.0.cmp(b.0));
                    runtime.say(&format!(
                        "checked with a local capture: {}",
                        entries
                            .into_iter()
                            .map(|(k, v)| format!(
                                "{k} {}",
                                if truthy(v) {
                                    format!("still loaded ({} tools)", text(v))
                                } else {
                                    "removed".into()
                                }
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))?;
                }
            }
            if host == "codex" && truthy(&added) {
                if runtime.call(&ctx, "codex_trusted", json!([]))? == false {
                    runtime.say("warning: this project is not trusted in Codex, which ignores a project's .codex/config.toml until it is. Trust it (Codex asks when you start it here), or the change does nothing.")?;
                }
                let applied = runtime.call(&ctx, "applied", json!([]))?;
                let state = runtime.call(&ctx, "verify_codex", json!([applied]))?;
                if truthy(&state) {
                    let mut entries: Vec<_> = object(&state)?.iter().collect();
                    entries.sort_by(|a, b| a.0.cmp(b.0));
                    runtime.say(&format!(
                        "codex features list: {}",
                        entries
                            .into_iter()
                            .map(|(k, v)| format!("{k}={}", if truthy(v) { "on" } else { "off" }))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))?;
                }
            }
            Ok(0)
        }
        "restore" => {
            let tools = tools()?;
            let removed = match runtime.call(
                &ctx,
                "deny_restore",
                json!([if tools.is_empty() {
                    Value::Null
                } else {
                    json!(tools)
                }]),
            ) {
                Ok(removed) => removed,
                Err(error) if error.is("RuntimeError") => {
                    runtime.say(&format!("not restored: {error}"))?;
                    return Ok(1);
                }
                Err(error) => return Err(error),
            };
            runtime.call(&ctx, "set_kept", json!([removed, []]))?;
            let kept = if truthy(&removed) {
                format!(
                    "; kept from now on (`toolprune apply {}` removes one again)",
                    words(&removed)?.first().ok_or("missing restored item")?
                )
            } else {
                String::new()
            };
            runtime.say(&format!(
                "removed: {} (from the next session){kept}",
                joined(&removed, "-")?
            ))?;
            Ok(0)
        }
        "apply-pending" => wait_and_apply(args, runtime),
        "refresh-catalog" => {
            if host == "codex" {
                runtime.say(&format!("Codex needs no catalog: its removable capabilities are a fixed set ({}), read from each session.",denylist::CODEX_LIVE_CAPABILITIES.join(", ")))?;
                return Ok(0);
            }
            let refreshed = runtime.call(&ctx, "refresh_catalog", json!([]))?;
            let refreshed = refreshed
                .as_array()
                .filter(|v| v.len() == 2)
                .ok_or("catalog refresh must return path and tools")?;
            let tools = words(&refreshed[1])?;
            runtime.say(&format!(
                "{} tools recorded in {}:\n  {}",
                tools.len(),
                text(&refreshed[0]),
                tools.join(", ")
            ))?;
            Ok(0)
        }
        "measure" => {
            let mut tools = tools()?;
            if tools.is_empty() {
                tools = words(&runtime.call(&ctx, "proposal", json!([unt]))?)?;
            }
            if tools.is_empty() {
                tools = words(&runtime.call(&ctx, "applied", json!([]))?)?;
            }
            if tools.is_empty() {
                runtime.say("no proposal to measure")?;
                return Ok(1);
            }
            let measured = match runtime.call(&ctx, "measure", json!([tools])) {
                Ok(measured) => measured,
                Err(error) if error.is("RuntimeError") => {
                    runtime.say(&format!("not measured: {error}"))?;
                    return Ok(1);
                }
                Err(error) => return Err(error),
            };
            runtime.say(&json_pretty(&measured, false))?;
            Ok(0)
        }
        _ => Ok(2),
    }
}
