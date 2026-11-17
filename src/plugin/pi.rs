pub use super::javascript::{disabled, space};
use super::javascript::{nullish, string, truthy};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub mod server;
pub type Result<T> = std::result::Result<T, String>;
pub const SECTION: &str = "tokeneconomizer";
pub const HOOK_TIMEOUT_S: f64 = 60.0;

pub fn carried_section(entries: &[Value]) -> Result<String> {
    let mut text = json!("");
    for entry in entries {
        if entry["type"] == "compaction" && truthy(&entry["systemMessage"]["sections"]) {
            text = nullish(entry["systemMessage"]["sections"].get(SECTION), &json!("")).clone();
        } else if entry["type"] == "message"
            && entry["message"]["role"] == "system"
            && truthy(&entry["message"]["sections"])
        {
            let sections = &entry["message"]["sections"];
            if !sections.is_object() && !sections.is_array() {
                return Err("system sections must be an object".into());
            }
            if let Some(value) = sections.get(SECTION) {
                text = nullish(Some(value), &json!("")).clone();
            }
        }
    }
    let text = text.as_str().ok_or("carried section must be text")?;
    let open = format!("<{SECTION}>\n");
    let close = format!("\n</{SECTION}>");
    Ok(text
        .strip_prefix(&open)
        .and_then(|s| s.strip_suffix(&close))
        .unwrap_or(text)
        .into())
}

pub trait Runtime {
    fn hook(&mut self, payload: &Value, context: &Value) -> Result<Value>;
    fn command(&mut self, args: &[String], context: &Value) -> Result<String>;
}
pub struct Prepared {
    event: String,
    context: Value,
    body: Value,
    payload: Value,
}
impl Prepared {
    fn failed_output(&self) -> Value {
        if self.event == "command" {
            json!("")
        } else {
            json!({})
        }
    }

    pub fn execute(&self, runtime: &mut dyn Runtime) -> Result<Value> {
        if self.event == "command" {
            let args = nullish(self.body.get("args"), &Value::Null);
            let args = if truthy(args) {
                args.as_str()
                    .ok_or("command arguments must be text")?
                    .trim_matches(space)
            } else {
                ""
            };
            let mut argv = vec![
                "--host".into(),
                "pi".into(),
                "--cwd".into(),
                self.context["cwd"]
                    .as_str()
                    .ok_or("cwd must be text")?
                    .into(),
                "--argline".into(),
            ];
            argv.push(if args.is_empty() { "status" } else { args }.into());
            runtime.command(&argv, &self.context).map(Value::String)
        } else {
            runtime.hook(&self.payload, &self.context)
        }
    }
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bridge {
    manual: Value,
    prompts: f64,
    turn: String,
}
impl Bridge {
    fn restore(state: Value) -> Result<Self> {
        let bridge: Self = serde_json::from_value(state).map_err(|e| e.to_string())?;
        if !bridge.prompts.is_finite()
            || !(0.0..=9_007_199_254_740_992.0).contains(&bridge.prompts)
            || bridge.prompts.fract() != 0.0
        {
            return Err("invalid Pi prompt counter".into());
        }
        Ok(bridge)
    }

