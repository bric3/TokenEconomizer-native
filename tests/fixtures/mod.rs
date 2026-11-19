// Synthetic transcript writers from reference/upstream/tests/fixtures/__init__.py.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn write_claude_transcript(path: &Path, calls: &[Value]) -> PathBuf {
    let mut lines = Vec::new();
    for (i, call) in calls.iter().enumerate() {
        let id = format!("t{i}");
        let mut event = json!({"message":{"content":[{"type":"tool_use","id":id,"name":call[0],"input":call[1]}]}});
        if let Some(cwd) = call.get(3) {
            event["cwd"] = cwd.clone();
        }
        lines.push(event.to_string());
        lines.push(json!({"message":{"content":[{"type":"tool_result","tool_use_id":id,"content":call[2]}]}}).to_string());
    }
    fs::write(path, lines.join("\n")).unwrap();
    path.into()
}

// Interactive fixture from reference/upstream/tests/test_live_hooks.py.
pub fn write_claude_session(path: &Path, prompts: &[Value], model: &str) -> PathBuf {
    let mut lines = Vec::new();
    let mut n = 0;
    for prompt in prompts {
        lines.push(json!({"type":"user","message":{"role":"user","content":prompt[0]}}));
        for call in prompt[1].as_array().unwrap() {
            n += 1;
            lines.push(json!({"type":"assistant","message":{"id":format!("msg_{}_{n}",path.file_name().unwrap().to_str().unwrap()),"model":model,"role":"assistant",
                "usage":{"input_tokens":10,"cache_read_input_tokens":1000,"cache_creation_input_tokens":100,"output_tokens":50},
                "content":[{"type":"tool_use","id":format!("tu{n}"),"name":call[0],"input":call[1]}]}}));
            lines.push(json!({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":format!("tu{n}"),"content":call[2],"is_error":call[3]}]}}));
        }
    }
    fs::write(
        path,
        format!(
            "{}\n",
            lines
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    path.into()
}

pub fn write_codex_transcript(path: &Path, items: &[Value]) -> PathBuf {
    let lines: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(i, fields)| {
            let mut item = json!({"id":format!("i{i}"),"status":"completed"});
            item.as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            json!({"type":"item.completed","item":item}).to_string()
        })
        .collect();
    fs::write(path, lines.join("\n")).unwrap();
    path.into()
}

pub fn opencode_session_export(sid: &str) -> Value {
    let tokens = json!({"input":18348,"output":68,"reasoning":2,"cache":{"read":896,"write":0}});
    let messages = vec![
        json!({"id":"msg_u","time":{},"type":"user","text":"Run the tests","files":[]}),
        json!({"id":"msg_a","time":{},"type":"assistant","agent":"build","model":{"id":"gpt-5.6-sol","providerID":"openai"},
            "content":[{"type":"text","text":"I'll run them."},
            {"type":"tool","id":"c1","name":"bash","executed":true,"state":{"status":"error","input":{"command":"pytest -q"},
                "error":{"message":"bash: pytest: command not found"}}},
            {"type":"tool","id":"c2","name":"bash","executed":true,"state":{"status":"completed","input":{"command":"python -m pytest -q"},
                "content":[{"type":"text","text":"1 passed in 0.1s"}]}}],"cost":0.0056,"tokens":tokens}),
        json!({"id":"msg_i","time":{},"type":"idle","outcome":"completed"}),
    ];
    json!({"info":{"id":sid,"projectID":"p","cost":0.0056},"messages":messages})
}

// Pi 1.2.0 run fixture from reference/upstream/tests/test_live_core.py.
pub fn pi_json_run() -> String {
    let events = [
        json!({"type":"session","version":3,"id":"01a13c79","cwd":"/tmp/te-side-x"}),
        json!({"type":"message_end","message":{"role":"system","content":"","sections":{"preamble":"You are an expert..."}}}),
        json!({"type":"message_end","message":{"role":"user","content":[{"type":"text","text":"propose facts"}]}}),
        json!({"type":"message_update","usage":{"input":745,"output":3}}),
        json!({"type":"message_end","message":{"role":"assistant","provider":"openrouter","model":"anthropic/claude-haiku-4.5",
            "content":[{"type":"thinking","thinking":"Two lines."},{"type":"text","text":"- use python -m pytest"}],
            "usage":{"input":745,"output":298,"cacheRead":0,"cacheWrite":0,"totalTokens":1043,"cacheWrite1h":0,"reasoning":281,
                "cost":{"input":0.000745,"output":0.00149,"total":0.002235}},"stopReason":"stop"}}),
        json!({"type":"agent_settled","aborted":false}),
    ];
    events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn opencode_run_events() -> String {
    let events = [
        json!({"type":"step_start","sessionID":"ses_x","part":{"type":"step-start"}}),
        json!({"type":"text","sessionID":"ses_x","part":{"type":"text","text":"po"}}),
        json!({"type":"text","sessionID":"ses_x","part":{"type":"text","text":"ng"}}),
        json!({"type":"step_finish","sessionID":"ses_x","part":{"type":"step-finish","cost":0.5,
            "tokens":{"input":10,"output":4,"reasoning":1,"cache":{"read":7,"write":2}}}}),
    ];
    format!(
        "{}\nnot json\n",
        events
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    )
}

// Copilot event fixture from reference/upstream/tests/test_live_core.py.
pub fn write_copilot_events(path: &Path, calls: &[Value], shutdown: bool) -> PathBuf {
    let mut events = vec![
        json!({"type":"session.start","data":{"sessionId":"cp1","copilotVersion":"1.0.83","selectedModel":"gpt-5.4","context":{"cwd":"/w/repo"}}}),
        json!({"type":"user.message","data":{"content":"fix it","turnId":"0"}}),
    ];
    for (n, call) in calls.iter().enumerate() {
        events.push(json!({"type":"tool.execution_start","data":{"toolCallId":format!("c{n}"),"toolName":call[0],"arguments":call[1],"turnId":"0"}}));
        events.push(json!({"type":"tool.execution_complete","data":{"toolCallId":format!("c{n}"),"success":call[3],"turnId":"0",
            "result":{"content":call[2],"detailedContent":call[2]}}}));
    }
    if shutdown {
        events.push(json!({"type":"session.shutdown","data":{"modelMetrics":{"gpt-5.4":{"requests":{"count":4},"usage":{
        "inputTokens":4000,"outputTokens":80,"cacheReadTokens":2400,"cacheWriteTokens":100,"reasoningTokens":0}}},"toolDefinitionsTokens":13149}}));
    }
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            "{}\n",
            events
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    path.into()
}
