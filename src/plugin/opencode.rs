use super::javascript::{nullish, string, truthy};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard},
};

pub mod server;
pub type Result<T> = std::result::Result<T, String>;
type Message = Arc<Mutex<Value>>;
type Id = Option<Value>;

fn key(id: Option<&Value>) -> String {
    id.map(Value::to_string).unwrap_or("undefined".into())
}
fn id_text(id: &Id) -> String {
    id.as_ref().map(string).unwrap_or("undefined".into())
}
fn text_of(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.into();
    }
    value
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .map(|part| {
                    if part.is_string() {
                        part
                    } else {
                        nullish(part.get("text"), &Value::Null)
                    }
                })
                .filter(|part| truthy(part))
                .map(string)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
fn text_parts(value: &Value) -> Result<String> {
    Ok(nullish(Some(value), &json!([]))
        .as_array()
        .ok_or("parts must be a list")?
        .iter()
        .filter(|part| part["type"] == "text")
        .map(|part| string(nullish(part.get("text"), &json!(""))))
        .collect::<Vec<_>>()
        .join("\n"))
}
fn extra(out: &Value) -> Option<&Value> {
    out.get("hookSpecificOutput")?.get("additionalContext")
}

pub trait Runtime {
    fn hook(&mut self, payload: &Value) -> Result<Value>;
    fn export(&mut self, sid: &Id, directory: &Id, records: Option<Value>) -> Result<String>;
    fn notice(&mut self, text: &str, stderr: bool) -> Result<()>;
    fn apply(&mut self, action: Value) -> Result<()>;
    fn mode(&mut self) -> Result<String>;
}

