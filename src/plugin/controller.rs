use super::{
    context::LiveContext,
    denylist, history,
    hookqueue::{self, Hook},
    hosts::{
        self,
        session::{self, Environment},
    },
    judge, repos,
};
use crate::{
    envelope,
    loopguard::turn::{FoldOptions, Judge, TurnGuard},
    value::{float_or_zero, int_value, scalar_text, truthy},
};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use std::{io, path::Path};

pub const TURN_HISTORY: usize = 200;
type Result<T> = std::result::Result<T, String>;

pub trait Runtime: judge::Runtime + session::Runtime {
    fn dispatch(&mut self, controller: &mut LiveController, method: &str) -> Result<Value>;
    fn queue_limits(&self, event: &str) -> hookqueue::Limits {
        hookqueue::Limits::for_event(event)
    }
    fn enqueue(&mut self, queue: &Path, payload: &Value) -> bool {
        hookqueue::enqueue(queue, payload)
    }
    fn launcher(&mut self, ctx: &LiveContext) -> io::Result<()>;
    fn paused_detail(&mut self) -> Result<Value>;
    fn call_name(&mut self) -> Result<String>;
    fn local_routes(&mut self) -> Result<bool>;
    fn map(&mut self, ctx: &LiveContext, method: &str, args: Value) -> Result<Value>;
    fn history_notice(&mut self, ctx: &LiveContext) -> Result<Value>;
    fn history_start(&mut self, ctx: &LiveContext) -> Result<()>;
    fn migration_start(&mut self, ctx: &LiveContext) -> Result<()>;
    fn pending_sessions(&mut self, ctx: &LiveContext, exclude: &[String]) -> Result<Vec<String>>;
    fn spawn_folds(&mut self, ctx: &LiveContext, sessions: &[String]) -> Result<()>;
}

#[derive(Clone)]
pub struct LiveController {
    pub ctx: LiveContext,
    pub p: Value,
    pub sid: String,
    pub replaying: bool,
    pub environment: Environment,
    pub parent_pid: u32,
    pub windows: bool,
}
pub fn handler(event: &str) -> Option<&'static str> {
    match event {
        "SessionStart" => Some("session_start"),
        "UserPromptSubmit" => Some("user_prompt"),
        "PostToolUse" | "PostToolUseFailure" => Some("post_tool"),
        "Stop" => Some("stop"),
        "SessionEnd" => Some("session_end"),
        _ => None,
    }
}
pub fn command(host: &str, name: &str) -> String {
    match host {
        "claude" | "copilot" => format!("/tokeneconomizer:{name}"),
        "pi" => format!("/te {name}"),
        "codex" => format!("$tokeneconomizer:{name}"),
        _ => format!("te --host {host} {name}"),
    }
}
fn count(doc: &Value, key: &str) -> Result<i64> {
    let Some(value) = doc.get(key) else {
        return Ok(0);
    };
    if let Some(n) = value.as_i64() {
        return Ok(n);
    }
    if let Some(n) = value.as_str().and_then(|s| s.trim().parse::<i64>().ok()) {
        return Ok(n);
    }
    // shortcut: session counters use i64, extend their range during the numeric parity audit.
    int_value(value)
        .filter(|n| *n >= i64::MIN as f64 && *n < -(i64::MIN as f64))
        .map(|n| n as i64)
        .ok_or_else(|| format!("invalid session counter: {value}"))
}
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}
fn read(ctx: &LiveContext, name: &str) -> Result<Value> {
    let value = ctx
        .store
        .read(name, Value::Null)
        .map_err(|e| e.to_string())?;
    Ok(if truthy(&value) { value } else { json!({}) })
}

#[derive(Clone, Copy)]
pub enum JudgeMode {
    None,
    NoVerdict,
    Async,
}
pub fn judge_mode(host: &str, envelope: &Value, runtime: &mut dyn Runtime) -> Result<JudgeMode> {
    Ok(if envelope.is_null() {
        JudgeMode::None
    } else if host == "antigravity"
        || judge::Runtime::paused(runtime)? && !runtime.local_routes()?
    {
        JudgeMode::NoVerdict
    } else {
        JudgeMode::Async
    })
}

