use super::{denylist, hosts::session::Environment, sidecall};
use crate::value::{json_text, number, round_decimal, truthy};
use serde_json::{Map, Value, json};
use std::path::Path;

pub trait Runtime: sidecall::Runtime {
    fn override_probe(&mut self, _method: &str, _args: Value) -> Result<Option<Value>, String> {
        Ok(None)
    }
    fn capture(
        &mut self,
        extra_args: &[String],
        cwd: Option<&Path>,
        cli: &str,
        timeout: f64,
    ) -> Result<Value, String>;
}

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
        let flags = match runtime.override_probe("claude_flags", json!({"cli":cli}))? {
            Some(value) => serde_json::from_value(value).map_err(|e| e.to_string())?,
            None => sidecall::claude_flags(&sidecall::claude_help(
                &cli,
                &mut self.help,
                &self.environment,
                runtime,
            )),
        };
        cmd.extend(flags);
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
        let body = runtime.capture(&[], Some(root), cli, 90.)?;
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
            let full = runtime.capture(&[], None, cli, 90.)?;
            let lean = runtime.capture(&args, None, cli, 90.)?;
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