pub struct State {
    closing: bool,
    initialised: bool,
    version: u8,
    directory: Id,
    manuals: HashMap<String, Value>,
    prompts: HashMap<String, f64>,
    turns: HashMap<String, String>,
    open: IndexMap<String, Id>,
    current: Value,
    unsettled: HashSet<String>,
    records: HashMap<String, Vec<Message>>,
    steps: HashMap<String, Message>,
    names: HashMap<String, Value>,
}
impl State {
    pub fn new(version: u8) -> Result<Self> {
        if ![1, 2].contains(&version) {
            return Err("opencode SDK version must be 1 or 2".into());
        }
        Ok(Self {
            closing: false,
            initialised: false,
            version,
            directory: None,
            manuals: HashMap::new(),
            prompts: HashMap::new(),
            turns: HashMap::new(),
            open: IndexMap::new(),
            current: json!(""),
            unsettled: HashSet::new(),
            records: HashMap::new(),
            steps: HashMap::new(),
            names: HashMap::new(),
        })
    }
    fn base(&self, sid: &Id, event: &str) -> Value {
        let mut payload = json!({"transcript_path":"","hook_event_name":event});
        if let Some(id) = sid {
            payload["session_id"] = id.clone();
        }
        if let Some(cwd) = &self.directory {
            payload["cwd"] = cwd.clone();
        }
        payload
    }
    fn start(&mut self, sid: &Id) -> bool {
        let key = key(sid.as_ref());
        if self.open.contains_key(&key) {
            return false;
        }
        self.open.insert(key.clone(), sid.clone());
        self.prompts.insert(key.clone(), 0.0);
        if self.version == 2 {
            self.records.insert(key, Vec::new());
        }
        true
    }
    fn prompt(&mut self, sid: &Id, body: &Value) -> Result<Value> {
        if self.closing {
            return Err("opencode SDK is exiting".into());
        }
        let key = key(sid.as_ref());
        let n = self.prompts.entry(key.clone()).or_default();
        *n += 1.0;
        let turn = format!("{}:{}", id_text(sid), string(&json!(n)));
        self.turns.insert(key.clone(), turn.clone());
        let text = if self.version == 1 {
            json!(text_parts(&body["output"]["parts"])?)
        } else {
            nullish(body["prompt"].get("text"), &json!("")).clone()
        };
        if self.version == 2 {
            self.unsettled.insert(key.clone());
            let message = json!({"info":{"id":string(nullish(body.get("messageID"), &json!(turn))),"role":"user"},"parts":[{"type":"text","text":text}]});
            if let Some(records) = self.records.get_mut(&key) {
                records.push(Arc::new(Mutex::new(message)));
            }
        }
        let mut payload = self.base(sid, "UserPromptSubmit");
        payload["prompt_id"] = json!(turn);
        payload["prompt"] = text;
        Ok(payload)
    }
    fn assistant(&mut self, sid: &Id, mid: Option<&Value>) -> Message {
        self.steps
            .entry(key(mid))
            .or_insert_with(|| {
                let mut message = json!({"info":{"role":"assistant","path":{}},"parts":[]});
                if let Some(id) = mid {
                    message["info"]["id"] = id.clone();
                }
                if let Some(cwd) = &self.directory {
                    message["info"]["path"]["cwd"] = cwd.clone();
                }
                let message = Arc::new(Mutex::new(message));
                if let Some(records) = self.records.get_mut(&key(sid.as_ref())) {
                    records.push(message.clone());
                }
                message
            })
            .clone()
    }
    fn record(&mut self, kind: &str, p: &Value, sid: &Id) -> Result<()> {
        if kind == "session.tool.input.started" {
            self.names.insert(
                key(p.get("id")),
                p.get("name").cloned().unwrap_or(Value::Null),
            );
            return Ok(());
        }
        let assistant = self.assistant(sid, p.get("assistantMessageID"));
        let mut m = assistant.lock().map_err(|e| e.to_string())?;
        match kind {
            "session.step.started" => {
                if let Some(model) = p["model"]
                    .get("id")
                    .filter(|v| !v.is_null())
                    .or_else(|| p["model"].get("modelID").filter(|v| !v.is_null()))
                {
                    m["info"]["modelID"] = model.clone();
                }
            }
            "session.step.ended" => {
                m["info"]["cost"] = nullish(p.get("cost"), &json!(0)).clone();
                if let Some(tokens) = p.get("tokens") {
                    m["info"]["tokens"] = tokens.clone();
                } else {
                    m["info"].as_object_mut().unwrap().shift_remove("tokens");
                }
            }
            "session.tool.called" => {
                let mut part = json!({"type":"tool","tool":nullish(self.names.get(&key(p.get("id"))), &json!("")),"state":{"status":"running","input":nullish(p.get("input"), &json!({}))}});
                if let Some(id) = p.get("id") {
                    part["callID"] = id.clone();
                }
                m["parts"].as_array_mut().unwrap().push(part);
            }
            "session.tool.success" | "session.tool.failed" => {
                if let Some(part) = m["parts"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|part| part.get("callID") == p.get("id"))
                {
                    let input = part["state"]["input"].clone();
                    part["state"] = if kind == "session.tool.success" {
                        json!({"status":"completed","input":input,"output":text_of(&p["content"]),"metadata":nullish(p.get("metadata"), &json!({}))})
                    } else {
                        json!({"status":"error","input":input,"error":string(nullish(p["error"].get("message"), &json!(text_of(&p["content"]))))})
                    };
                }
            }
            _ => return Err(format!("unknown record event: {kind}")),
        }
        Ok(())
    }
    fn tool(&self, sid: &Id, body: &Value) -> (Value, String) {
        let v2 = self.version == 2;
        let output = if v2 { &body["result"] } else { &body["output"] };
        let ok = !v2 || body["status"] == "completed";
        let text = if v2 && !ok {
            string(nullish(
                body.get("errorMessage"),
                nullish(body.get("error"), &json!("")),
            ))
        } else if v2 {
            let text = text_of(&output["content"]);
            if text.is_empty() {
                string(nullish(output.get("output"), &json!("")))
            } else {
                text
            }
        } else {
            string(nullish(output.get("output"), &json!("")))
        };
        let exit = if ok {
            output["metadata"].get("exit").filter(|v| v.is_number())
        } else {
            None
        };
        let mut response =
            json!({"content":text,"isError":!ok || exit.is_some_and(|n| n.as_f64() != Some(0.0))});
        if let Some(exit) = exit {
            response["exit_code"] = exit.clone();
        }
        let mut payload = self.base(sid, "PostToolUse");
        payload["prompt_id"] = json!(
            self.turns
                .get(&key(sid.as_ref()))
                .cloned()
                .unwrap_or_default()
        );
        payload["tool_input"] =
            nullish(body.get(if v2 { "input" } else { "args" }), &json!({})).clone();
        payload["tool_response"] = response;
        for (source, target) in [
            ("tool", "tool_name"),
            (if v2 { "id" } else { "callID" }, "tool_use_id"),
        ] {
            if let Some(value) = body.get(source) {
                payload[target] = value.clone();
            }
        }
        (payload, text)
    }
    pub fn prepare(&mut self, method: &str, body: &Value) -> Result<Job> {
        if self.closing {
            return Err("opencode SDK is exiting".into());
        }
        if !self.initialised && !["init", "exit"].contains(&method) {
            return Err("opencode SDK is not initialised".into());
        }
        let mut job = Job {
            sid: body.get("sessionID").cloned(),
            start: false,
            operation: Operation::Done,
            body: body.clone(),
            payload: None,
            actions: Vec::new(),
        };
        match method {
            "init" => {
                if self.initialised {
                    return Err("opencode SDK is already initialised".into());
                }
                self.initialised = true;
                self.directory = if self.version == 1 {
                    body.get("directory").cloned()
                } else {
                    Some(
                        nullish(
                            body.get("directory"),
                            nullish(body.get("path"), &body["process_cwd"]),
                        )
                        .clone(),
                    )
                };
            }
            "prompt" => {
                if self.version == 1 && !truthy(job.sid.as_ref().unwrap_or(&Value::Null)) {
                    return Ok(job);
                }
                if self.version == 1 {
                    self.current = job.sid.clone().unwrap();
                }
                job.start = self.start(&job.sid);
                if !job.start {
                    job.payload = Some(self.prompt(&job.sid, body)?);
                }
                job.operation = Operation::Prompt;
            }
            "context" => {
                let id = if self.version == 1 {
                    Some(nullish(body.get("sessionID"), &self.current))
                } else {
                    body.get("sessionID")
                };
                if let Some(manual) = self.manuals.get(&key(id)).filter(|v| truthy(v))
                    && (self.version == 1 || body["system_array"] == true)
                {
                    job.actions.push(json!({"method":"append_system","value":if self.version == 1 { manual.clone() } else { json!({"type":"text","text":manual}) }}));
                }
            }
            "tool" => {
                if self.version == 1 && !truthy(job.sid.as_ref().unwrap_or(&Value::Null)) {
                    return Ok(job);
                }
                let (payload, text) = self.tool(&job.sid, body);
                job.payload = Some(payload);
                job.operation = Operation::Tool(text);
            }
            "event" => {
                let kind = body["type"].as_str().unwrap_or("");
                let p = if self.version == 1 {
                    nullish(body.get("properties"), &Value::Null)
                } else {
                    nullish(body.get("properties"), nullish(body.get("data"), body))
                };
                job.sid = if self.version == 1
                    && ["session.created", "session.deleted"].contains(&kind)
                {
                    Some(nullish(p["info"].get("id"), &p["sessionID"]).clone())
                } else {
                    p.get("sessionID").cloned()
                };
                if !truthy(job.sid.as_ref().unwrap_or(&Value::Null)) {
                    return Ok(job);
                }
                let key = key(job.sid.as_ref());
                match kind {
                    "session.created" if self.version == 1 && !truthy(&p["info"]["parentID"]) => {
                        job.start = self.start(&job.sid);
                    }
                    "session.idle" if self.version == 1 => {
                        job.start = self.start(&job.sid);
                        job.operation = Operation::Stop;
                    }
                    "session.idle"
                    | "session.execution.succeeded"
                    | "session.execution.failed"
                    | "session.execution.interrupted"
                        if self.version == 2 =>
                    {
                        if self.unsettled.remove(&key) {
                            job.operation = Operation::Stop;
                        }
                    }
                    "session.deleted" => {
                        if self.open.shift_remove(&key).is_some() {
                            job.operation = Operation::End("deleted".into());
                        }
                    }
                    "session.step.started"
                    | "session.step.ended"
                    | "session.tool.input.started"
                    | "session.tool.called"
                    | "session.tool.success"
                    | "session.tool.failed"
                        if self.version == 2 =>
                    {
                        self.record(kind, p, &job.sid)?
                    }
                    _ => (),
                }
            }
            "exit" => {
                self.closing = true;
                job.operation = Operation::Exit;
            }
            _ => return Err(format!("unknown opencode callback: {method}")),
        }
        Ok(job)
    }
    fn document(&self, sid: &Id) -> Result<Value> {
        let mut info = json!({});
        if let Some(id) = sid {
            info["id"] = id.clone();
        }
        let messages = self
            .records
            .get(&key(sid.as_ref()))
            .map(|records| {
                records
                    .iter()
                    .map(|m| m.lock().map(|m| m.clone()).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(json!({"info":info,"messages":messages}))
    }
}

enum Operation {
    Done,
    Prompt,
    Tool(String),
    Stop,
    End(String),
    Exit,
}

#[cfg(test)]
mod tests;
pub struct Job {
    sid: Id,
    start: bool,
    operation: Operation,
    body: Value,
    payload: Option<Value>,
    actions: Vec<Value>,
}
pub struct Bridge(Mutex<State>);
impl Bridge {
    pub fn new(version: u8) -> Result<Self> {
        Ok(Self(Mutex::new(State::new(version)?)))
    }
    fn state(&self) -> Result<MutexGuard<'_, State>> {
        self.0.lock().map_err(|e| e.to_string())
    }
    pub fn prepare(&self, method: &str, body: &Value) -> Result<Job> {
        self.state()?.prepare(method, body)
    }
    fn notify(&self, out: &Value, runtime: &mut dyn Runtime) -> Result<()> {
        if let Some(text) = out["systemMessage"].as_str().filter(|s| !s.is_empty()) {
            let version = self.state()?.version;
            let stderr = version == 2 || runtime.mode()? == "run";
            if runtime.notice(text, stderr).is_err() && !stderr {
                runtime.notice(text, true)?;
            }
        }
        Ok(())
    }
    fn exported(&self, sid: &Id, event: &str, runtime: &mut dyn Runtime) -> Result<Value> {
        let (mut payload, directory, doc) = {
            let state = self.state()?;
            (
                state.base(sid, event),
                state.directory.clone(),
                if state.version == 2 {
                    Some(state.document(sid)?)
                } else {
                    None
                },
            )
        };
        payload["transcript_path"] = json!(runtime.export(sid, &directory, doc)?);
        Ok(payload)
    }
    fn end(&self, sid: &Id, reason: &str, runtime: &mut dyn Runtime) -> Result<()> {
        let mut payload = self.exported(sid, "SessionEnd", runtime)?;
        payload["reason"] = json!(reason);
        runtime.hook(&payload)?;
        Ok(())
    }
    pub fn run(&self, mut job: Job, runtime: &mut dyn Runtime) -> Result<Value> {
        if self.state()?.closing && !matches!(job.operation, Operation::Exit) {
            return Ok(json!({"actions":[]}));
        }
        if job.start {
            let mut payload = self.state()?.base(&job.sid, "SessionStart");
            payload["source"] = json!("startup");
            let out = runtime.hook(&payload)?;
            {
                let mut state = self.state()?;
                if state.closing {
                    return Ok(json!({"actions":[]}));
                }
                state.manuals.insert(
                    key(job.sid.as_ref()),
                    nullish(extra(&out), &json!("")).clone(),
                );
            }
            self.notify(&out, runtime)?;
        }
        match job.operation {
            Operation::Done => (),
            Operation::Prompt => {
                let payload = match job.payload {
                    Some(payload) => payload,
                    None => self.state()?.prompt(&job.sid, &job.body)?,
                };
                let out = runtime.hook(&payload)?;
                if self.state()?.closing {
                    return Ok(json!({"actions":[]}));
                }
                if let Some(extra) = extra(&out).filter(|v| truthy(v)) {
                    let mut state = self.state()?;
                    if state.closing {
                        return Ok(json!({"actions":[]}));
                    }
                    let manual = state
                        .manuals
                        .entry(key(job.sid.as_ref()))
                        .or_insert(Value::Null);
                    *manual = if truthy(manual) {
                        json!(format!("{}\n\n{}", string(manual), string(extra)))
                    } else {
                        extra.clone()
                    };
                }
                self.notify(&out, runtime)?;
            }
            Operation::Tool(text) => {
                let out = runtime.hook(job.payload.as_ref().unwrap())?;
                if self.state()?.closing {
                    return Ok(json!({"actions":[]}));
                }
                if let Some(nudge) = extra(&out).filter(|v| truthy(v)) {
                    if self.state()?.version == 1 {
                        job.actions.push(json!({"method":"set_output","value":format!("{text}\n\n{}", string(nudge))}));
                    } else if job.body["status"] == "completed" && truthy(&job.body["result"]) {
                        let content = &job.body["result"]["content"];
                        if let Some(text) = content.as_str() {
                            job.actions.push(json!({"method":"set_content","value":format!("{text}\n\n{}",string(nudge))}));
                        } else if content.is_array() {
                            job.actions.push(json!({"method":"append_content","value":{"type":"text","text":nudge}}));
                        } else {
                            job.actions
                                .push(json!({"method":"set_content","value":nudge}));
                        }
                    }
                }
                for action in job.actions.drain(..) {
                    runtime.apply(action)?;
                }
                self.notify(&out, runtime)?;
            }
            Operation::Stop => {
                let mut payload = self.exported(&job.sid, "Stop", runtime)?;
                payload["prompt_id"] = json!(
                    self.state()?
                        .turns
                        .get(&key(job.sid.as_ref()))
                        .cloned()
                        .unwrap_or_default()
                );
                payload["stop_hook_active"] = json!(false);
                runtime.hook(&payload)?;
                if self.state()?.closing {
                    return Ok(json!({"actions":[]}));
                }
                let end = if runtime.mode()? == "run" {
                    let mut state = self.state()?;
                    if state.closing {
                        return Ok(json!({"actions":[]}));
                    }
                    state.open.shift_remove(&key(job.sid.as_ref())).is_some()
                } else {
                    false
                };
                if end {
                    self.end(&job.sid, "run", runtime)?;
                }
            }
            Operation::End(reason) => self.end(&job.sid, &reason, runtime)?,
            Operation::Exit => {
                let sessions = self.state()?.open.values().cloned().collect::<Vec<_>>();
                for sid in sessions {
                    self.end(&sid, "exit", runtime)?;
                }
                self.state()?.open.clear();
            }
        }
        Ok(json!({"actions":job.actions}))
    }
}