impl LiveController {
    pub fn new(
        ctx: LiveContext,
        payload: Value,
        environment: Environment,
        parent_pid: u32,
        windows: bool,
    ) -> Self {
        let p = if truthy(&payload) { payload } else { json!({}) };
        let sid = hosts::session_id(&ctx.host, &p);
        Self {
            ctx,
            p,
            sid,
            replaying: false,
            environment,
            parent_pid,
            windows,
        }
    }
    fn setting(&self, key: &str) -> &str {
        self.ctx.config[key].as_str().unwrap_or("")
    }
    fn shadow(&self) -> bool {
        truthy(&self.ctx.config["shadow"])
    }
    pub fn sess_name(&self) -> String {
        format!("sessions/{}.json", self.sid)
    }
    pub fn session(&self) -> Result<Value> {
        let saved = self
            .ctx
            .store
            .read(&self.sess_name(), Value::Null)
            .map_err(|e| e.to_string())?;
        if saved.is_object() {
            return Ok(saved);
        }
        let mut s = json!({"session_id":self.sid,"host":self.ctx.host,"started":history::now(),"cwd":session::cwd(&self.ctx.host,&self.p,&json!(self.ctx.env),&self.environment.cwd),"turns":[],"nudges":0,"aborts":0,"judge_tokens":0.0,"writer_tokens":0.0});
        s.as_object_mut().unwrap().extend(
            session::session_state(&self.ctx.host, &self.ctx.env, &self.environment.home)
                .as_object()
                .unwrap()
                .clone(),
        );
        Ok(s)
    }
    pub fn save(&self, s: &mut Value) -> Result<()> {
        let tp = session::transcript(&self.ctx.host, &self.p, &self.environment)
            .map_err(|e| e.to_string())?;
        if truthy(&tp) {
            s["transcript"] = tp;
        }
        s["updated"] = json!(history::now());
        self.ctx
            .store
            .write(&self.sess_name(), s)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    pub fn cmd(&self, name: &str) -> String {
        command(&self.ctx.host, name)
    }
    pub fn log(&self, kind: &str, mut fields: Value) -> Result<()> {
        fields.as_object_mut().ok_or("event fields must be an object")?.extend(json!({"kind":kind,"at":history::now(),"session_id":self.sid,"shadow":self.shadow()}).as_object().unwrap().clone());
        self.ctx
            .store
            .append("events.jsonl", &fields)
            .map_err(|e| e.to_string())
    }
    pub fn record_error(&self, error: &str) {
        repos::record_error(&self.ctx.root, error);
    }
    pub fn handle(&mut self, runtime: &mut dyn Runtime) -> Result<Value> {
        if !truthy(&self.ctx.config["enabled"])
            || hosts::session_id(&self.ctx.host, &self.p).is_empty()
        {
            return Ok(json!({}));
        }
        self.ctx.settle()?;
        let event = self.p["hook_event_name"].as_str().unwrap_or("").to_string();
        if ["SessionStart", "UserPromptSubmit"].contains(&event.as_str())
            && let Err(error) = runtime.launcher(&self.ctx)
        {
            self.record_error(&error.to_string());
        }
        if handler(&event).is_none() {
            return Ok(json!({}));
        }
        run_queue(self, &event, runtime)
    }
    pub fn invoke(&mut self, method: &str, runtime: &mut dyn Runtime) -> Result<Value> {
        match method {
            "session_start" => self.session_start(runtime),
            "user_prompt" => self.user_prompt(runtime),
            "post_tool" => self.post_tool(runtime),
            "stop" => self.stop(),
            "session_end" => self.session_end(runtime),
            _ => Err(format!("unknown controller handler: {method}")),
        }
    }
    pub fn session_start(&mut self, runtime: &mut dyn Runtime) -> Result<Value> {
        let source = self
            .p
            .get("source")
            .filter(|v| truthy(v))
            .and_then(Value::as_str)
            .unwrap_or("startup")
            .to_string();
        let mut s = self.session()?;
        s["source"] = json!(source);
        if s.get("cwd").is_none() {
            s["cwd"] = session::cwd(
                &self.ctx.host,
                &self.p,
                &json!(self.ctx.env),
                &self.environment.cwd,
            );
        }
        let mut outputs = Vec::new();
        let mut seen = read(&self.ctx, "notices.json")?;
        let before = seen.clone();
        let profile = hosts::profile(&self.ctx.host);
        if self.setting("microskills") != "off" {
            let inj = read(&self.ctx, "injection.json")?;
            let mut text = inj["rendered"].as_str().unwrap_or("").to_string();
            let fresh = ["startup", "new", "clear", "compact"].contains(&source.as_str());
            if !fresh && !profile.keeps_start_context {
                text = s
                    .get("injected_text")
                    .and_then(Value::as_str)
                    .unwrap_or(if truthy(&s["injected_chars"]) {
                        &text
                    } else {
                        ""
                    })
                    .to_string();
            }
            if !text.is_empty() && (fresh || !profile.keeps_start_context) {
                let facts = text
                    .split([
                        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}',
                        '\u{2028}', '\u{2029}',
                    ])
                    .filter(|line| line.starts_with("- "))
                    .count();
                let chars = text.chars().count();
                self.log(
                    "inject",
                    json!({"chars":chars,"facts":facts,"source":source}),
                )?;
                if !self.shadow() {
                    outputs.push(hosts::context(&self.ctx.host, "SessionStart", &text));
                    s["injected_chars"] = json!(chars);
                    s["injected_text"] = json!(text);
                    if let Some(ids) = inj.get("rendered_ids") {
                        s["injected_ids"] = ids.clone();
                    }
                    let digest = format!("{:x}", Sha1::digest(text.as_bytes()));
                    if seen["injection"] != digest {
                        outputs.push(hosts::notice(&self.ctx.host,&format!("TokenEconomizer: added {facts} learned fact(s) about {} to this session's context (review with `{}`).",self.ctx.repo.label,self.cmd("microskills"))));
                        seen["injection"] = json!(digest);
                    }
                }
            }
        }
        if self.setting("toolprune") != "off"
            && denylist::supports(&self.ctx.host)
            && !self.shadow()
        {
            outputs.extend(self.toolprune_notices(&mut seen)?);
        }
        let pause = runtime.paused_detail()?;
        let calls_model =
            self.setting("microskills").contains("distiller") || self.setting("loopguard") != "off";
        if truthy(&pause) && calls_model && !self.shadow() && seen["sidecall_paused"] != pause["at"]
        {
            let byok = if self.ctx.host == "copilot" {
                " (with your own model key, Copilot does not pass it to the plugin's hooks)"
            } else {
                ""
            };
            let call = runtime.call_name()?;
            let command = self.cmd("config");
            outputs.push(hosts::notice(&self.ctx.host,&format!("TokenEconomizer: the plugin's own model calls go through `{call}`, which answered that it is not signed in{byok}, so they are paused; facts that need no model call are still learned, and the loop guard redirects only at its backstop. `{command} set microskills=on` (or `loopguard=on`) tries again.")));
            seen["sidecall_paused"] = pause["at"].clone();
        }
        if let Some(note) = repos::migration_notice(&self.ctx.root, &self.cmd("repos"))?
            && !note.is_empty()
            && !self.shadow()
        {
            outputs.push(hosts::notice(&self.ctx.host, &note));
        }
        if !self.shadow() {
            match runtime.history_notice(&self.ctx) {
                Ok(note) if truthy(&note) => outputs.push(hosts::notice(
                    &self.ctx.host,
                    note.as_str().ok_or("history notice must be text")?,
                )),
                Err(e) => self.record_error(&e),
                _ => {}
            }
        }
        if self.setting("repomap") != "off" {
            if truthy(&runtime.map(&self.ctx, "stale", json!([]))?) {
                runtime.map(&self.ctx, "spawn_refresh", json!([]))?;
            }
            if (self.setting("repomap") == "start" || !profile.prompt_context)
                && ["startup", "new", "clear", "compact"].contains(&source.as_str())
            {
                let block = runtime.map(&self.ctx, "cached_block", json!([]))?;
                outputs.extend(self.inject_map(
                    &mut s,
                    &mut seen,
                    block.as_str().ok_or("repository map must be text")?,
                    "SessionStart",
                    "cached",
                )?);
                s["repomap_done"] = json!(true);
            }
        }
        if seen != before {
            self.ctx
                .store
                .write("notices.json", &seen)
                .map_err(|e| e.to_string())?;
        }
        self.save(&mut s)?;
        repos::note_repo(&self.ctx.root, &self.ctx.repo)?;
        repos::prune_pins(&self.ctx.root, &self.ctx.host, None, repos::PIN_MAX_AGE_S)?;
        runtime.migration_start(&self.ctx)?;
        if let Err(e) = runtime.history_start(&self.ctx) {
            self.record_error(&e);
        }
        let pending = runtime.pending_sessions(&self.ctx, std::slice::from_ref(&self.sid))?;
        runtime.spawn_folds(&self.ctx, &pending)?;
        hosts::merge_outputs(&outputs)
    }
    pub fn toolprune_notices(&self, seen: &mut Value) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        let cmd = self.cmd("toolprune");
        let unit = denylist::unit(&self.ctx.host, false);
        let unt = read(&self.ctx, "untool.json")?;
        let mut mine = denylist::applied_for(&self.ctx.store, &self.ctx.repo.root, &self.ctx.host)
            .map_err(|e| e.to_string())?;
        mine.sort();
        let few = |items: &[String]| {
            format!(
                "{}{}",
                items.iter().take(4).cloned().collect::<Vec<_>>().join(", "),
                if items.len() > 4 { "..." } else { "" }
            )
        };
        if self.setting("toolprune") == "auto"
            && seen["untool_applied"].as_str().unwrap_or("") != mine.join(",")
        {
            if !mine.is_empty() {
                let trusted = match self.ctx.host.as_str() {
                    "codex" => denylist::codex_project_trusted(
                        &self.ctx.repo.root,
                        &self.environment.env,
                        &self.environment.home,
                    ),
                    "copilot" => denylist::copilot_folder_trusted(
                        &self.ctx.repo.root,
                        &self.environment.env,
                        &self.environment.home,
                    ),
                    _ => Ok(None),
                }
                .map_err(|e| e.to_string())?;
                let trust = if trusted == Some(false) {
                    " (The agent reads that setting only in a trusted folder: trust this one.)"
                } else {
                    ""
                };
                out.push(hosts::notice(&self.ctx.host,&format!("TokenEconomizer removed {} {unit}(s) this repository's sessions never used ({}), from this session on. Review with `{cmd}`; `{cmd} restore` gives them back.{trust}",mine.len(),few(&mine))));
            }
            seen["untool_applied"] = json!(mine.join(","));
        }
        let kept = denylist::kept_for(&self.ctx.store, &self.ctx.repo.root, &self.ctx.host)
            .map_err(|e| e.to_string())?;
        let mut prop: Vec<_> = strings(&unt["proposal"])
            .into_iter()
            .filter(|t| !mine.contains(t) && !kept.contains(t))
            .collect();
        let refused = if self.setting("toolprune") == "auto" {
            &unt["auto_apply_refused"]
        } else {
            &Value::Null
        };
        let mut history_only = false;
        if self.setting("toolprune") == "auto" && !truthy(refused) {
            let waiting = strings(&unt["needs_apply"]);
            prop.retain(|t| waiting.contains(t));
            history_only = !prop.is_empty();
        }
        if !prop.is_empty() && seen["untool"].as_str() != Some(&prop.join(",")) {
            let history = if history_only {
                ", counting sessions from before the plugin"
            } else {
                ""
            };
            let tail = if truthy(refused) {
                format!(
                    "they could not be applied ({}), and `{cmd} apply` tries again.",
                    scalar_text(refused)
                )
            } else {
                format!("nothing changes until you run `{cmd} apply`.")
            };
            out.push(hosts::notice(&self.ctx.host,&format!("TokenEconomizer: {} {unit}(s) went unused in this repository's recent sessions ({}){history}. Review with `{cmd}`; {tail}",prop.len(),few(&prop))));
            seen["untool"] = json!(prop.join(","));
        }
        Ok(out)
    }
    pub fn inject_map(
        &self,
        s: &mut Value,
        seen: &mut Value,
        block: &str,
        event: &str,
        how: &str,
    ) -> Result<Vec<Value>> {
        let chars = block.chars().count();
        self.log("repomap", json!({"chars":chars,"how":how}))?;
        if block.is_empty() || self.shadow() {
            return Ok(Vec::new());
        }
        s["repomap_chars"] = json!(chars);
        let mut out = vec![hosts::context(&self.ctx.host, event, block)];
        if !truthy(&seen["repomap"]) {
            out.push(hosts::notice(&self.ctx.host,&format!("TokenEconomizer: added a map of {}'s key files and symbols to this session's context (~{} tokens; `repomap=off` to stop).",self.ctx.repo.label,chars/4)));
            seen["repomap"] = json!(true);
        }
        Ok(out)
    }
    pub fn close_turn(&self, s: &mut Value, stopped_normally: bool) -> Result<Value> {
        let data = &s["turn"];
        if !truthy(data) {
            return Ok(Value::Null);
        }
        let g = TurnGuard::new("", data)?;
        if truthy(&g.held) {
            self.log(
                "undelivered",
                json!({"action":g.held[0],"reason":g.held[1]}),
            )?;
        }
        let summary = g.summary(stopped_normally);
        if g.cum_turns > 0 && stopped_normally {
            envelope::record_turn(
                &self.ctx.store,
                &g,
                self.setting("loopguard_aggr"),
                true,
                &self.ctx.config["loopguard_thresholds"],
            )
            .map_err(|e| e.to_string())?;
        }
        let mut turns = s["turns"].as_array().cloned().unwrap_or_default();
        if turns.len() >= TURN_HISTORY {
            turns.drain(..turns.len() - (TURN_HISTORY - 1));
        }
        turns.push(summary.clone());
        s["turns"] = json!(turns);
        s["judge_tokens"] = json!(float_or_zero(&s["judge_tokens"])? + g.judge_tokens);
        s["writer_tokens"] = json!(float_or_zero(&s["writer_tokens"])? + g.writer_tokens);
        s["turn"] = Value::Null;
        Ok(summary)
    }
    pub fn user_prompt(&self, runtime: &mut dyn Runtime) -> Result<Value> {
        let prompt_context = hosts::profile(&self.ctx.host).prompt_context;
        if self.setting("loopguard") == "off"
            && !(self.setting("repomap") == "on" && prompt_context)
        {
            return Ok(json!({}));
        }
        let mut s = self.session()?;
        let mut outputs = Vec::new();
        if self.setting("repomap") == "on" && prompt_context && !truthy(&s["repomap_done"]) {
            s["repomap_done"] = json!(true);
            let mut seen = read(&self.ctx, "notices.json")?;
            let mapped = runtime.map(
                &self.ctx,
                "for_prompt",
                json!([self
                    .p
                    .get("prompt")
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or(json!(""))]),
            )?;
            outputs = self.inject_map(
                &mut s,
                &mut seen,
                mapped[0].as_str().ok_or("repository map must be text")?,
                "UserPromptSubmit",
                mapped[1].as_str().ok_or("map source must be text")?,
            )?;
            if !outputs.is_empty() {
                self.ctx
                    .store
                    .write("notices.json", &seen)
                    .map_err(|e| e.to_string())?;
            }
        }
        if self.setting("loopguard") != "off" {
            self.close_turn(&mut s, false)?;
            s["turn"] =
                TurnGuard::new(&hosts::turn_key(&self.ctx.host, &self.p), &json!({}))?.to_dict();
        }
        self.save(&mut s)?;
        hosts::merge_outputs(&outputs)
    }
    pub fn stop(&self) -> Result<Value> {
        if self.setting("loopguard") == "off" || truthy(&self.p["stop_hook_active"]) {
            return Ok(json!({}));
        }
        let mut s = self.session()?;
        let summary = self.close_turn(&mut s, hosts::stopped_normally(&self.ctx.host, &self.p))?;
        if truthy(&summary) {
            self.log("turn", summary)?;
        }
        self.save(&mut s)?;
        Ok(json!({}))
    }
    pub fn post_tool(&self, runtime: &mut dyn Runtime) -> Result<Value> {
        if self.setting("loopguard") == "off" || hosts::is_subagent(&self.ctx.host, &self.p) {
            return Ok(json!({}));
        }
        let event = self.p["hook_event_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("PostToolUse");
        let mut s = self.session()?;
        let key = hosts::turn_key(&self.ctx.host, &self.p);
        let mut data = s["turn"].clone();
        if truthy(&data) && !key.is_empty() && truthy(&data["turn_key"]) && data["turn_key"] != key
        {
            self.close_turn(&mut s, false)?;
            data = Value::Null;
        }
        let mut g = TurnGuard::new(&key, &data)?;
        let (name, args, text, code) = hosts::tool_call(&self.ctx.host, &self.p)?;
        let tracker = envelope::load_envelope(
            &self.ctx.store,
            self.setting("loopguard_aggr"),
            &self.ctx.config["loopguard_thresholds"],
        )
        .map_err(|e| e.to_string())?;
        let env = tracker.envelope();
        let mut thr = envelope::thresholds(&tracker, &self.ctx.config["loopguard_thresholds"]);
        thr["gate_over_envelope"] = json!(false);
        thr["gate_min_turns"] = json!(
            thr["gate_min_turns"]
                .as_f64()
                .unwrap_or(0.0)
                .max(thr["min_turns"].as_f64().unwrap_or(0.0))
        );
        let mode = judge_mode(&self.ctx.host, &json!(env), runtime)?;
        let mut no_verdict = judge::NoVerdict;
        let mut asynchronous = if matches!(mode, JudgeMode::Async) {
            Some(judge::AsyncJudge::new(
                &self.ctx.store,
                &self.sid,
                &thr["judge_confidence"],
                runtime,
            )?)
        } else {
            None
        };
        let background = match mode {
            JudgeMode::None => None,
            JudgeMode::NoVerdict => Some(Judge::Background(&mut no_verdict)),
            JudgeMode::Async => Some(Judge::Background(asynchronous.as_mut().unwrap())),
        };
        let mut decision = g.fold(
            name.as_str().ok_or("tool name must be text")?,
            &args,
            &text,
            code.as_i64(),
            FoldOptions {
                envelope: env,
                thresholds: Some(&thr),
                judge: background,
                nudge_budget: self.ctx.config["loopguard_nudges"].as_u64().unwrap_or(1),
            },
        )?;
        if self.replaying {
            if !truthy(&g.held) {
                g.held = json!(decision);
            }
            decision = None;
        } else if truthy(&g.held) {
            decision = Some(serde_json::from_value(g.held.take()).map_err(|e| e.to_string())?);
        }
        let mut out = json!({});
        if let Some((action, reason)) = decision {
            let short = reason.split(':').next().unwrap_or("");
            if action == "nudge" {
                let body = g.nudge_body(env, None);
                g.after_nudge();
                s["nudges"] = json!(
                    count(&s, "nudges")?
                        .checked_add(1)
                        .ok_or("session counter exceeds i64")?
                );
                self.log(
                    "nudge",
                    json!({"reason":reason,"tool_calls":g.cum_turns,"envelope":env}),
                )?;
                if !self.shadow() {
                    out = hosts::merge_outputs(&[
                        hosts::context(&self.ctx.host, event, &body),
                        hosts::notice(
                            &self.ctx.host,
                            &format!(
                                "TokenEconomizer nudged this turn ({short}): the same failure kept recurring."
                            ),
                        ),
                    ])?;
                }
            } else if action == "abort" && !g.abort_delivered {
                g.abort_delivered = true;
                s["aborts"] = json!(
                    count(&s, "aborts")?
                        .checked_add(1)
                        .ok_or("session counter exceeds i64")?
                );
                self.log("abort",json!({"reason":reason,"tool_calls":g.cum_turns,"envelope":env,"delivered":"notice"}))?;
                if !self.shadow() {
                    let transcript =
                        session::transcript(&self.ctx.host, &self.p, &self.environment)
                            .map_err(|e| e.to_string())?;
                    let headless = match self.ctx.headless_override() {
                        Some(value) => value,
                        None => session::headless(
                            &self.ctx.host,
                            &json!(self.ctx.env),
                            transcript.as_str(),
                            &self.environment,
                            self.parent_pid,
                            self.windows,
                            runtime,
                        )
                        .map_err(|e| e.to_string())?,
                    };
                    let profile = hosts::profile(&self.ctx.host);
                    let hint =
                        hosts::stop_hint(&json!(headless), profile.interrupt, profile.end_run);
                    let text = format!(
                        "TokenEconomizer: this turn still looks stuck after a redirect ({short}, {} tool calls). {hint}",
                        g.cum_turns
                    );
                    out = hosts::notice(&self.ctx.host, &text);
                    if profile.relays_notices {
                        out = hosts::merge_outputs(&[
                            out,
                            hosts::context(&self.ctx.host, event, &hosts::relay(&text)),
                        ])?;
                    }
                }
            }
        }
        s["turn"] = g.to_dict();
        self.save(&mut s)?;
        Ok(out)
    }
    pub fn session_end(&self, runtime: &mut dyn Runtime) -> Result<Value> {
        let tp = session::transcript(&self.ctx.host, &self.p, &self.environment)
            .map_err(|e| e.to_string())?;
        if !self.ctx.store.base.join(self.sess_name()).exists()
            && !tp
                .as_str()
                .is_some_and(|p| !p.is_empty() && self.environment.path(Path::new(p)).exists())
        {
            return Ok(json!({}));
        }
        let mut s = self.session()?;
        self.close_turn(&mut s, hosts::completed_at_end(&self.ctx.host, &self.p))?;
        s["ended"] = json!(history::now());
        s["end_reason"] = self.p["reason"].clone();
        self.save(&mut s)?;
        runtime.spawn_folds(&self.ctx, std::slice::from_ref(&self.sid))?;
        Ok(json!({}))
    }
}

