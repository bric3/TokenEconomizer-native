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
