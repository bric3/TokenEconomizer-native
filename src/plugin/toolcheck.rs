mod capture;

use super::{adapter, denylist, hosts::session::Environment, process, sidecall};
use crate::value::{json_text, number, round_decimal, truthy};
use serde_json::{Map, Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub trait Runtime: sidecall::Runtime {
    fn override_probe(&mut self, _method: &str, _args: Value) -> Result<Option<Value>, String> {
        Ok(None)
    }
    fn start_probe(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &Map<String, Value>,
        new_session: bool,
    ) -> Result<Box<dyn process::Probe>, String> {
        process::start_probe(argv, cwd, env, new_session).map_err(|e| e.to_string())
    }
}
impl Runtime for sidecall::runner::NativeRuntime {}

pub struct Checks {
    pub environment: Environment,
    pub windows: bool,
    pub help: Map<String, Value>,
}
fn whitespace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}'..='\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}
pub fn request_tools(body: &Value) -> Vec<Value> {
    body["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|tool| {
            [&tool["function"]["name"], &tool["name"]]
                .into_iter()
                .find(|v| truthy(v))
                .cloned()
                .unwrap_or_else(|| tool["type"].clone())
        })
        .collect()
}
impl Checks {
    fn flags<R: Runtime>(&mut self, cli: &str, runtime: &mut R) -> Result<Vec<String>, String> {
        match runtime.override_probe("claude_flags", json!({"cli":cli}))? {
            Some(value) => serde_json::from_value(value).map_err(|e| e.to_string()),
            None => Ok(sidecall::claude_flags(&sidecall::claude_help(
                cli,
                &mut self.help,
                &self.environment,
                runtime,
            ))),
        }
    }
    pub fn init_tools<R: Runtime>(
        &mut self,
        extra_args: &[String],
        cli: &str,
        timeout: f64,
        runtime: &mut R,
    ) -> Result<(Value, Value), String> {
        if let Some(value) = runtime.override_probe(
            "_init_tools",
            json!({"extra_args":extra_args,"cli":cli,"timeout":timeout}),
        )? {
            return Ok((value[0].clone(), value[1].clone()));
        }
        let cli = self.cli(cli, runtime)?;
        let mut cmd: Vec<String> = [
            cli.as_str(),
            "-p",
            "Reply with OK",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            "haiku",
            "--max-turns",
            "1",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        cmd.extend(self.flags(&cli, runtime)?);
        cmd.extend_from_slice(extra_args);
        let env = self.env();
        sidecall::with_directory("te-probe-", true, runtime, |cwd, runtime| {
            let mut probe = runtime.start_probe(&cmd, cwd, &env, !self.windows)?;
            let started = Instant::now();
            let result = (|| {
                let deadline =
                    Duration::try_from_secs_f64(timeout.max(0.)).map_err(|e| e.to_string())?;
                while let Some(remaining) = deadline.checked_sub(started.elapsed()) {
                    let Some(line) = probe
                        .line(remaining.as_secs_f64())
                        .map_err(|e| e.to_string())?
                    else {
                        break;
                    };
                    let Ok(event) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    if event["type"] == "system" && event["subtype"] == "init" {
                        return Ok((event["tools"].clone(), event["claude_code_version"].clone()));
                    }
                }
                Ok((Value::Null, Value::Null))
            })();
            probe.kill_tree().map_err(|e| e.to_string())?;
            result
        })
    }
    pub fn refresh_catalog<R: Runtime>(
        &mut self,
        root: &Path,
        cli: &str,
        host: &str,
        runtime: &mut R,
    ) -> Result<Value, String> {
        let (tools, version, version_key) = match host {
            "copilot" => {
                let body = self.capture(
                    &[],
                    None,
                    if cli.is_empty() { "copilot" } else { cli },
                    90.,
                    runtime,
                )?;
                let tools: Vec<_> = request_tools(&body).into_iter().filter(truthy).collect();
                if tools.is_empty() {
                    return Err("could not capture the tool list of a `copilot -p` request".into());
                }
                let version =
                    match runtime.override_probe("_copilot_version", json!({"cli":"copilot"}))? {
                        Some(value) => value,
                        None => self.copilot_version("copilot", runtime),
                    };
                (json!(tools), version, "copilot_version")
            }
            "claude" => {
                let (tools, version) = self.init_tools(&[], cli, 60., runtime)?;
                if !truthy(&tools) {
                    return Err("could not read the tool list from a `claude -p` init event".into());
                }
                (tools, version, "claude_code_version")
            }
            _ => return Err(format!("no catalog probe for {host}")),
        };
        let mut tools: Vec<String> = serde_json::from_value(tools).map_err(|e| e.to_string())?;
        tools.sort();
        let path = adapter::catalog_path(&self.environment.path(root), host);
        let mut doc = json!({"tools":tools});
        doc[version_key] = version;
        doc["probed_at"] = json!(runtime.now()?);
        if runtime
            .override_probe("_write_json", json!({"path":path,"obj":doc}))?
            .is_none()
        {
            denylist::write_json(&path, &doc).map_err(|e| e.to_string())?;
        }
        Ok(json!([path, tools]))
    }
    pub fn capture<R: Runtime>(
        &self,
        extra_args: &[String],
        cwd: Option<&Path>,
        cli: &str,
        timeout: f64,
        runtime: &mut R,
    ) -> Result<Value, String> {
        if let Some(value) = runtime.override_probe(
            "copilot_capture",
            json!({"extra_args":extra_args,"cwd":cwd,"cli":cli,"timeout":timeout}),
        )? {
            return Ok(value);
        }
        let server = capture::Capture::start().map_err(|e| e.to_string())?;
        let mut model = sidecall::copilot_model("", &self.environment.env);
        if model.is_empty() {
            model = "gpt-5.4".into();
        }
        let model = model.split_once('/').map(|(_, m)| m).unwrap_or(&model);
        let mut env: Map<_, _> = self
            .environment
            .env
            .iter()
            .filter(|(k, _)| !k.starts_with("COPILOT_PROVIDER_") && *k != "COPILOT_MODEL")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (key, value) in [
            ("TE_SIDECALL", "1".into()),
            ("COPILOT_PROVIDER_TYPE", "openai".into()),
            (
                "COPILOT_PROVIDER_BASE_URL",
                format!("http://127.0.0.1:{}/v1", server.port),
            ),
            ("COPILOT_PROVIDER_API_KEY", "te-local-probe".into()),
            ("COPILOT_MODEL", model.to_string()),
            ("COPILOT_PROVIDER_WIRE_API", "completions".into()),
            ("NO_PROXY", "127.0.0.1,localhost".into()),
            ("no_proxy", "127.0.0.1,localhost".into()),
        ] {
            env.insert(key.into(), json!(value));
        }
        let temporary = if cwd.is_none() {
            Some(sidecall::WorkDir::new("te-probe-").map_err(|e| e.to_string())?)
        } else {
            None
        };
        let cwd = cwd
            .map(|p| self.environment.path(p))
            .unwrap_or_else(|| temporary.as_ref().unwrap().path().into());
        let _ = (|| -> Result<(), String> {
            let mut cmd = runtime.program(cli, self.windows, &self.environment)?;
            cmd.extend([
                "-p".into(),
                "Reply with OK".into(),
                "--no-auto-update".into(),
                "--no-color".into(),
            ]);
            cmd.extend_from_slice(extra_args);
            runtime
                .run(&cmd, &cwd, &env, None, timeout)
                .map_err(|e| e.to_string())?;
            Ok(())
        })();
        Ok(server.finish())
    }
    fn env(&self) -> Map<String, Value> {
        sidecall::environment(&self.environment.env, false)
    }
    fn cli<R: Runtime>(&self, cli: &str, runtime: &mut R) -> Result<String, String> {
        if cli.is_empty() {
            runtime.claude_cli(&self.environment, self.windows)
        } else {
            Ok(cli.into())
        }
    }
    pub fn prompt_tokens<R: Runtime>(
        &mut self,
        extra_args: &[String],
        cli: &str,
        timeout: f64,
        runtime: &mut R,
    ) -> Result<(Value, Value), String> {
        if let Some(value) = runtime.override_probe(
            "_prompt_tokens",
            json!({"extra_args":extra_args,"cli":cli,"timeout":timeout}),
        )? {
            return Ok((value[0].clone(), value[1].clone()));
        }
        let cli = self.cli(cli, runtime)?;
        let mut cmd: Vec<String> = [
            cli.as_str(),
            "-p",
            "Reply with exactly: OK",
            "--output-format",
            "json",
            "--model",
            "haiku",
            "--max-turns",
            "1",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        cmd.extend(self.flags(&cli, runtime)?);
        cmd.extend_from_slice(extra_args);
        let env = self.env();
        let out = sidecall::with_directory("te-measure-", true, runtime, |cwd, runtime| {
            runtime
                .run(&cmd, cwd, &env, None, timeout)
                .map_err(|e| e.to_string())
        })?;
        let data = sidecall::claude_json(&out.stdout);
        let mut total = 0i128;
        for key in [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
        ] {
            total = total
                .checked_add(sidecall::usage_count(&data["usage"][key])?)
                .ok_or("probe usage exceeds i128")?;
        }
        if out.code != 0 || truthy(&data["is_error"]) || total == 0 {
            let what = if out.code == 0 {
                "reported no usage".into()
            } else {
                format!("exited {}", out.code)
            };
            let text = if !out.stderr.is_empty() {
                &out.stderr
            } else {
                &out.stdout
            };
            let text: String = text.trim_matches(whitespace).chars().take(300).collect();
            return Err(format!(
                "`claude -p` {what}: {}",
                if text.is_empty() { "no output" } else { &text }
            ));
        }
        // shortcut: returned totals use JSON's integer range, widen it in the numeric parity audit.
        Ok((
            serde_json::to_value(total).map_err(|e| e.to_string())?,
            data["total_cost_usd"].clone(),
        ))
    }
    pub fn codex_prompt_chars<R: Runtime>(
        &self,
        overrides: &[(Option<&str>, &str, &str)],
        cli: &str,
        timeout: f64,
        runtime: &mut R,
    ) -> Result<Option<i64>, String> {
        if let Some(value) = runtime.override_probe(
            "_codex_prompt_chars",
            json!({"overrides":overrides,"cli":cli,"timeout":timeout}),
        )? {
            return Ok(value.as_i64());
        }
        let mut cmd = runtime.program(cli, self.windows, &self.environment)?;
        cmd.extend(["debug".into(), "prompt-input".into()]);
        for (table, key, value) in overrides {
            cmd.extend([
                "-c".into(),
                format!(
                    "{}{key}={value}",
                    table
                        .filter(|t| !t.is_empty())
                        .map(|t| format!("{t}."))
                        .unwrap_or_default()
                ),
            ]);
        }
        cmd.push("OK".into());
        let env = self.env();
        sidecall::with_directory("te-measure-", true, runtime, |cwd, runtime| {
            let out = runtime
                .run(&cmd, cwd, &env, None, timeout)
                .map_err(|e| e.to_string())?;
            Ok(
                (out.code == 0 && !out.stdout.is_empty())
                    .then(|| out.stdout.chars().count() as i64),
            )
        })
    }
    pub fn verify_codex<R: Runtime>(
        &self,
        root: &Path,
        capabilities: &[String],
        cli: &str,
        timeout: f64,
        runtime: &mut R,
    ) -> Result<Value, String> {
        let features: Vec<&str> = capabilities
            .iter()
            .flat_map(|c| denylist::toggles(c))
            .filter_map(|(table, key, _)| (*table == Some("features")).then_some(*key))
            .collect();
        if features.is_empty() || runtime.which(cli)?.is_none() {
            return Ok(Value::Null);
        }
        let out = (|| {
            let mut cmd = runtime.program(cli, self.windows, &self.environment)?;
            cmd.extend(["features".into(), "list".into()]);
            runtime
                .run(
                    &cmd,
                    &self.environment.path(root),
                    &self.env(),
                    None,
                    timeout,
                )
                .map_err(|e| e.to_string())
        })();
        let Ok(out) = out else {
            return Ok(Value::Null);
        };
        let mut state = Map::new();
        for line in out.stdout.split(line_break) {
            let parts: Vec<_> = line.split(whitespace).filter(|s| !s.is_empty()).collect();
            if parts.len() >= 2 && features.contains(&parts[0]) {
                state.insert(parts[0].into(), json!(parts.last() == Some(&"true")));
            }
        }
        Ok(if state.is_empty() {
            Value::Null
        } else {
            json!(state)
        })
    }
    pub fn copilot_version<R: Runtime>(&self, cli: &str, runtime: &mut R) -> Value {
        let result = (|| {
            let mut cmd = runtime.program(cli, self.windows, &self.environment)?;
            cmd.push("--version".into());
            runtime
                .run(
                    &cmd,
                    &self.environment.cwd,
                    &self.environment.env,
                    None,
                    30.,
                )
                .map_err(|e| e.to_string())
        })();
        result
            .ok()
            .and_then(|out| {
                out.stdout
                    .trim_matches(whitespace)
                    .split(line_break)
                    .find(|s| !s.is_empty())
                    .map(|v| json!(v))
            })
            .unwrap_or(Value::Null)
    }
    pub fn verify_copilot<R: Runtime>(
        &self,
        root: &Path,
        servers: &[String],
        cli: &str,
        runtime: &mut R,
    ) -> Result<Value, String> {
        let body = self.capture(&[], Some(root), cli, 90., runtime)?;
        if !truthy(&body) {
            return Ok(Value::Null);
        }
        let names = request_tools(&body);
        let result: Map<String, Value> = servers
            .iter()
            .map(|s| {
                (
                    s.clone(),
                    json!(
                        names
                            .iter()
                            .filter(|n| n.as_str().is_some_and(|n| n.starts_with(&format!("{s}-"))))
                            .count()
                    ),
                )
            })
            .collect();
        Ok(json!(result))
    }
    pub fn measure<R: Runtime>(
        &mut self,
        tools: &[String],
        cli: &str,
        host: &str,
        runtime: &mut R,
    ) -> Result<Value, String> {
        if tools.is_empty() {
            return Ok(Value::Null);
        }
        if host == "codex" {
            let toggles: Vec<_> = tools
                .iter()
                .flat_map(|c| denylist::toggles(c).iter().copied())
                .collect();
            let cli = if cli.is_empty() { "codex" } else { cli };
            let full = self.codex_prompt_chars(&[], cli, 60., runtime)?;
            let lean = self.codex_prompt_chars(&toggles, cli, 60., runtime)?;
            let (Some(full), Some(lean)) = (full, lean) else {
                return Err("`codex debug prompt-input` failed".into());
            };
            return Ok(
                json!({"capabilities":tools,"prompt_input_chars_full":full,"prompt_input_chars_lean":lean,"saved_chars_per_request":full-lean,"saved_tokens_per_request_est":(full-lean).div_euclid(4),"note":"prompt input only (developer/user items); tool schemas not included","probe_cost_usd":0.0}),
            );
        }
        if host == "copilot" {
            let args: Vec<String> = tools
                .iter()
                .flat_map(|s| ["--disable-mcp-server".into(), s.clone()])
                .collect();
            let cli = if cli.is_empty() { "copilot" } else { cli };
            let full = self.capture(&[], None, cli, 90., runtime)?;
            let lean = self.capture(&args, None, cli, 90., runtime)?;
            if !truthy(&full) || !truthy(&lean) {
                return Err("could not capture a `copilot -p` request".into());
            }
            let size = |b: &Value| {
                (json_text(&b["tools"]).chars().count() + json_text(&b["messages"]).chars().count())
                    as i64
            };
            let (f, l) = (size(&full), size(&lean));
            return Ok(
                json!({"mcp_servers":tools,"tools_full":request_tools(&full).len(),"tools_lean":request_tools(&lean).len(),"request_chars_full":f,"request_chars_lean":l,"saved_chars_per_request":f-l,"saved_tokens_per_request_est":(f-l).div_euclid(4),"probe_cost_usd":0.0}),
            );
        }
        let cli = self.cli(cli, runtime)?;
        let (full, c1) = self.prompt_tokens(&[], &cli, 180., runtime)?;
        let (lean, c2) = sidecall::with_directory("te-deny-", true, runtime, |dir, runtime| {
            let settings = dir.join("settings.json");
            denylist::write_json(&settings, &json!({"permissions":{"deny":tools}}))
                .map_err(|e| e.to_string())?;
            self.prompt_tokens(
                &["--settings".into(), settings.to_string_lossy().into_owned()],
                &cli,
                180.,
                runtime,
            )
        })?;
        let saved = sidecall::usage_count(&full)?
            .checked_sub(sidecall::usage_count(&lean)?)
            .ok_or("probe savings exceed i128")?;
        Ok(
            json!({"tools":tools,"prompt_tokens_full":full,"prompt_tokens_lean":lean,"saved_per_request":serde_json::to_value(saved).map_err(|e|e.to_string())?,"probe_cost_usd":round_decimal(number(&c1).unwrap_or(0.) + number(&c2).unwrap_or(0.), 4)}),
        )
    }
}
