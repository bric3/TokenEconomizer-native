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