    pub fn prepare(&mut self, event: &str, context: &Value, body: &Value) -> Result<Prepared> {
        let mut payload = Map::new();
        for key in ["session_id", "transcript_path", "cwd"] {
            if let Some(value) = context.get(key) {
                payload.insert(key.into(), value.clone());
            }
        }
        let fields = match event {
            "session_start" => {
                let entries = body["entries"]
                    .as_array()
                    .ok_or("branch entries must be a list")?;
                self.prompts = 0.0;
                let carried = entries.iter().any(|entry| entry["type"] == "message");
                let source = if carried {
                    "resume"
                } else {
                    match body["reason"].as_str() {
                        Some("new") => "clear",
                        Some("resume" | "fork" | "reload") => "resume",
                        _ => "startup",
                    }
                };
                json!({"hook_event_name":"SessionStart","source":source})
            }
            "before_agent_start" => {
                self.prompts += 1.0;
                self.turn = format!(
                    "{}:{:.0}",
                    context
                        .get("session_id")
                        .map(string)
                        .unwrap_or("undefined".into()),
                    self.prompts
                );
                let mut fields =
                    json!({"hook_event_name":"UserPromptSubmit","prompt_id":self.turn});
                if let Some(prompt) = body.get("prompt") {
                    fields["prompt"] = prompt.clone();
                }
                fields
            }
            "tool_result" => {
                let content = nullish(body.get("content"), &json!([]))
                    .as_array()
                    .ok_or("tool content must be a list")?
                    .iter()
                    .filter(|c| c["type"] == "text")
                    .map(|c| nullish(c.get("text"), &json!("")).to_owned())
                    .map(|v| string(&v))
                    .collect::<Vec<_>>()
                    .join("\n");
                let mut response = json!({"content":content});
                if let Some(value) = body.get("isError") {
                    response["isError"] = value.clone();
                }
                if let Some(value) = body["structuredContent"].get("exit_code") {
                    response["exit_code"] = value.clone();
                }
                let mut fields = json!({"hook_event_name":"PostToolUse","prompt_id":self.turn,"tool_input":nullish(body.get("input"),&json!({})),"tool_response":response});
                for (source, target) in [("toolName", "tool_name"), ("toolCallId", "tool_use_id")] {
                    if let Some(value) = body.get(source) {
                        fields[target] = value.clone();
                    }
                }
                fields
            }
            "agent_end" => {
                json!({"hook_event_name":"Stop","prompt_id":self.turn,"stop_hook_active":false})
            }
            "session_shutdown" => {
                let mut fields = json!({"hook_event_name":"SessionEnd"});
                if let Some(reason) = body.get("reason") {
                    fields["reason"] = reason.clone();
                }
                fields
            }
            "command" => json!({}),
            _ => return Err(format!("unknown Pi event: {event}")),
        };
        payload.extend(fields.as_object().unwrap().clone());
        Ok(Prepared {
            event: event.into(),
            context: context.clone(),
            body: body.clone(),
            payload: json!(payload),
        })
    }
    pub fn complete(&mut self, prepared: &Prepared, out: Value) -> Result<Value> {
        let mut actions = Vec::new();
        let extra = out
            .get("hookSpecificOutput")
            .and_then(|v| v.get("additionalContext"));
        let notice = out
            .get("systemMessage")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        match prepared.event.as_str() {
            "session_start" => {
                let entries = prepared.body["entries"]
                    .as_array()
                    .ok_or("branch entries must be a list")?;
                self.manual = if entries.iter().any(|e| e["type"] == "message") {
                    json!(carried_section(entries)?)
                } else {
                    nullish(extra, &json!("")).clone()
                };
                if let Some(text) = notice {
                    actions.push(json!({"method":"notify","text":text}));
                }
            }
            "before_agent_start" => {
                if let Some(extra) = extra.filter(|v| truthy(v)) {
                    self.manual = if truthy(&self.manual) {
                        json!(format!("{}\n\n{}", string(&self.manual), string(extra)))
                    } else {
                        extra.clone()
                    };
                }
                if let Some(text) = notice {
                    actions.push(json!({"method":"notify","text":text}));
                }
                if truthy(&self.manual) {
                    actions.push(json!({"method":"sections","sections":{SECTION:self.manual}}));
                }
            }
            "tool_result" => {
                if let Some(nudge) = extra.filter(|v| truthy(v)) {
                    actions.push(json!({"method":"steer","message":{"customType":SECTION,"content":nudge,"display":true},"options":{"deliverAs":"steer"}}));
                }
                if let Some(text) = notice {
                    actions.push(json!({"method":"notify","text":text}));
                }
            }
            "command" => {
                let text = out
                    .as_str()
                    .ok_or("CLI output must be text")?
                    .trim_matches(space);
                actions.push(json!({"method":"notify","text":if text.is_empty(){"te: no output (check the native CLI)"}else{text}}));
            }
            _ => (),
        }
        Ok(json!({"actions":actions}))
    }
}