struct Queue<'a> {
    controller: &'a mut LiveController,
    runtime: &'a mut dyn Runtime,
    event: &'a str,
    replay: Option<(LiveController, String)>,
}
impl hookqueue::Runtime for Queue<'_> {
    fn prepare_replay(&mut self, payload: &Value, event: &str) -> Result<hookqueue::Replay> {
        let other = LiveController::new(
            self.controller.ctx.clone(),
            payload.clone(),
            self.controller.environment.clone(),
            self.controller.parent_pid,
            self.controller.windows,
        );
        let info = hookqueue::Replay {
            session_id: other.sid.clone(),
            has_handler: handler(event).is_some(),
        };
        self.replay = Some((other, event.into()));
        Ok(info)
    }
    fn replay_handler(&mut self, replaying: bool) -> Result<()> {
        let (mut ctl, event) = self.replay.take().ok_or("missing replay controller")?;
        ctl.replaying = replaying;
        if let Some(method) = handler(&event) {
            self.runtime.dispatch(&mut ctl, method)?;
        }
        Ok(())
    }
    fn record_error(&mut self, error: &str) -> Result<()> {
        self.controller.record_error(error);
        Ok(())
    }
    fn handler(&mut self) -> Result<Value> {
        self.runtime.dispatch(
            self.controller,
            handler(self.event).ok_or("missing hook handler")?,
        )
    }
    fn enqueue(&mut self, queue: &Path, payload: &Value) -> bool {
        self.runtime.enqueue(queue, payload)
    }
}
fn run_queue(
    controller: &mut LiveController,
    event: &str,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let root = controller.ctx.root.clone();
    let host = controller.ctx.host.clone();
    let sid = controller.sid.clone();
    let repomap = controller.setting("repomap").to_string();
    let payload = controller.p.clone();
    let limits = runtime.queue_limits(event);
    hookqueue::run(
        Hook {
            root: &root,
            host: &host,
            session_id: &sid,
            repomap: &repomap,
            event,
            payload: &payload,
        },
        &limits,
        &mut Queue {
            controller,
            runtime,
            event,
            replay: None,
        },
    )
}
