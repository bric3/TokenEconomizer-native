use serde_json::{Map, Value, json};

pub mod server;
pub type Result<T> = std::result::Result<T, String>;
pub const SECTION: &str = "tokeneconomizer";
pub const HOOK_TIMEOUT_S: f64 = 60.0;

pub fn space(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}
pub fn disabled(env: &Map<String, Value>) -> bool {
    ["TE_SIDECALL", "TE_DISABLE"].iter().any(|key| {
        env.get(*key).and_then(Value::as_str).is_some_and(|v| {
            ["1", "true", "yes", "on"].contains(&v.trim_matches(space).to_lowercase().as_str())
        })
    })
}
fn nullish<'a>(value: Option<&'a Value>, fallback: &'a Value) -> &'a Value {
    value.filter(|v| !v.is_null()).unwrap_or(fallback)
}
fn string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            let n = n.as_f64().expect("JSON number fits f64");
            if n == 0.0 {
                "0".into()
            } else if n.abs() >= 1e21 {
                format!("{n:e}").replacen('e', "e+", 1)
            } else if n.abs() < 1e-6 {
                format!("{n:e}")
            } else {
                n.to_string()
            }
        }
        Value::Array(values) => values
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    string(v)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

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
#[derive(Default)]
pub struct Bridge {
    manual: Value,
    prompts: f64,
    turn: String,
}
impl Bridge {
    pub fn prepare(&mut self, event: &str, context: &Value, body: &Value) -> Result<Prepared> {
        let mut payload = Map::new();
        for key in ["session_id", "transcript_path", "cwd"] {
            if let Some(value) = context.get(key) {
                payload.insert(key.into(), value.clone());
            }
        }
        let fields = match event {
            "session_start" => {
                self.prompts = 0.0;
                let entries = body["entries"]
                    .as_array()
                    .ok_or("branch entries must be a list")?;
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
